use crate::{SignetBundleError, SignetCallBundle, SignetCallBundleResponse};
use alloy::{consensus::TxEnvelope, primitives::U256};
use signet_evm::OrderDetector;
use signet_types::MarketContext;
use std::fmt::Debug;
use trevm::{
    revm::{Database, DatabaseCommit},
    trevm_bail, trevm_ensure, unwrap_or_trevm_err, BundleDriver, BundleError,
};

/// A bundle driver for the Signet EVM.
///
/// This type allows for the simulation of a [`SignetCallBundle`] and ensuring
/// that it conforms to market rules as a unit.
#[derive(Debug)]
pub struct SignetBundleDriver<'a> {
    /// The bundle to drive.
    bundle: &'a SignetCallBundle,
    /// The accumulated results of the bundle, if applicable.
    response: SignetCallBundleResponse,
    /// The market context.
    context: MarketContext,
    /// The host chain id.
    host_chain_id: u64,
}

impl<'a> SignetBundleDriver<'a> {
    /// Create a new bundle driver with the given bundle and response.
    pub fn new(bundle: &'a SignetCallBundle, host_chain_id: u64) -> Self {
        let context = bundle.build_context(host_chain_id);
        Self { bundle, response: Default::default(), context, host_chain_id }
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

    /// Get a reference to the market context.
    pub const fn context(&self) -> &MarketContext {
        &self.context
    }

    /// Take the response from the bundle driver. This consumes
    pub fn into_response(self) -> SignetCallBundleResponse {
        self.response
    }

    /// Clear the driver, resetting the response and the market context. This
    /// resets the driver, allowing for re-simulation of the same bundle.
    ///
    /// The returned context contains the amount of overfill, i.e. the amount
    /// that was filled, but not required by the orders in the bundle.
    pub fn clear(&mut self) -> (SignetCallBundleResponse, MarketContext) {
        let r = std::mem::take(&mut self.response);
        let context = self.bundle.build_context(self.host_chain_id);
        let c = std::mem::replace(&mut self.context, context);
        (r, c)
    }

    /// Check the market context, accept the result, accumulate the transaction
    /// details into the response.
    fn check_market_and_accept<'a, Db: Database + DatabaseCommit, I>(
        &mut self,
        mut trevm: signet_evm::EvmTransacted<'a, Db, I>,
        tx: &TxEnvelope,
        pre_sim_coinbase_balance: &mut U256,
        basefee: U256,
    ) -> signet_evm::DriveBundleResult<'a, Db, Self, I> {
        let coinbase = trevm.inner().block().coinbase;

        // Taking these clears the context for reuse.
        let (aggregate, market_context) =
            trevm.inner_mut_unchecked().context.external.take_aggregate();

        // We check the market context here, and if it fails, we discard the
        // transaction outcome and push a failure receipt.
        if let Err(err) = self.context.checked_remove_ru_tx_events(&aggregate, &market_context) {
            tracing::debug!(%err, "Discarding transaction outcome due to market error");
            return Err(trevm.errored(SignetBundleError::MarketError(err)));
        }

        let (execution_result, mut trevm) = trevm.accept();

        // Get the post simulation coinbase balance
        let post_sim_coinbase_balance = unwrap_or_trevm_err!(
            trevm.try_read_balance(coinbase).map_err(SignetBundleError::evm_db),
            trevm
        );

        // Calculate the coinbase diff
        let coinbase_diff = post_sim_coinbase_balance.saturating_sub(*pre_sim_coinbase_balance);

        // Accumulate the transaction
        unwrap_or_trevm_err!(
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
// which is used to simulate a signet bundle while respecting market context.
impl<I> BundleDriver<OrderDetector<I>> for SignetBundleDriver<'_> {
    type Error<Db: Database + DatabaseCommit> = SignetBundleError<Db>;

    fn run_bundle<'a, Db: Database + DatabaseCommit>(
        &mut self,
        trevm: signet_evm::EvmNeedsTx<'a, Db, I>,
    ) -> signet_evm::DriveBundleResult<'a, Db, Self, I> {
        // convenience binding to make usage later less verbose
        let bundle = &self.bundle.bundle;

        // Ensure that the bundle has transactions
        trevm_ensure!(!bundle.txs.is_empty(), trevm, BundleError::BundleEmpty.into());

        // Check if the block we're in is valid for this bundle. Both must match
        trevm_ensure!(
            trevm.block_number().to::<u64>() == bundle.block_number,
            trevm,
            BundleError::BlockNumberMismatch.into()
        );
        // Set the state block number this simulation was based on
        self.response.state_block_number = trevm.block_number().to::<u64>();

        // Check if the state block number is valid (not 0, and not a tag)
        trevm_ensure!(
            bundle.state_block_number.is_number()
                && bundle.state_block_number.as_number().unwrap_or_default() != 0,
            trevm,
            BundleError::BlockNumberMismatch.into()
        );

        // Decode and validate the transactions in the bundle
        let txs = unwrap_or_trevm_err!(self.bundle.decode_and_validate_txs(), trevm);

        trevm.try_with_block(self.bundle, |mut trevm| {
            // Get the coinbase and basefee from the block
            // NB: Do not move these outside the `try_with_block` closure, as
            // they may be rewritten by the bundle
            let coinbase = trevm.block().coinbase;
            let basefee = trevm.block().basefee;

            // Cache the pre simulation coinbase balance, so we can use it to calculate the coinbase diff after every tx simulated.
            let initial_coinbase_balance = unwrap_or_trevm_err!(
                trevm.try_read_balance(coinbase).map_err(SignetBundleError::evm_db),
                trevm
            );

            // Stack vars to keep track of the coinbase balance across txs.
            let mut pre_sim_coinbase_balance = initial_coinbase_balance;

            for tx in txs.iter() {
                let run_result = trevm.run_tx(tx);

                let transacted_trevm = run_result.map_err(|e| e.map_err(Into::into))?;

                // Set the trevm instance to the committed one
                trevm = self.check_market_and_accept(
                    transacted_trevm,
                    tx,
                    &mut pre_sim_coinbase_balance,
                    basefee,
                )?;
            }

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

            // return the final state
            Ok(trevm)
        })
    }

    fn post_bundle<Db: Database + DatabaseCommit>(
        &mut self,
        _trevm: &signet_evm::EvmNeedsTx<'_, Db, I>,
    ) -> Result<(), Self::Error<Db>> {
        Ok(())
    }
}
