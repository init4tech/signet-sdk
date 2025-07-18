use crate::{SignetCallBundle, SignetCallBundleResponse};
use alloy::{consensus::TxEnvelope, primitives::U256};
use signet_evm::{DriveBundleResult, EvmNeedsTx, EvmTransacted, SignetInspector, SignetLayered};
use std::fmt::Debug;
use tracing::{debug_span, instrument, Level};
use trevm::{
    helpers::Ctx,
    revm::{context::result::EVMError, Database, DatabaseCommit, Inspector},
    trevm_bail, trevm_ensure, trevm_try, BundleDriver, BundleError,
};

/// A call bundle driver for the Signet EVM.
///
/// This type allows for the simulation of a [`SignetCallBundle`], outputting
/// the results of the simulation in a [`SignetCallBundleResponse`].
#[derive(Debug)]
pub struct SignetBundleDriver<'a> {
    /// The bundle to drive.
    bundle: &'a SignetCallBundle,
    /// The accumulated results of the bundle, if applicable.
    response: SignetCallBundleResponse,
}

impl<'a> From<&'a SignetCallBundle> for SignetBundleDriver<'a> {
    fn from(bundle: &'a SignetCallBundle) -> Self {
        Self::new(bundle)
    }
}

impl<'a> SignetBundleDriver<'a> {
    /// Create a new bundle driver with the given bundle and response.
    pub fn new(bundle: &'a SignetCallBundle) -> Self {
        Self { bundle, response: Default::default() }
    }
}

impl SignetBundleDriver<'_> {
    /// Get a reference to the bundle.
    pub const fn bundle(&self) -> &SignetCallBundle {
        self.bundle
    }

    /// Get a reference to the response.
    pub const fn response(&self) -> &SignetCallBundleResponse {
        &self.response
    }

    /// Take the response from the bundle driver. This consumes
    pub fn into_response(self) -> SignetCallBundleResponse {
        self.response
    }

    /// Clear the driver, resetting the response.
    pub fn clear(&mut self) -> SignetCallBundleResponse {
        std::mem::take(&mut self.response)
    }

    /// Check the aggregate fills, accept the result, accumulate the transaction
    /// details into the response.
    fn accept_and_accumulate<Db, Insp>(
        &mut self,
        trevm: EvmTransacted<Db, Insp>,
        tx: &TxEnvelope,
        pre_sim_coinbase_balance: &mut U256,
        basefee: u64,
    ) -> DriveBundleResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        let beneficiary = trevm.beneficiary();

        let (execution_result, mut trevm) = trevm.accept();

        // Get the post simulation coinbase balance
        let post_sim_coinbase_balance = trevm_try!(
            trevm
                .try_read_balance(beneficiary)
                .map_err(EVMError::Database)
                .map_err(BundleError::from),
            trevm
        );

        // Calculate the coinbase diff
        let coinbase_diff = post_sim_coinbase_balance.saturating_sub(*pre_sim_coinbase_balance);

        // Accumulate the transaction
        trevm_try!(
            self.response.accumulate_tx(tx, coinbase_diff, basefee, execution_result),
            trevm
        );

        // update the coinbase balance
        *pre_sim_coinbase_balance = post_sim_coinbase_balance;

        Ok(trevm)
    }
}

// [`BundleDriver`] Implementation for [`SignetCallBundle`].
// This is useful mainly for the `signet_simBundle` endpoint,
// which is used to simulate a signet bundle while respecting aggregate fills.
impl<Db, Insp> BundleDriver<Db, SignetLayered<Insp>> for SignetBundleDriver<'_>
where
    Db: Database + DatabaseCommit,
    Insp: Inspector<Ctx<Db>>,
{
    type Error = BundleError<Db>;

    #[instrument(skip_all, level = Level::DEBUG)]
    fn run_bundle(&mut self, trevm: EvmNeedsTx<Db, Insp>) -> DriveBundleResult<Self, Db, Insp> {
        // convenience binding to make usage later less verbose
        let bundle = &self.bundle.bundle;

        // Ensure that the bundle has transactions
        trevm_ensure!(!bundle.txs.is_empty(), trevm, BundleError::BundleEmpty);

        // Check if the block we're in is valid for this bundle. Both must match
        trevm_ensure!(
            trevm.block_number().to::<u64>() == bundle.block_number,
            trevm,
            BundleError::BlockNumberMismatch
        );
        // Set the state block number this simulation was based on
        self.response.state_block_number = trevm.block_number().to();

        // Check if the state block number is valid (not 0, and not a tag)
        trevm_ensure!(
            bundle.state_block_number.is_number()
                && bundle.state_block_number.as_number().unwrap_or_default() != 0,
            trevm,
            BundleError::BlockNumberMismatch
        );

        // Decode and validate the transactions in the bundle
        let txs = trevm_try!(self.bundle.decode_and_validate_txs(), trevm);

        trevm.try_with_block(self.bundle, |mut trevm| {
            // Get the coinbase and basefee from the block
            // NB: Do not move these outside the `try_with_block` closure, as
            // they may be rewritten by the bundle
            let coinbase = trevm.beneficiary();
            let basefee = trevm.block().basefee;

            // Cache the pre simulation coinbase balance, so we can use it to calculate the coinbase diff after every tx simulated.
            let initial_coinbase_balance = trevm_try!(
                trevm
                    .try_read_balance(coinbase)
                    .map_err(EVMError::Database)
                    .map_err(BundleError::from),
                trevm
            );

            // Stack vars to keep track of the coinbase balance across txs.
            let mut pre_sim_coinbase_balance = initial_coinbase_balance;

            let span = debug_span!("bundle loop", count = txs.len()).entered();
            for (idx, tx) in txs.iter().enumerate() {
                let _span = debug_span!("tx loop", tx = %tx.tx_hash(), idx).entered();
                let run_result = trevm.run_tx(tx);

                let transacted_trevm = run_result.map_err(|e| e.map_err(Into::into))?;

                // Set the trevm instance to the committed one
                trevm = self.accept_and_accumulate(
                    transacted_trevm,
                    tx,
                    &mut pre_sim_coinbase_balance,
                    basefee,
                )?;
            }
            drop(span);

            // Accumulate the total results
            self.response.coinbase_diff =
                pre_sim_coinbase_balance.saturating_sub(initial_coinbase_balance);
            self.response.eth_sent_to_coinbase =
                self.response.coinbase_diff.saturating_sub(self.response.gas_fees);
            self.response.bundle_gas_price = self
                .response
                .coinbase_diff
                .checked_div(U256::from(self.response.total_gas_used))
                .unwrap_or_default();
            self.response.bundle_hash = self.bundle.bundle_hash();

            // Taking these clears the order detector
            let (orders, fills) =
                trevm.inner_mut_unchecked().inspector.as_mut_detector().take_aggregates();
            self.response.orders = orders;
            self.response.fills = fills;

            // return the final state
            Ok(trevm)
        })
    }

    fn post_bundle(&mut self, _trevm: &EvmNeedsTx<Db, Insp>) -> Result<(), Self::Error> {
        Ok(())
    }
}
