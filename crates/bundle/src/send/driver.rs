use crate::send::SignetEthBundle;
use alloy::primitives::U256;
use signet_evm::{DriveBundleResult, EvmNeedsTx, SignetLayered};
use signet_zenith::SignedOrderError;
use trevm::{
    helpers::Ctx,
    inspectors::{Layered, TimeLimit},
    revm::{
        context::result::{EVMError, ExecutionResult, HaltReason},
        inspector::InspectorEvmTr,
        Database, DatabaseCommit, Inspector,
    },
    trevm_bail, trevm_ensure, trevm_try, BundleDriver, BundleError,
};

/// Inspector used in the impl of [`BundleDriver`] for
/// [`SignetEthBundleDriver`].
pub type SignetEthBundleInsp<I> = Layered<TimeLimit, I>;

/// Errors while running a [`SignetEthBundle`] on the EVM.
#[derive(thiserror::Error)]
pub enum SignetEthBundleError<Db: Database> {
    /// Bundle error.
    #[error(transparent)]
    BundleError(#[from] BundleError<Db>),

    /// SignedOrderError.
    #[error(transparent)]
    SignedOrderError(#[from] SignedOrderError),

    /// Contract error.
    #[error(transparent)]
    ContractError(#[from] alloy::contract::Error),
}

impl<Db: Database> core::fmt::Debug for SignetEthBundleError<Db> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignetEthBundleError::BundleError(bundle_error) => {
                f.debug_tuple("BundleError").field(bundle_error).finish()
            }
            SignetEthBundleError::SignedOrderError(signed_order_error) => {
                f.debug_tuple("SignedOrderError").field(signed_order_error).finish()
            }
            SignetEthBundleError::ContractError(contract_error) => {
                f.debug_tuple("ContractError").field(contract_error).finish()
            }
        }
    }
}

impl<Db: Database> From<EVMError<Db::Error>> for SignetEthBundleError<Db> {
    fn from(err: EVMError<Db::Error>) -> Self {
        Self::BundleError(BundleError::from(err))
    }
}

/// Driver for applying a Signet Ethereum bundle to an EVM.
#[derive(Debug, Clone)]
pub struct SignetEthBundleDriver<'a> {
    bundle: &'a SignetEthBundle,
    deadline: std::time::Instant,

    total_gas_used: u64,
    beneficiary_balance_increase: U256,
}

impl<'a> SignetEthBundleDriver<'a> {
    /// Creates a new [`SignetEthBundleDriver`] with the given bundle and
    /// response.
    pub const fn new(bundle: &'a SignetEthBundle, deadline: std::time::Instant) -> Self {
        Self { bundle, deadline, total_gas_used: 0, beneficiary_balance_increase: U256::ZERO }
    }

    /// Get a reference to the bundle.
    pub const fn bundle(&self) -> &SignetEthBundle {
        self.bundle
    }

    /// Get the deadline for this driver.
    pub const fn deadline(&self) -> std::time::Instant {
        self.deadline
    }

    /// Get the total gas used by this driver during tx execution.
    pub const fn total_gas_used(&self) -> u64 {
        self.total_gas_used
    }

    /// Get the beneficiary balance increase for this driver during tx execution.
    pub const fn beneficiary_balance_increase(&self) -> U256 {
        self.beneficiary_balance_increase
    }
}

impl<Db, Insp> BundleDriver<Db, SignetLayered<Layered<TimeLimit, Insp>>>
    for SignetEthBundleDriver<'_>
where
    Db: Database + DatabaseCommit,
    Insp: Inspector<Ctx<Db>>,
{
    type Error = SignetEthBundleError<Db>;

    fn run_bundle(
        &mut self,
        mut trevm: EvmNeedsTx<Db, SignetEthBundleInsp<Insp>>,
    ) -> DriveBundleResult<Self, Db, SignetEthBundleInsp<Insp>> {
        let bundle = &self.bundle.bundle;

        // Reset the total gas used and beneficiary balance increase
        // to 0 before running the bundle.
        self.total_gas_used = 0;
        self.beneficiary_balance_increase = U256::ZERO;

        // Get the beneficiary address and its initial balance
        let beneficiary = trevm.beneficiary();
        let inital_beneficiary_balance =
            trevm_try!(trevm.try_read_balance(beneficiary).map_err(EVMError::Database), trevm);

        // Ensure that the bundle has transactions
        trevm_ensure!(!bundle.txs.is_empty(), trevm, BundleError::BundleEmpty.into());

        // Check if the block we're in is valid for this bundle. Both must match
        trevm_ensure!(
            trevm.block_number() == bundle.block_number,
            trevm,
            BundleError::BlockNumberMismatch.into()
        );

        // Check if the state block number is valid (not 0, and not a tag)
        let timestamp = trevm.block_timestamp();
        trevm_ensure!(
            timestamp >= bundle.min_timestamp.unwrap_or_default()
                && timestamp <= bundle.max_timestamp.unwrap_or(u64::MAX),
            trevm,
            BundleError::TimestampOutOfRange.into()
        );

        // Check that the `SignedOrder` is valid at the timestamp.
        if self.bundle().validate_fills_offchain(timestamp).is_err() {
            return Err(trevm.errored(BundleError::BundleReverted.into()));
        }

        // Decode and validate the transactions in the bundle
        let txs = trevm_try!(self.bundle.decode_and_validate_txs(), trevm);

        for tx in txs.into_iter() {
            // Update the inner deadline.
            let limit = trevm.inner_mut_unchecked().ctx_inspector().1.outer_mut().outer_mut();
            *limit = TimeLimit::new(self.deadline - std::time::Instant::now());

            let tx_hash = tx.hash();

            trevm = match trevm.run_tx(&tx) {
                Ok(trevm) => {
                    // Check if the transaction was reverted or halted
                    let result = trevm.result();

                    match result {
                        ExecutionResult::Success { gas_used, .. } => {
                            self.total_gas_used = self.total_gas_used.saturating_add(*gas_used);
                        }
                        // `CallTooDeep` represents the timelimit being reached
                        ExecutionResult::Halt { reason, .. }
                            if *reason == HaltReason::CallTooDeep =>
                        {
                            return Err(trevm.errored(BundleError::BundleReverted.into()));
                        }
                        ExecutionResult::Halt { gas_used, .. }
                        | ExecutionResult::Revert { gas_used, .. } => {
                            if !self.bundle.reverting_tx_hashes().contains(tx_hash) {
                                return Err(trevm.errored(BundleError::BundleReverted.into()));
                            }
                            self.total_gas_used = self.total_gas_used.saturating_add(*gas_used);
                        }
                    }
                    trevm.accept_state()
                }
                Err(err) => return Err(err.err_into()),
            };
        }

        let beneficiary_balance =
            trevm_try!(trevm.try_read_balance(beneficiary).map_err(EVMError::Database), trevm);

        self.beneficiary_balance_increase =
            beneficiary_balance.saturating_sub(inital_beneficiary_balance);

        Ok(trevm)
    }

    fn post_bundle(
        &mut self,
        _trevm: &EvmNeedsTx<Db, SignetEthBundleInsp<Insp>>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
