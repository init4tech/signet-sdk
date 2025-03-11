use crate::{
    eth::{EthError, TxCacheForwarder},
    interest::{ActiveFilter, FilterManager, FilterOutput, SubscriptionManager},
    util::BlockRangeInclusiveIter,
    Pnt,
};
use alloy::{
    consensus::{BlockHeader, Header, Signed, Transaction, TxEnvelope},
    eips::{BlockId, BlockNumberOrTag, NumHash},
    network::Ethereum,
    primitives::{B256, U64},
    rpc::types::{FeeHistory, Filter, Log},
};
use reth::{
    core::primitives::SignedTransaction,
    primitives::{Block, EthPrimitives, Receipt, Recovered, RecoveredBlock, TransactionSigned},
    providers::{
        providers::{BlockchainProvider, ProviderNodeTypes},
        BlockHashReader, BlockIdReader, BlockNumReader, CanonStateSubscriptions, HeaderProvider,
        ProviderBlock, ProviderError, ProviderReceipt, ReceiptProvider, StateProviderFactory,
        TransactionsProvider,
    },
    revm::database::StateProviderDatabase,
    rpc::{
        compat::block::from_block,
        eth::{filter::EthFilterError, EthTxBuilder},
        server_types::eth::{
            fee_history::{
                calculate_reward_percentiles_for_block, fee_history_cache_new_blocks_task,
            },
            logs_utils::{self, append_matching_block_logs, ProviderOrBlock},
            EthApiError, EthConfig, EthReceiptBuilder, EthStateCache, FeeHistoryCache,
            FeeHistoryEntry, GasPriceOracle,
        },
        types::{FilterBlockOption, FilteredParams},
    },
    tasks::{TaskExecutor, TaskSpawner},
};
use reth_chainspec::{BaseFeeParams, ChainSpec, ChainSpecProvider};
use reth_node_api::{BlockBody, FullNodeComponents};
use reth_rpc_eth_api::{RpcBlock, RpcReceipt, RpcTransaction};
use signet_evm::{EvmNeedsTx, RuRevmState};
use signet_types::{config::SignetSystemConstants, MagicSig};
use std::{marker::PhantomData, sync::Arc};
use tracing::{instrument, trace, Level};
use trevm::{
    revm::{
        primitives::{AnalysisKind, CfgEnv},
        StateBuilder,
    },
    Cfg,
};

/// The maximum number of headers we read at once when handling a range filter.
const MAX_HEADERS_RANGE: u64 = 1_000; // with ~530bytes per header this is ~500kb

/// RPC context. Contains all necessary host and signet components for serving
/// RPC requests.
#[derive(Debug)]
pub struct RpcCtx<Host, Signet>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    inner: Arc<RpcCtxInner<Host, Signet>>,
}

impl<Host, Signet> RpcCtx<Host, Signet>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    /// Create a new `RpcCtx`.
    pub fn new<Tasks>(
        host: Host,
        constants: SignetSystemConstants,
        provider: BlockchainProvider<Signet>,
        eth_config: EthConfig,
        forwarder: Option<TxCacheForwarder>,
        spawner: Tasks,
    ) -> Self
    where
        Tasks: TaskSpawner + Clone + 'static,
    {
        let inner = RpcCtxInner::new(host, constants, provider, eth_config, forwarder, spawner);

        Self { inner: Arc::new(inner) }
    }
}

impl<Host, Signet> Clone for RpcCtx<Host, Signet>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl<Host, Signet> core::ops::Deref for RpcCtx<Host, Signet>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    type Target = RpcCtxInner<Host, Signet>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// Inner context for [`RpcCtx`].
#[derive(Debug)]
pub struct RpcCtxInner<Host, Signet>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    host: Host,
    signet: SignetCtx<Signet>,
}

