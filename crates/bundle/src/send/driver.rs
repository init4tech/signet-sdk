use crate::send::{SignetEthBundle, SignetEthBundleResponse};
use signet_evm::{DriveBundleResult, EvmNeedsTx, SignetLayered};
use signet_zenith::SignedOrderError;
use trevm::{
    helpers::Ctx,
    revm::{context::result::EVMError, Database, DatabaseCommit, Inspector},
    trevm_bail, trevm_ensure, trevm_try, BundleDriver, BundleError,
};

/// Erros while running a [`SignetEthBundle`] on the EVM.
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
    response: SignetEthBundleResponse,
}

impl<'a> SignetEthBundleDriver<'a> {
    /// Creates a new [`SignetEthBundleDriver`] with the given bundle and
    /// response.
    pub const fn new(bundle: &'a SignetEthBundle, response: SignetEthBundleResponse) -> Self {
        Self { bundle, response }
    }

    /// Get a reference to the bundle.
    pub const fn bundle(&self) -> &SignetEthBundle {
        self.bundle
    }

    /// Get a reference to the response.
    pub const fn response(&self) -> &SignetEthBundleResponse {
        &self.response
    }
}

impl<Db, Insp> BundleDriver<Db, SignetLayered<Insp>> for SignetEthBundleDriver<'_>
where
    Db: Database + DatabaseCommit,
    Insp: Inspector<Ctx<Db>>,
{
    type Error = SignetEthBundleError<Db>;

    fn run_bundle(&mut self, mut trevm: EvmNeedsTx<Db, Insp>) -> DriveBundleResult<Self, Db, Insp> {
        let bundle = &self.bundle.bundle;

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
            trevm = match trevm.run_tx(&tx) {
                Ok(trevm) => trevm.accept_state(),
                Err(err) => {
                    let trevm = err.discard_error();
                    if bundle.reverting_tx_hashes.contains(tx.tx_hash()) {
                        trevm
                    } else {
                        return Err(trevm.errored(BundleError::BundleReverted.into()));
                    }
                }
            };
        }

        Ok(trevm)
    }

    fn post_bundle(&mut self, _trevm: &EvmNeedsTx<Db, Insp>) -> Result<(), Self::Error> {
        Ok(())
    }
}
