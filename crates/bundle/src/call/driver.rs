use crate::{SignetCallBundle, SignetCallBundleResponse};
use alloy::{
    consensus::TxEnvelope,
    eips::eip2718::Decodable2718,
    primitives::{bytes::Buf, Bytes, U256},
};
use signet_evm::OrderDetector;
use signet_types::{MarketContext, MarketError};
use std::fmt::Debug;
use trevm::{
    revm::{primitives::EVMError, Database, DatabaseCommit},
    trevm_bail, trevm_ensure, unwrap_or_trevm_err, BundleDriver, BundleError, DriveBundleResult,
    EvmNeedsTx,
};

/// Errors that can occur when running a bundle on the Signet EVM.
#[derive(thiserror::Error)]
pub enum SignetBundleError<Db: Database> {
    /// A primitive [`BundleError`] error ocurred.
    #[error(transparent)]
    BundleError(#[from] BundleError<Db>),
    /// A [`MarketError`] ocurred.
    #[error(transparent)]
    MarketError(#[from] MarketError),
}

impl<Db: Database> Debug for SignetBundleError<Db> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignetBundleError::BundleError(e) => write!(f, "BundleError({:?})", e),
            SignetBundleError::MarketError(e) => write!(f, "MarketError({:?})", e),
        }
    }
}

impl<Db: Database> From<EVMError<Db::Error>> for SignetBundleError<Db> {
    fn from(e: EVMError<Db::Error>) -> Self {
        SignetBundleError::BundleError(BundleError::EVMError { inner: e })
    }
}

/// A bundle driver for the Signet EVM.
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
        let context = bundle.make_context(host_chain_id);
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
    /// reset the driver, allowing for resimulation of the same bundle.
    ///
    /// The returned context contains the amount of overfill, i.e. the amount
    /// that was filled, but not required by the orders in the bundle.
    pub fn clear(&mut self) -> (SignetCallBundleResponse, MarketContext) {
        let r = std::mem::take(&mut self.response);
        let context = self.bundle.make_context(self.host_chain_id);
        let c = std::mem::replace(&mut self.context, context);
        (r, c)
    }

    /// Decode and validate the transactions in the bundle.
    pub fn decode_and_validate_txs<Db: Database>(
        txs: &[Bytes],
    ) -> Result<Vec<TxEnvelope>, SignetBundleError<Db>> {
        let txs = txs
            .iter()
            .map(|tx| TxEnvelope::decode_2718(&mut tx.chunk()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                SignetBundleError::BundleError(BundleError::TransactionDecodingError(err))
            })?;

        if txs.iter().any(|tx| tx.is_eip4844()) {
            return Err(BundleError::UnsupportedTransactionType.into());
        }

        Ok(txs)
    }
}

// [`BundleDriver`] Implementation for [`SignetCallBundle`].
// This is useful mainly for the `signet_simBundle` endpoint,
// which is used to simulate a signet bundle while respecting market context.
impl<I> BundleDriver<OrderDetector<I>> for SignetBundleDriver<'_> {
    type Error<Db: Database + DatabaseCommit> = SignetBundleError<Db>;

    fn run_bundle<'a, Db: Database + DatabaseCommit>(
        &mut self,
        trevm: EvmNeedsTx<'a, OrderDetector<I>, Db>,
    ) -> DriveBundleResult<'a, OrderDetector<I>, Db, Self> {
        // convenience binding to make usage later less verbose
        let bundle = &self.bundle.bundle;

        // Check if the block we're in is valid for this bundle. Both must match
        trevm_ensure!(
            trevm.inner().block().number.to::<u64>() == bundle.block_number,
            trevm,
            BundleError::BlockNumberMismatch.into()
        );

        // Ensure that the bundle has transactions
        trevm_ensure!(!bundle.txs.is_empty(), trevm, BundleError::BundleEmpty.into());

        // Check if the state block number is valid (not 0, and not a tag)
        trevm_ensure!(
            bundle.state_block_number.is_number()
                && bundle.state_block_number.as_number().unwrap_or_default() != 0,
            trevm,
            BundleError::BlockNumberMismatch.into()
        );

        // Set the state block number this simulation was based on
        self.response.state_block_number = bundle
            .state_block_number
            .as_number()
            .unwrap_or(trevm.inner().block().number.to::<u64>());

        let run_result = trevm.try_with_block(self.bundle, |mut trevm| {
            // Decode and validate the transactions in the bundle
            let txs = unwrap_or_trevm_err!(Self::decode_and_validate_txs(&bundle.txs), trevm);

            // Cache the pre simulation coinbase balance, so we can use it to calculate the coinbase diff after every tx simulated.
            let initial_coinbase_balance = unwrap_or_trevm_err!(
                trevm.try_read_balance(trevm.inner().block().coinbase).map_err(|e| {
                    SignetBundleError::BundleError(BundleError::EVMError {
                        inner: trevm::revm::primitives::EVMError::Database(e),
                    })
                }),
                trevm
            );

            // Stack vars to keep track of the coinbase balance across txs.
            let mut pre_sim_coinbase_balance = initial_coinbase_balance;
            let post_sim_coinbase_balance = pre_sim_coinbase_balance;

            for tx in txs.iter() {
                let run_result = trevm.run_tx(tx);

                let mut res = run_result
                    .map_err(|e| e.map_err(|e| SignetBundleError::BundleError(e.into())))?;

                // Check & respect market context. This MUST be done after every transaction before accepting the state.
                let (aggregate, mkt_ctx) =
                    res.inner_mut_unchecked().context.external.take_aggregate();

                if let Err(err) = self.context.checked_remove_ru_tx_events(&aggregate, &mkt_ctx) {
                    tracing::debug!(%err, "Discarding bundle simulation due to market error");
                    return Err(res.errored(SignetBundleError::MarketError(err)));
                }

                // Accept the tx outcome.
                let (execution_result, mut committed_trevm) = res.accept();

                // Get the coinbase and basefee from the block
                let coinbase = committed_trevm.inner().block().coinbase;
                let basefee = committed_trevm.inner().block().basefee;

                // Get the post simulation coinbase balance
                let post_sim_coinbase_balance = unwrap_or_trevm_err!(
                    committed_trevm.try_read_balance(coinbase).map_err(|e| {
                        SignetBundleError::BundleError(BundleError::EVMError {
                            inner: trevm::revm::primitives::EVMError::Database(e),
                        })
                    }),
                    committed_trevm
                );

                // Calculate the coinbase diff
                let coinbase_diff =
                    post_sim_coinbase_balance.saturating_sub(pre_sim_coinbase_balance);

                // Accumulate the transaction
                unwrap_or_trevm_err!(
                    self.response.accumulate_tx(tx, coinbase_diff, basefee, execution_result),
                    committed_trevm
                );

                // update the coinbase balance
                pre_sim_coinbase_balance = post_sim_coinbase_balance;

                // Set the trevm instance to the committed one
                trevm = committed_trevm;
            }

            // Accumulate the total results
            self.response.coinbase_diff =
                post_sim_coinbase_balance.saturating_sub(initial_coinbase_balance);
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
        });

        run_result
    }

    fn post_bundle<Db: Database + DatabaseCommit>(
        &mut self,
        _trevm: &EvmNeedsTx<'_, OrderDetector<I>, Db>,
    ) -> Result<(), Self::Error<Db>> {
        Ok(())
    }
}