impl<Host, Signet> RpcCtxInner<Host, Signet>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    /// Create a new `RpcCtxInner`.
    pub fn new<Tasks>(
        host: Host,
        constants: SignetSystemConstants,
        provider: BlockchainProvider<Signet>,
        eth_config: EthConfig,
        forwarder: Option<TxCacheForwarder>,
        spawner: Tasks,
    ) -> Self
    where
        Tasks: TaskSpawner + Clone + 'static,
    {
        let signet = SignetCtx::new(constants, provider, eth_config, forwarder, spawner);
        Self { host, signet }
    }

    pub const fn host(&self) -> &Host {
        &self.host
    }

    pub const fn signet(&self) -> &SignetCtx<Signet> {
        &self.signet
    }

    pub fn task_executor(&self) -> &TaskExecutor {
        self.host.task_executor()
    }

    /// Create a trevm instance.
    pub fn trevm(
        &self,
        block_id: BlockId,
        block: &Header,
    ) -> Result<EvmNeedsTx<'_, RuRevmState>, EthApiError> {
        // decrement if the id is pending, so that the state is on the latest block
        let height = block.number() - block_id.is_pending() as u64;
        let spec_id = self.signet.evm_spec_id(block);

        let db = self.signet.state_provider_database(height)?;

        let mut trevm = signet_evm::signet_evm(
            db,
            self.signet.constants.ru_orders(),
            self.signet.constants.ru_chain_id(),
        )
        .fill_cfg(&self.signet)
        .fill_block(block);

        trevm.set_spec_id(spec_id);

        Ok(trevm)
    }
}

/// Signet context. This struct contains all the necessary components for
/// accessing Signet node state, and serving RPC requests.
#[derive(Debug)]
pub struct SignetCtx<Inner>
where
    Inner: Pnt,
{
    // Basics
    constants: SignetSystemConstants,
    eth_config: EthConfig,

    // State stuff
    provider: BlockchainProvider<Inner>,
    cache: EthStateCache<
        ProviderBlock<BlockchainProvider<Inner>>,
        ProviderReceipt<BlockchainProvider<Inner>>,
    >,

    // Gas stuff
    gas_oracle: GasPriceOracle<BlockchainProvider<Inner>>,
    fee_history: FeeHistoryCache,

    // Tx stuff
    forwarder: Option<TxCacheForwarder>,

    // Filter and subscription stuff
    filters: FilterManager,
    subs: SubscriptionManager<Inner>,

    // Spooky ghost stuff
    _pd: std::marker::PhantomData<fn() -> Inner>,
}

