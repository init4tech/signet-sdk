use crate::send::SignetEthBundle;
use alloy::primitives::U256;
use signet_evm::{
    DriveBundleResult, EvmErrored, EvmNeedsTx, EvmTransacted, SignetInspector, SignetLayered,
};
use signet_types::{AggregateFills, MarketError, SignedPermitError};
use tracing::{debug, error};
use trevm::{
    helpers::Ctx,
    inspectors::{Layered, TimeLimit},
    revm::{
        context::result::EVMError, inspector::InspectorEvmTr, Database, DatabaseCommit, Inspector,
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

    /// SignedPermitError.
    #[error(transparent)]
    SignedPermitError(#[from] SignedPermitError),

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
            SignetEthBundleError::SignedPermitError(signed_order_error) => {
                f.debug_tuple("SignedPermitError").field(signed_order_error).finish()
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
    /// The bundle to apply.
    bundle: &'a SignetEthBundle,

    /// Execution deadline for this bundle. This limits the total WALLCLOCK
    /// time spent simulating the bundle.
    deadline: std::time::Instant,

    /// Aggregate fills derived from the bundle's host fills.
    agg_fills: AggregateFills,

    /// Total gas used by this bundle during execution, an output of the driver.
    total_gas_used: u64,
    /// Beneficiary balance increase during execution, an output of the driver.
    beneficiary_balance_increase: U256,
}

impl<'a> SignetEthBundleDriver<'a> {
    /// Creates a new [`SignetEthBundleDriver`] with the given bundle and
    /// response.
    pub fn new(
        bundle: &'a SignetEthBundle,
        host_chain_id: u64,
        deadline: std::time::Instant,
    ) -> Self {
        let mut agg_fills = AggregateFills::default();
        if let Some(host_fills) = &bundle.host_fills {
            agg_fills.add_signed_fill(host_chain_id, host_fills);
        }

        Self {
            bundle,
            deadline,
            agg_fills,
            total_gas_used: 0,
            beneficiary_balance_increase: U256::ZERO,
        }
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

    /// Get the aggregate fills for this driver.
    ///
    /// This may be used to check that the bundle does not overfill, by
    /// inspecting the agg fills after execution.
    pub const fn agg_fills(&self) -> &AggregateFills {
        &self.agg_fills
    }

    /// Check the [`AggregateFills`], discard if invalid, otherwise accumulate
    /// payable gas and call [`Self::accept_tx`].
    ///
    /// This path is used by
    /// - [`TransactionSigned`] objects
    /// - [`Transactor::Transact`] events
    pub(crate) fn check_fills<Db, Insp>(
        &mut self,
        trevm: &mut EvmTransacted<Db, Insp>,
    ) -> Result<(), MarketError>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // Taking these clears the context for reuse.
        let (agg_orders, agg_fills) =
            trevm.inner_mut_unchecked().inspector.as_mut_detector().take_aggregates();

        // We check the AggregateFills here, and if it fails, we discard the
        // transaction outcome and push a failure receipt.
        self.agg_fills.checked_remove_ru_tx_events(&agg_orders, &agg_fills)
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
            trevm.block_number().to::<u64>() == bundle.block_number,
            trevm,
            BundleError::BlockNumberMismatch.into()
        );

        // Check if the state block number is valid (not 0, and not a tag)
        let timestamp = trevm.block_timestamp();
        trevm_ensure!(
            self.bundle.is_valid_at_timestamp(timestamp.to()),
            trevm,
            BundleError::TimestampOutOfRange.into()
        );

        // Check that the `SignedFill` is valid at the timestamp.
        if self.bundle().validate_fills_offchain(timestamp.to()).is_err() {
            return Err(trevm.errored(BundleError::BundleReverted.into()));
        }

        // Decode and validate the transactions in the bundle
        let txs = trevm_try!(self.bundle.decode_and_validate_txs(), trevm);

        for tx in txs.into_iter() {
            let _span = tracing::debug_span!("bundle_tx_loop", tx_hash = %tx.hash()).entered();

            // Update the inner deadline.
            let limit = trevm.inner_mut_unchecked().ctx_inspector().1.outer_mut().outer_mut();
            *limit = TimeLimit::new(self.deadline - std::time::Instant::now());

            let tx_hash = tx.hash();

            // Temporary rebinding of trevm within each loop iteration.
            // The type of t is `EvmTransacted`, while the type of trevm is
            // `EvmNeedsTx`.
            let mut t = trevm
                .run_tx(&tx)
                .map_err(EvmErrored::err_into)
                .inspect_err(|err| error!(err = %err.error(), "error while running transaction"))?;

            // Check the result of the transaction.
            let result = t.result();

            let gas_used = result.gas_used();

            // EVM Execution succeeded.
            // We now check if the orders are valid with the bundle's fills. If
            // not, and the tx is not marked as revertible by the bundle, we
            // error our simulation.
            if result.is_success() {
                if self.check_fills(&mut t).is_err() {
                    debug!("transaction dropped due to insufficient fills");
                    if self.bundle.reverting_tx_hashes().contains(tx_hash) {
                        trevm = t.reject();
                        continue;
                    } else {
                        return Err(t.errored(BundleError::BundleReverted.into()));
                    }
                }

                self.total_gas_used = self.total_gas_used.saturating_add(gas_used);
            } else {
                // If not success, we are in a revert or halt. If the tx is
                // not marked as revertible by the bundle, we error our
                // simulation.
                if !self.bundle.reverting_tx_hashes().contains(tx_hash) {
                    debug!("transaction reverted, not marked as revertible");
                    return Err(t.errored(BundleError::BundleReverted.into()));
                }
                self.total_gas_used = self.total_gas_used.saturating_add(gas_used);
            }

            // If we did not shortcut return/continue, we accept the state
            // changes from this transaction.
            trevm = t.accept_state()
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
