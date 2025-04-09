use std::u64;

use crate::send::{SignetEthBundle, SignetEthBundleResponse};
use signet_evm::{DriveBundleResult, EvmNeedsTx, SignetLayered};
use trevm::{
    helpers::Ctx,
    revm::{Database, DatabaseCommit, Inspector},
    trevm_bail, trevm_ensure, trevm_try, BundleDriver, BundleError,
};

/// Driver for applying a Signet Ethereum bundle to an EVM.
#[derive(Debug, Clone)]
pub struct SignetEthBundleDriver<'a> {
    bundle: &'a SignetEthBundle,
    response: SignetEthBundleResponse,
}

impl<'a> SignetEthBundleDriver<'a> {
    /// Creates a new [`SignetEthBundleDriver`] with the given bundle and
    /// response.
    pub fn new(bundle: &'a SignetEthBundle, response: SignetEthBundleResponse) -> Self {
        Self { bundle, response }
    }

    /// Get a reference to the bundle.
    pub fn bundle(&self) -> &SignetEthBundle {
        self.bundle
    }

    /// Get a reference to the response.
    pub fn response(&self) -> &SignetEthBundleResponse {
        &self.response
    }
}

impl<'a, Db, Insp> BundleDriver<Db, SignetLayered<Insp>> for SignetEthBundleDriver<'a>
where
    Db: Database + DatabaseCommit,
    Insp: Inspector<Ctx<Db>>,
{
    type Error = BundleError<Db>;

    fn run_bundle(&mut self, mut trevm: EvmNeedsTx<Db, Insp>) -> DriveBundleResult<Self, Db, Insp> {
        let bundle = &self.bundle.bundle;

        // Ensure that the bundle has transactions
        trevm_ensure!(!bundle.txs.is_empty(), trevm, BundleError::BundleEmpty);

        // Check if the block we're in is valid for this bundle. Both must match
        trevm_ensure!(
            trevm.block_number() == bundle.block_number,
            trevm,
            BundleError::BlockNumberMismatch
        );

        // Check if the state block number is valid (not 0, and not a tag)
        let timestamp = trevm.block_timestamp();
        trevm_ensure!(
            timestamp >= bundle.min_timestamp.unwrap_or_default()
                && timestamp <= bundle.max_timestamp.unwrap_or(u64::MAX),
            trevm,
            BundleError::TimestampOutOfRange
        );

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
                        return Err(trevm.errored(BundleError::BundleReverted));
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