impl<Inner> SignetCtx<Inner>
where
    Inner: ProviderNodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives>,
{
    /// Instantiate a new `SignetCtx`, spawning necessary tasks to keep the
    /// relevant caches up to date.
    pub fn new<Tasks>(
        constants: SignetSystemConstants,
        provider: BlockchainProvider<Inner>,
        eth_config: EthConfig,
        forwarder: Option<TxCacheForwarder>,
        spawner: Tasks,
    ) -> Self
    where
        Tasks: TaskSpawner + Clone + 'static,
    {
        let cache = EthStateCache::spawn_with(provider.clone(), eth_config.cache, spawner.clone());
        let gas_oracle =
            GasPriceOracle::new(provider.clone(), eth_config.gas_oracle, cache.clone());
        let fee_history = FeeHistoryCache::new(eth_config.fee_history_cache);

        let fee_task = fee_history_cache_new_blocks_task(
            fee_history.clone(),
            provider.canonical_state_stream(),
            provider.clone(),
            cache.clone(),
        );

        spawner.spawn_critical("fee_history_cache_new_blocks", Box::pin(fee_task));

        let filters = FilterManager::new(eth_config.stale_filter_ttl, eth_config.stale_filter_ttl);

        let subs = SubscriptionManager::new(provider.clone(), eth_config.stale_filter_ttl);

        Self {
            constants,
            provider,
            eth_config,
            cache,
            gas_oracle,
            fee_history,
            forwarder,
            filters,
            subs,
            _pd: PhantomData,
        }
    }

    /// Access the signet constants
    pub const fn constants(&self) -> SignetSystemConstants {
        self.constants
    }

    /// Access the signet DB
    pub const fn provider(&self) -> &BlockchainProvider<Inner> {
        &self.provider
    }

    /// Access the signet [`EthConfig`]
    pub const fn config(&self) -> &EthConfig {
        &self.eth_config
    }

    /// Access the forwarder
    pub fn forwarder(&self) -> Option<TxCacheForwarder> {
        self.forwarder.clone()
    }

    /// Access the [`ChainSpec`].
    pub fn chain_spec(&self) -> Arc<ChainSpec> {
        self.provider.chain_spec()
    }

    /// Get the EVM spec ID for a given block.
    pub fn evm_spec_id(&self, header: &Header) -> trevm::revm::primitives::SpecId {
        reth_evm_ethereum::revm_spec(&self.chain_spec(), header)
    }

    /// Access the subscription manager.
    pub const fn subscriptions(&self) -> &SubscriptionManager<Inner> {
        &self.subs
    }

    /// Make a [`StateProviderDatabase`] from the read-write provider, suitable
    /// for use with Trevm.
    fn state_provider_database(&self, height: u64) -> Result<RuRevmState, EthApiError> {
        // Get the state provider for the block number
        let sp = self.provider.history_by_block_number(height)?;

        // Wrap in Revm compatibility layer
        let spd = StateProviderDatabase::new(sp);

        let builder = StateBuilder::new_with_database(spd);

        Ok(builder.build())
    }

    /// Get the block for a given block, returning the block hash and
    /// the block itself.
    pub async fn raw_block(
        &self,
        t: impl Into<BlockId>,
    ) -> Result<Option<(B256, Arc<RecoveredBlock<Block>>)>, EthApiError> {
        let Some(hash) = self.provider.block_hash_for_id(t.into())? else {
            return Ok(None);
        };

        self.cache
            .get_sealed_block_with_senders(hash)
            .await
            .map_err(Into::into)
            .map(|b| b.map(|b| (hash, b)))
    }

    /// Get the block for a given block, formatting the block for
    /// the RPC API.
    pub async fn block(
        &self,
        t: impl Into<BlockId>,
        full: Option<bool>,
    ) -> Result<Option<RpcBlock<Ethereum>>, EthApiError> {
        let Some(hash) = self.provider.block_hash_for_id(t.into())? else {
            return Ok(None);
        };

        let Some(block) = self.cache.get_sealed_block_with_senders(hash).await? else {
            return Ok(None);
        };

        from_block((*block).clone(), full.unwrap_or_default().into(), &EthTxBuilder::default())
            .map(Some)
    }

    /// Get the tx count for a given block.
    pub async fn tx_count(&self, t: impl Into<BlockId>) -> Result<Option<U64>, EthApiError> {
        let Some(hash) = self.provider.block_hash_for_id(t.into())? else {
            return Ok(None);
        };

        if let Some(block) = self.cache.get_sealed_block_with_senders(hash).await? {
            // ambiguous function names
            let txns = BlockBody::transactions(block.body());
            Ok(Some(U64::from(txns.len())))
        } else {
            Ok(None)
        }
    }

    /// Get the receipts for a given block.
    pub async fn raw_receipts(
        &self,
        t: impl Into<BlockId>,
    ) -> Result<Option<Arc<Vec<Receipt>>>, EthApiError> {
        let Some(hash) = self.provider.block_hash_for_id(t.into())? else {
            return Ok(None);
        };

        self.cache.get_receipts(hash).await.map_err(Into::into)
    }

    /// Get the transaction for a given hash, returning the transaction and its
    /// block-related metadata.
    pub fn raw_transaction_by_hash(
        &self,
        h: B256,
    ) -> Result<Option<(TransactionSigned, reth::primitives::TransactionMeta)>, EthApiError> {
        self.provider.transaction_by_hash_with_meta(h).map_err(Into::into)
    }

    /// Format a transaction for the RPC API.
    fn format_rpc_tx(
        tx: TransactionSigned,
        block_hash: B256,
        block_number: u64,
        index: usize,
        base_fee: Option<u64>,
    ) -> Result<alloy::rpc::types::Transaction, EthApiError> {
        let sig = tx.signature();

        let sender = if let Some(sender) = MagicSig::try_from_signature(sig).map(|s| s.sender()) {
            sender
        } else {
            tx.recover_signer().map_err(|_| EthApiError::InvalidTransactionSignature)?
        };

        let tx = Recovered::new_unchecked(tx, sender);

        let from = tx.signer();
        let hash = *tx.hash();
        let signature = *tx.signature();

        let inner: TxEnvelope = match tx.into_tx().into_transaction() {
            reth::primitives::Transaction::Legacy(tx) => {
                Signed::new_unchecked(tx, signature, hash).into()
            }
            reth::primitives::Transaction::Eip2930(tx) => {
                Signed::new_unchecked(tx, signature, hash).into()
            }
            reth::primitives::Transaction::Eip1559(tx) => {
                Signed::new_unchecked(tx, signature, hash).into()
            }
            reth::primitives::Transaction::Eip4844(tx) => {
                Signed::new_unchecked(tx, signature, hash).into()
            }
            reth::primitives::Transaction::Eip7702(tx) => {
                Signed::new_unchecked(tx, signature, hash).into()
            }
        };

        let egp = base_fee
            .map(|base_fee| {
                inner.effective_tip_per_gas(base_fee).unwrap_or_default() as u64 + base_fee
            })
            .unwrap_or_else(|| inner.max_fee_per_gas() as u64);

        Ok(alloy::rpc::types::Transaction {
            inner,
            block_hash: Some(block_hash),
            block_number: Some(block_number),
            transaction_index: Some(index as u64),
            from,
            effective_gas_price: Some(egp as u128),
        })
    }

    /// Get a transaction by its hash, and format it for the RPC API.
    pub fn rpc_transaction_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<RpcTransaction<Ethereum>>, EthApiError> {
        let Some((tx, meta)) = self.raw_transaction_by_hash(hash)? else {
            return Ok(None);
        };

        Self::format_rpc_tx(
            tx,
            meta.block_hash,
            meta.block_number,
            meta.index as usize,
            meta.base_fee,
        )
        .map(Some)
    }

    /// Get a transaction by its block and index, and format it for the RPC API.
    pub async fn rpc_transaction_by_block_idx(
        &self,
        id: impl Into<BlockId>,
        index: usize,
    ) -> Result<Option<RpcTransaction<Ethereum>>, EthApiError> {
        let Some((hash, block)) = self.raw_block(id).await? else {
            return Ok(None);
        };

        block
            .body()
            .transactions
            .get(index)
            .map(|tx| {
                Self::format_rpc_tx(
                    tx.clone(),
                    hash,
                    block.number(),
                    index,
                    block.base_fee_per_gas(),
                )
            })
            .transpose()
    }

    /// Get a receipt by its hash, and format it for the RPC API.
    pub async fn rpc_receipt_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<RpcReceipt<Ethereum>>, EthApiError> {
        let Some((tx, meta)) = self.raw_transaction_by_hash(hash)? else {
            trace!(%hash, "Transaction not found for receipt hash");
            return Ok(None);
        };

        let Some(receipt) = self.provider.receipt_by_hash(hash)? else {
            trace!(%hash, "Receipt not found for transaction hash");
            return Ok(None);
        };

        let Some(all_receipts) = self.cache.get_receipts(meta.block_hash).await? else {
            trace!(%hash, "Block not found for transaction hash");
            return Ok(None);
        };

        // last arg is blobparams, which we don't have
        EthReceiptBuilder::new(&tx, meta, &receipt, &all_receipts, None)
            .map(EthReceiptBuilder::build)
            .map(Some)
    }

    /// Create the [`Block`] object for a specific [`BlockId`].
    pub async fn block_cfg(&self, mut block_id: BlockId) -> Result<Header, EthApiError> {
        // If the block is pending, we'll load the latest and
        let pending = block_id.is_pending();
        if pending {
            block_id = BlockId::latest();
        }

        let Some((_, block)) = self.raw_block(block_id).await? else {
            return Err(EthApiError::HeaderNotFound(block_id));
        };

        let mut header = block.clone_header();

        // Modify the header for pending blocks, to simulate the next block.
        if pending {
            header.parent_hash = header.hash_slow();
            header.number += 1;
            header.timestamp += 12;
            header.base_fee_per_gas = header.next_block_base_fee(BaseFeeParams::ethereum());
            header.gas_limit = self.eth_config.rpc_gas_cap;
        }

        Ok(header)
    }

    /// Create a gas price oracle.
    pub const fn gas_oracle(&self) -> &GasPriceOracle<BlockchainProvider<Inner>> {
        &self.gas_oracle
    }

    /// Approximates reward at a given percentile for a specific block
    /// Based on the configured resolution
    ///
    /// Implementation reproduced from reth.
    fn approximate_percentile(&self, entry: &FeeHistoryEntry, requested_percentile: f64) -> u128 {
        let resolution = self.fee_history.resolution();
        let rounded_percentile =
            (requested_percentile * resolution as f64).round() / resolution as f64;
        let clamped_percentile = rounded_percentile.clamp(0.0, 100.0);

        // Calculate the index in the precomputed rewards array
        let index = (clamped_percentile / (1.0 / resolution as f64)).round() as usize;
        // Fetch the reward from the FeeHistoryEntry
        entry.rewards.get(index).copied().unwrap_or_default()
    }

    /// Implements the `eth_feeHistory` RPC method.
    ///
    /// Implementation reproduced from reth, trimmed of 4844 support.
    pub async fn fee_history(
        &self,
        mut block_count: u64,
        mut newest: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> Result<FeeHistory, EthApiError> {
        if block_count == 0 {
            return Ok(FeeHistory::default());
        }

        // See https://github.com/ethereum/go-ethereum/blob/2754b197c935ee63101cbbca2752338246384fec/eth/gasprice/feehistory.go#L218C8-L225
        let max_fee_history = if reward_percentiles.is_none() {
            self.gas_oracle().config().max_header_history
        } else {
            self.gas_oracle().config().max_block_history
        };

        if block_count > max_fee_history {
            block_count = max_fee_history
        }

        if newest.is_pending() {
            // cap the target block since we don't have fee history for the pending block
            newest = BlockNumberOrTag::Latest;
            // account for missing pending block
            block_count = block_count.saturating_sub(1);
        }

        let end_block = self
            .provider()
            .block_number_for_id(newest.into())?
            .ok_or(EthApiError::HeaderNotFound(newest.into()))?;

        // need to add 1 to the end block to get the correct (inclusive) range
        let end_block_plus = end_block + 1;
        // Ensure that we would not be querying outside of genesis
        if end_block_plus < block_count {
            block_count = end_block_plus;
        }

        // If reward percentiles were specified, we
        // need to validate that they are monotonically
        // increasing and 0 <= p <= 100
        // Note: The types used ensure that the percentiles are never < 0
        if let Some(percentiles) = &reward_percentiles {
            if percentiles.windows(2).any(|w| w[0] > w[1] || w[0] > 100.) {
                return Err(EthApiError::InvalidRewardPercentiles);
            }
        }

        // Fetch the headers and ensure we got all of them
        //
        // Treat a request for 1 block as a request for `newest_block..=newest_block`,
        // otherwise `newest_block - 2
        // NOTE: We ensured that block count is capped
        let start_block = end_block_plus - block_count;

        // Collect base fees, gas usage ratios and (optionally) reward percentile data
        let mut base_fee_per_gas: Vec<u128> = Vec::new();
        let mut gas_used_ratio: Vec<f64> = Vec::new();

        let mut rewards: Vec<Vec<u128>> = Vec::new();

        // Check if the requested range is within the cache bounds
        let fee_entries = self.fee_history.get_history(start_block, end_block).await;

        if let Some(fee_entries) = fee_entries {
            if fee_entries.len() != block_count as usize {
                return Err(EthApiError::InvalidBlockRange);
            }

            for entry in &fee_entries {
                base_fee_per_gas.push(entry.base_fee_per_gas as u128);
                gas_used_ratio.push(entry.gas_used_ratio);

                if let Some(percentiles) = &reward_percentiles {
                    let mut block_rewards = Vec::with_capacity(percentiles.len());
                    for &percentile in percentiles {
                        block_rewards.push(self.approximate_percentile(entry, percentile));
                    }
                    rewards.push(block_rewards);
                }
            }
            let last_entry = fee_entries.last().expect("is not empty");

            // Also need to include the `base_fee_per_gas` and `base_fee_per_blob_gas` for the
            // next block
            base_fee_per_gas
                .push(last_entry.next_block_base_fee(self.provider().chain_spec()) as u128);
        } else {
            // read the requested header range
            let headers = self.provider().sealed_headers_range(start_block..=end_block)?;
            if headers.len() != block_count as usize {
                return Err(EthApiError::InvalidBlockRange);
            }

            for header in &headers {
                base_fee_per_gas.push(header.base_fee_per_gas().unwrap_or_default() as u128);
                gas_used_ratio.push(header.gas_used() as f64 / header.gas_limit() as f64);

                // Percentiles were specified, so we need to collect reward percentile ino
                if let Some(percentiles) = &reward_percentiles {
                    let (block, receipts) = self
                        .cache
                        .get_block_and_receipts(header.hash())
                        .await?
                        .ok_or(EthApiError::InvalidBlockRange)?;
                    rewards.push(
                        calculate_reward_percentiles_for_block(
                            percentiles,
                            header.gas_used(),
                            header.base_fee_per_gas().unwrap_or_default(),
                            &block.body().transactions,
                            &receipts,
                        )
                        .unwrap_or_default(),
                    );
                }
            }

            // The spec states that `base_fee_per_gas` "[..] includes the next block after the
            // newest of the returned range, because this value can be derived from the
            // newest block"
            //
            // The unwrap is safe since we checked earlier that we got at least 1 header.
            let last_header = headers.last().expect("is present");
            base_fee_per_gas.push(
                last_header
                    .next_block_base_fee(
                        self.provider()
                            .chain_spec()
                            .base_fee_params_at_timestamp(last_header.timestamp()),
                    )
                    .unwrap_or_default() as u128,
            );
        };

        let base_fee_per_blob_gas = vec![0; base_fee_per_gas.len()];
        let blob_gas_used_ratio = vec![0.; gas_used_ratio.len()];

        Ok(FeeHistory {
            base_fee_per_gas,
            gas_used_ratio,
            base_fee_per_blob_gas,
            blob_gas_used_ratio,
            oldest_block: start_block,
            reward: reward_percentiles.map(|_| rewards),
        })
    }

    /// Get logs for a given block hash based on a filter
    ///
    /// ## Panics
    ///
    /// Panics if the filter is a range filter
    async fn logs_at_hash(&self, filter: &Filter) -> Result<Vec<Log>, EthApiError> {
        let hash = *filter.block_option.as_block_hash().expect("COU");

        let (block, receipts) = tokio::try_join!(self.raw_block(hash), self.raw_receipts(hash),)?;

        // Return an error if the block isn't found
        let (_, block) = block.ok_or(EthApiError::HeaderNotFound(hash.into()))?;
        // Return an error if the receipts aren't found
        let receipts = receipts.ok_or(EthApiError::HeaderNotFound(hash.into()))?;

        let block_num_hash = NumHash::new(block.number(), hash);
        let timestamp = block.timestamp();

        let mut all_logs = Vec::new();
        append_matching_block_logs(
            &mut all_logs,
            ProviderOrBlock::<BlockchainProvider<Inner>>::Block(block),
            &FilteredParams::new(Some(filter.clone())),
            block_num_hash,
            &receipts,
            false,
            timestamp,
        )?;

        Ok(all_logs)
    }

    /// Returns all logs in the given _inclusive_ range that match the filter
    ///
    /// Returns an error if:
    ///  - underlying database error
    ///  - amount of matches exceeds configured limit
    async fn get_logs_in_block_range(
        &self,
        filter: &Filter,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<Log>, EthFilterError> {
        trace!(target: "rpc::eth::filter", from=from_block, to=to_block, ?filter, "finding logs in range");

        if to_block < from_block {
            return Err(EthFilterError::InvalidBlockRangeParams);
        }
        let max_blocks = self.config().max_blocks_per_filter;

        if to_block - from_block > max_blocks {
            return Err(EthFilterError::QueryExceedsMaxBlocks(max_blocks));
        }

        let mut all_logs = Vec::new();
        let filter_params = FilteredParams::new(Some(filter.clone()));

        // derive bloom filters from filter input, so we can check headers for matching logs
        let address_filter = FilteredParams::address_filter(&filter.address);
        let topics_filter = FilteredParams::topics_filter(&filter.topics);

        // loop over the range of new blocks and check logs if the filter matches the log's bloom
        // filter
        for (from, to) in BlockRangeInclusiveIter::new(from_block..=to_block, MAX_HEADERS_RANGE) {
            let headers = self.provider().headers_range(from..=to)?;

            for (idx, header) in headers.iter().enumerate() {
                // only if filter matches
                if FilteredParams::matches_address(header.logs_bloom(), &address_filter)
                    && FilteredParams::matches_topics(header.logs_bloom(), &topics_filter)
                {
                    // these are consecutive headers, so we can use the parent hash of the next
                    // block to get the current header's hash
                    let hash = match headers.get(idx + 1) {
                        Some(parent) => parent.parent_hash(),
                        None => self
                            .provider()
                            .block_hash(header.number())?
                            .ok_or_else(|| ProviderError::HeaderNotFound(header.number().into()))?,
                    };

                    let (block, receipts) =
                        tokio::try_join!(self.raw_block(hash), self.raw_receipts(hash),)?;

                    // Return an error if the block isn't found
                    let (_, block) = block.ok_or(EthApiError::HeaderNotFound(hash.into()))?;
                    // Return an error if the receipts aren't found
                    let receipts = receipts.ok_or(EthApiError::HeaderNotFound(hash.into()))?;

                    let block_num_hash = NumHash::new(block.number(), hash);
                    let timestamp = block.timestamp();

                    append_matching_block_logs(
                        &mut all_logs,
                        ProviderOrBlock::<BlockchainProvider<Inner>>::Block(block),
                        &filter_params,
                        block_num_hash,
                        &receipts,
                        false,
                        timestamp,
                    )?;

                    // size check but only if range is multiple blocks, so we always return all
                    // logs of a single block
                    let max_logs = self.config().max_logs_per_response;
                    let is_multi_block_range = from_block != to_block;
                    if is_multi_block_range && all_logs.len() > max_logs {
                        return Err(EthFilterError::QueryExceedsMaxResults {
                            max_logs,
                            from_block,
                            to_block: block_num_hash.number.saturating_sub(1),
                        });
                    }
                }
            }
        }

        Ok(all_logs)
    }

    /// Get logs for a given block range based on a filter
    ///
    /// ## Panics
    ///
    /// Panics if the filter is not a range filter
    async fn logs_in_range(&self, filter: &Filter) -> Result<Vec<Log>, EthFilterError> {
        // compute the range
        let (from_block, to_block) = filter.block_option.as_range();

        let info = self.provider().chain_info()?;

        // we start at the most recent block if unset in filter
        let start_block = info.best_number;
        let from =
            from_block.map(|num| self.provider().convert_block_number(*num)).transpose()?.flatten();
        let to =
            to_block.map(|num| self.provider().convert_block_number(*num)).transpose()?.flatten();
        let (from_block_number, to_block_number) =
            logs_utils::get_filter_block_range(from, to, start_block, info);
        self.get_logs_in_block_range(filter, from_block_number, to_block_number).await
    }

    /// Logic for `eth_getLogs` RPC method.
    pub async fn logs(&self, filter: &Filter) -> Result<Vec<Log>, EthError> {
        if filter.block_option.is_range() {
            self.logs_in_range(filter).await.map_err(Into::into)
        } else {
            self.logs_at_hash(filter).await.map_err(Into::into)
        }
    }

    /// Install a log filter.
    pub fn install_log_filter(&self, filter: Filter) -> Result<U64, EthError> {
        let chain_info = self.provider().chain_info()?;

        Ok(self.filters.install_log_filter(chain_info.best_number, filter))
    }

    /// Install a block filter.
    pub fn install_block_filter(&self) -> Result<U64, EthError> {
        let chain_info = self.provider().chain_info()?;

        Ok(self.filters.install_block_filter(chain_info.best_number))
    }

    /// Poll an active log filter for changes.
    ///
    /// # Panics
    ///
    /// Panics if the filter is not a Log filter
    #[instrument(level = Level::DEBUG, skip_all, fields(since_last_poll = filter.time_since_last_poll().as_millis(), next_start_block = filter.next_start_block()))]
    async fn get_log_filter_changes(
        &self,
        filter: &ActiveFilter,
    ) -> Result<(u64, FilterOutput), EthError> {
        debug_assert!(filter.is_filter());

        // Load the current tip
        let info = self.provider().chain_info()?;
        let current_height = info.best_number;

        trace!(%filter, current_height, "Polling filter");

        // If the filter was polled AFTER the current tip, we return an empty
        // result
        let start_block = filter.next_start_block();
        if start_block > current_height {
            return Ok((current_height, FilterOutput::empty()));
        }

        // Cast to a filter (this is checked by dbg_assert and by the caller)
        let filter = filter.as_filter().unwrap();

        let (from_block_number, to_block_number) = match filter.block_option {
            FilterBlockOption::Range { from_block, to_block } => {
                let from = from_block
                    .map(|num| self.provider().convert_block_number(num))
                    .transpose()?
                    .flatten();
                let to = to_block
                    .map(|num| self.provider().convert_block_number(num))
                    .transpose()?
                    .flatten();
                logs_utils::get_filter_block_range(from, to, start_block, info)
            }
            FilterBlockOption::AtBlockHash(_) => {
                // blockHash is equivalent to fromBlock = toBlock = the block number with
                // hash blockHash
                // get_logs_in_block_range is inclusive
                (start_block, current_height)
            }
        };
        let logs = self.get_logs_in_block_range(filter, from_block_number, to_block_number).await?;

        Ok((to_block_number, logs.into()))
    }

    #[instrument(level = Level::DEBUG, skip_all, fields(since_last_poll = filter.time_since_last_poll().as_millis(), next_start_block = filter.next_start_block()))]
    async fn get_block_filter_changes(
        &self,
        filter: &ActiveFilter,
    ) -> Result<(u64, FilterOutput), EthError> {
        debug_assert!(filter.is_block());
        // Get the current tip number
        let info = self.provider().chain_info()?;
        let current_height = info.best_number;

        trace!(%filter, current_height, "Polling filter");

        let start_block = filter.next_start_block();
        if start_block > current_height {
            return Ok((current_height, FilterOutput::empty()));
        }

        // Note: we need to fetch the block hashes from inclusive range
        // [start_block..best_block]
        let end_block = current_height + 1;

        let block_hashes = self
            .provider()
            .canonical_hashes_range(start_block, end_block)
            .map_err(|_| EthApiError::HeaderRangeNotFound(start_block.into(), end_block.into()))?;
        Ok((current_height, block_hashes.into()))
    }

    /// Get the changes for a filter
    #[instrument(level = Level::DEBUG, skip(self))]
    pub async fn filter_changes(&self, id: U64) -> Result<FilterOutput, EthError> {
        let mut ref_mut = self
            .filters
            .get_mut(id)
            .ok_or_else(|| EthFilterError::FilterNotFound(id.saturating_to::<u64>().into()))?;
        let filter = ref_mut.value_mut();

        let (polled_to_block, res) = if filter.is_block() {
            self.get_block_filter_changes(filter).await?
        } else {
            self.get_log_filter_changes(filter).await?
        };
        filter.mark_polled(polled_to_block);

        trace!(%filter, "Marked polled");
        Ok(res)
    }

    /// Uninstall a filter.
    pub fn uninstall_filter(&self, id: U64) -> bool {
        self.filters.uninstall(id).is_some()
    }
}

impl<Inner> Cfg for SignetCtx<Inner>
where
    Inner: Pnt,
{
    fn fill_cfg_env(&self, cfg_env: &mut CfgEnv) {
        let CfgEnv { chain_id, perf_analyse_created_bytecodes, .. } = cfg_env;
        *chain_id = self.constants.ru_chain_id();
        *perf_analyse_created_bytecodes = AnalysisKind::Raw;
    }
}

// Some code in this file has been copied and modified from reth
// <https://github.com/paradigmxyz/reth>
// The original license is included below:
//
// The MIT License (MIT)
//
// Copyright (c) 2022-2025 Reth Contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//.
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
