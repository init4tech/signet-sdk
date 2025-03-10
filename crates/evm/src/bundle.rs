//! Signet's bundle driver and related bundle utilities.

use alloy::{
    consensus::{Transaction, TxEnvelope},
    eips::eip2718::Decodable2718,
    primitives::{bytes::Buf, Address, Bytes, TxKind, U256},
    rpc::types::mev::EthCallBundleTransactionResult,
};
use signet_types::{
    bundle::SignetCallBundleResponse, MarketContext, MarketError, SignetCallBundle,
};
use std::fmt::Debug;
use trevm::{
    revm::{
        primitives::{EVMError, ExecutionResult},
        Database, DatabaseCommit,
    },
    trevm_bail, trevm_ensure, unwrap_or_trevm_err, BundleDriver, BundleError,
};
use zenith_types::HostOrders::{self, Output};

use crate::OrderDetector;

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
pub struct SignetBundleDriver<B, R> {
    /// The bundle to drive.
    bundle: B,
    /// The accumulated results of the bundle, if applicable.
    response: R,
    /// The market context.
    context: MarketContext,
    /// The host chain id.
    host_chain_id: u64,
}

impl SignetBundleDriver<SignetCallBundle, SignetCallBundleResponse> {
    /// Create a new bundle driver with the given bundle and response.
    pub fn new(bundle: SignetCallBundle, host_chain_id: u64) -> Self {
        let mut context = MarketContext::default();
        bundle.host_fills.iter().for_each(|(asset, fills)| {
            fills.iter().for_each(|(recipient, amount)| {
                context.add_raw_fill(host_chain_id, *asset, *recipient, *amount)
            })
        });

        Self {
            bundle,
            response: Default::default(),
            context,
            host_chain_id,
        }
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
        let c = std::mem::take(&mut self.context);
        (r, c)
    }
}

impl<R> SignetBundleDriver<SignetCallBundle, R> {
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

impl<B, R> SignetBundleDriver<B, R> {
    /// Get a reference to the bundle.
    pub const fn bundle(&self) -> &B {
        &self.bundle
    }

    /// Get a reference to the response.
    pub const fn response(&self) -> &R {
        &self.response
    }

    /// Get a reference to the market context.
    pub const fn context(&self) -> &MarketContext {
        &self.context
    }

    /// Process a bundle transaction and accumulate the results into a [`EthCallBundleTransactionResult`].
    pub fn process_call_bundle_tx<Db: Database>(
        tx: &TxEnvelope,
        pre_sim_coinbase_balance: U256,
        post_sim_coinbase_balance: U256,
        basefee: U256,
        execution_result: ExecutionResult,
    ) -> Result<(EthCallBundleTransactionResult, U256), SignetBundleError<Db>> {
        if let TxEnvelope::Eip4844(_) = tx {
            return Err(SignetBundleError::BundleError(
                BundleError::UnsupportedTransactionType,
            ));
        }

        let from_address = tx.recover_signer().map_err(|e| {
            SignetBundleError::BundleError(BundleError::TransactionSenderRecoveryError(e))
        })?;

        let gas_used = execution_result.gas_used();

        // Calculate the gas price
        let gas_price = match tx {
            TxEnvelope::Legacy(tx) => U256::from(tx.tx().gas_price),
            TxEnvelope::Eip2930(tx) => U256::from(tx.tx().gas_price),
            TxEnvelope::Eip1559(tx) => {
                U256::from(tx.tx().effective_gas_price(Some(basefee.to::<u64>())))
            }
            _ => unreachable!(),
        };

        // Calculate the gas fees paid
        let gas_fees = gas_price * U256::from(gas_used);

        // set the return data for the response
        let (value, revert) = if execution_result.is_success() {
            let value = execution_result.into_output().unwrap_or_default();
            (Some(value), None)
        } else {
            let revert = execution_result.into_output().unwrap_or_default();
            (None, Some(revert))
        };

        let coinbase_diff = post_sim_coinbase_balance.saturating_sub(pre_sim_coinbase_balance);
        let eth_sent_to_coinbase = coinbase_diff.saturating_sub(gas_fees);

        Ok((
            EthCallBundleTransactionResult {
                tx_hash: *tx.tx_hash(),
                coinbase_diff,
                eth_sent_to_coinbase,
                from_address,
                to_address: match tx.kind() {
                    TxKind::Call(to) => Some(to),
                    _ => Some(Address::ZERO),
                },
                value,
                revert,
                gas_used,
                gas_price,
                gas_fees,
            },
            post_sim_coinbase_balance,
        ))
    }
}

// [`BundleDriver`] Implementation for [`SignetCallBundle`].
// This is useful mainly for the `signet_simBundle` endpoint,
// which is used to simulate a signet bundle while respecting market context.
impl<I> BundleDriver<OrderDetector<I>>
    for SignetBundleDriver<SignetCallBundle, SignetCallBundleResponse>
{
    type Error<Db: Database + DatabaseCommit> = SignetBundleError<Db>;

    fn run_bundle<'a, Db: Database + DatabaseCommit>(
        &mut self,
        trevm: trevm::EvmNeedsTx<'a, OrderDetector<I>, Db>,
    ) -> trevm::DriveBundleResult<'a, OrderDetector<I>, Db, Self> {
        // Check if the block we're in is valid for this bundle. Both must match
        trevm_ensure!(
            trevm.inner().block().number.to::<u64>() == self.bundle.bundle.block_number,
            trevm,
            BundleError::BlockNumberMismatch.into()
        );

        // Check if the bundle has any transactions
        trevm_ensure!(
            !self.bundle.bundle.txs.is_empty(),
            trevm,
            BundleError::BundleEmpty.into()
        );

        // Check if the state block number is valid (not 0, and not a tag)
        trevm_ensure!(
            self.bundle.bundle.state_block_number.is_number()
                && self
                    .bundle
                    .bundle
                    .state_block_number
                    .as_number()
                    .unwrap_or(0)
                    != 0,
            trevm,
            BundleError::BlockNumberMismatch.into()
        );

        // Load the host fills into the context for simulation
        for fill in self.bundle.host_fills.iter() {
            self.context.add_fill(
                self.host_chain_id,
                &HostOrders::Filled {
                    outputs: fill
                        .1
                        .iter()
                        .map(|(user, amount)| Output {
                            token: *fill.0,
                            recipient: *user,
                            amount: *amount,
                            chainId: self.host_chain_id as u32,
                        })
                        .collect(),
                },
            )
        }

        // Set the state block number this simulation was based on
        self.response.response.state_block_number = self
            .bundle
            .bundle
            .state_block_number
            .as_number()
            .unwrap_or(trevm.inner().block().number.to::<u64>());

        let run_result = trevm.try_with_block(&self.bundle, |mut trevm| {

            // Decode and validate the transactions in the bundle
            let txs =
                unwrap_or_trevm_err!(Self::decode_and_validate_txs(&self.bundle.bundle.txs), trevm);

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

                match run_result {
                    // return immediately if errored
                    Err(e) => {
                        return Err(e.map_err(|e| {
                            SignetBundleError::BundleError(e.into())
                        }));
                    }
                    // Accept + accumulate state
                    Ok(mut res) => {

                        // Check & respect market context. This MUST be done after every transaction before accepting the state.
                        let (aggregate, mkt_ctx) =
                            res.inner_mut_unchecked().context.external.take_aggregate();
                        if let Err(err) = self.context.checked_remove_ru_tx_events(&mkt_ctx, &aggregate) {
                            tracing::debug!(%err, "Discarding bundle simulation due to market error");
                            return Err(res.errored(SignetBundleError::MarketError(err)));
                        }

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

                        // Process the transaction and accumulate the results
                        let (response, post_sim_coinbase_balance) = unwrap_or_trevm_err!(
                            Self::process_call_bundle_tx(
                                tx,
                                pre_sim_coinbase_balance,
                                post_sim_coinbase_balance,
                                basefee,
                                execution_result
                            ),
                            committed_trevm
                        );

                        // Accumulate overall results from response
                        self.response.response.total_gas_used += response.gas_used;
                        self.response.response.gas_fees += response.gas_fees;
                        self.response.response.results.push(response);

                        // update the coinbase balance
                        pre_sim_coinbase_balance = post_sim_coinbase_balance;

                        // Set the trevm instance to the committed one
                        trevm = committed_trevm;
                    }
                }
            }

            // Accumulate the total results
            self.response.response.coinbase_diff =
                post_sim_coinbase_balance.saturating_sub(initial_coinbase_balance);
            self.response.response.eth_sent_to_coinbase =
                self.response.response.coinbase_diff.saturating_sub(self.response.response.gas_fees);
            self.response.response.bundle_gas_price = self
                .response
                .response
                .coinbase_diff
                .checked_div(U256::from(self.response.response.total_gas_used))
                .unwrap_or_default();
            self.response.response.bundle_hash = self.bundle.bundle_hash();

            // return the final state
            Ok(trevm)
        });

        run_result
    }

    fn post_bundle<Db: Database + DatabaseCommit>(
        &mut self,
        _trevm: &trevm::EvmNeedsTx<'_, OrderDetector<I>, Db>,
    ) -> Result<(), Self::Error<Db>> {
        Ok(())
    }
}
