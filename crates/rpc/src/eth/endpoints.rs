use crate::{
    ctx::RpcCtx,
    eth::{CallErrorData, EthError},
    interest::{FilterOutput, InterestKind},
    receipts::build_signet_receipt,
    util::{await_jh_option, await_jh_option_response, response_tri},
    Pnt,
};
use ajj::{HandlerCtx, ResponsePayload};
use alloy::{
    consensus::{BlockHeader, TxEnvelope},
    eips::{
        eip2718::{Decodable2718, Encodable2718},
        BlockId, BlockNumberOrTag,
    },
    network::Ethereum,
    primitives::{Address, B256, U256, U64},
    rpc::types::{
        pubsub::SubscriptionKind, state::StateOverride, BlockOverrides, Filter, TransactionRequest,
    },
};
use reth::{
    network::NetworkInfo,
    primitives::TransactionMeta,
    providers::{BlockNumReader, StateProviderFactory, TransactionsProvider},
};
use reth_node_api::FullNodeComponents;
use reth_rpc_eth_api::{RpcBlock, RpcHeader, RpcReceipt, RpcTransaction};
use serde::Deserialize;
use signet_evm::EvmErrored;
use std::borrow::Cow;
use tracing::{trace_span, Instrument};
use trevm::revm::context::result::ExecutionResult;

/// Args for `eth_estimateGas` and `eth_call`.
#[derive(Debug, Deserialize)]
pub(super) struct TxParams(
    TransactionRequest,
    #[serde(default)] Option<BlockId>,
    #[serde(default)] Option<StateOverride>,
    #[serde(default)] Option<Box<BlockOverrides>>,
);

/// Args for `eth_getBlockByHash` and `eth_getBlockByNumber`.
#[derive(Debug, Deserialize)]
pub(super) struct BlockParams<T>(T, #[serde(default)] Option<bool>);

/// Args for `eth_feeHistory`.
#[derive(Debug, Deserialize)]
pub(super) struct FeeHistoryArgs(U64, BlockNumberOrTag, #[serde(default)] Option<Vec<f64>>);

/// Args for `eth_getStorageAt`.
#[derive(Debug, Deserialize)]
pub(super) struct StorageAtArgs(Address, U256, #[serde(default)] Option<BlockId>);

/// Args for `eth_getBalance`, `eth_getTransactionCount`, and `eth_getCode`.
#[derive(Debug, Deserialize)]
pub(super) struct AddrWithBlock(Address, #[serde(default)] Option<BlockId>);

/// Args for `eth_subscribe`.
#[derive(Debug, Deserialize)]
pub struct SubscribeArgs(pub SubscriptionKind, #[serde(default)] pub Option<Box<Filter>>);

impl TryFrom<SubscribeArgs> for InterestKind {
    type Error = String;

    fn try_from(args: SubscribeArgs) -> Result<Self, Self::Error> {
        match args.0 {
            SubscriptionKind::Logs => {
                if let Some(filter) = args.1 {
                    Ok(InterestKind::Log(filter))
                } else {
                    Err("missing filter for Logs subscription".to_string())
                }
            }
            SubscriptionKind::NewHeads => {
                if args.1.is_some() {
                    Err("filter not supported for NewHeads subscription".to_string())
                } else {
                    Ok(InterestKind::Block)
                }
            }

            _ => Err(format!("unsupported subscription kind: {:?}", args.0)),
        }
    }
}

pub(super) async fn not_supported() -> ResponsePayload<(), ()> {
    ResponsePayload::internal_error_message(Cow::Borrowed(
        "Method not supported. See signet documentation for a list of unsupported methods: https://docs.signet.sh/.",
    ))
}

pub(super) async fn protocol_version<Host, Signet>(ctx: RpcCtx<Host, Signet>) -> Result<U64, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ctx.host()
        .network()
        .network_status()
        .await
        .map(|info| info.protocol_version)
        .map(U64::from)
        .map_err(|s| s.to_string())
}

pub(super) async fn syncing<Host, Signet>(ctx: RpcCtx<Host, Signet>) -> Result<bool, ()>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(ctx.host().network().is_syncing())
}

pub(super) async fn block_number<Host, Signet>(ctx: RpcCtx<Host, Signet>) -> Result<U64, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ctx.signet().provider().last_block_number().map(U64::from).map_err(|s| s.to_string())
}

pub(super) async fn chain_id<Host, Signet>(ctx: RpcCtx<Host, Signet>) -> Result<U64, ()>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(U64::from(ctx.signet().constants().ru_chain_id()))
}

pub(super) async fn block<T, Host, Signet>(
    hctx: HandlerCtx,
    BlockParams(t, full): BlockParams<T>,
    ctx: RpcCtx<Host, Signet>,
) -> Result<Option<RpcBlock<Ethereum>>, String>
where
    T: Into<BlockId>,
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let id = t.into();
    let task = async move { ctx.signet().block(id, full).await.map_err(|e| e.to_string()) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn block_tx_count<T, Host, Signet>(
    hctx: HandlerCtx,
    (t,): (T,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<Option<U64>, String>
where
    T: Into<BlockId>,
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let id = t.into();
    let task = async move { ctx.signet().tx_count(id).await.map_err(|e| e.to_string()) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn block_receipts<Host, Signet>(
    hctx: HandlerCtx,
    (id,): (BlockId,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<Option<Vec<RpcReceipt<Ethereum>>>, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move {
        let Some(receipts) = ctx.signet().raw_receipts(id).await.map_err(|e| e.to_string())? else {
            return Ok(None);
        };

        let Some((block_hash, block)) =
            ctx.signet().raw_block(id).await.map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };

        let header = block.header();
        let block_number = header.number;
        let base_fee = header.base_fee_per_gas;
        let excess_blob_gas = None;
        let timestamp = header.timestamp;

        block
            .body()
            .transactions()
            .zip(receipts.iter())
            .enumerate()
            .map(|(idx, (tx, receipt))| {
                let meta = TransactionMeta {
                    tx_hash: *tx.hash(),
                    index: idx as u64,
                    block_hash,
                    block_number,
                    base_fee,
                    excess_blob_gas,
                    timestamp,
                };
                build_signet_receipt(tx.to_owned(), meta, receipt.to_owned(), receipts.to_vec())
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
            .map_err(|e| e.to_string())
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn raw_transaction_by_hash<Host, Signet>(
    hctx: HandlerCtx,
    (hash,): (B256,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<Option<alloy::primitives::Bytes>, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move {
        ctx.signet()
            .provider()
            .transaction_by_hash(hash)
            .map_err(|e| e.to_string())
            .map(|tx| tx.as_ref().map(Encodable2718::encoded_2718).map(Into::into))
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn transaction_by_hash<Host, Signet>(
    hctx: HandlerCtx,
    (hash,): (B256,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<Option<RpcTransaction<Ethereum>>, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move { ctx.signet().rpc_transaction_by_hash(hash).map_err(|e| e.to_string()) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn raw_transaction_by_block_and_index<T, Host, Signet>(
    hctx: HandlerCtx,
    (t, index): (T, U64),
    ctx: RpcCtx<Host, Signet>,
) -> Result<Option<alloy::primitives::Bytes>, String>
where
    T: Into<BlockId>,
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let id: BlockId = t.into();
    let task = async move {
        let Some((_, block)) = ctx.signet().raw_block(id).await.map_err(|e| e.to_string())? else {
            return Ok(None);
        };

        Ok(block.body().transactions.get(index.to::<usize>()).map(|tx| tx.encoded_2718().into()))
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn transaction_by_block_and_index<T, Host, Signet>(
    hctx: HandlerCtx,
    (t, index): (T, U64),
    ctx: RpcCtx<Host, Signet>,
) -> Result<Option<RpcTransaction<Ethereum>>, String>
where
    T: Into<BlockId>,
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let id = t.into();

    let task = async move {
        ctx.signet()
            .rpc_transaction_by_block_idx(id, index.to::<usize>())
            .await
            .map_err(|e| e.to_string())
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn transaction_receipt<Host, Signet>(
    hctx: HandlerCtx,
    (hash,): (B256,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<Option<RpcReceipt<Ethereum>>, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task =
        async move { ctx.signet().rpc_receipt_by_hash(hash).await.map_err(|e| e.to_string()) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn balance<Host, Signet>(
    hctx: HandlerCtx,
    AddrWithBlock(address, block): AddrWithBlock,
    ctx: RpcCtx<Host, Signet>,
) -> Result<U256, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let block = block.unwrap_or(BlockId::latest());
    let task = async move {
        let state = ctx.signet().provider().state_by_block_id(block).map_err(|e| e.to_string())?;
        let bal = state.account_balance(&address).map_err(|e| e.to_string())?;
        Ok(bal.unwrap_or_default())
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn storage_at<Host, Signet>(
    hctx: HandlerCtx,
    StorageAtArgs(address, key, block): StorageAtArgs,
    ctx: RpcCtx<Host, Signet>,
) -> Result<B256, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let block = block.unwrap_or(BlockId::latest());
    let task = async move {
        let state = ctx.signet().provider().state_by_block_id(block).map_err(|e| e.to_string())?;
        let val = state.storage(address, key.into()).map_err(|e| e.to_string())?;
        Ok(val.unwrap_or_default().to_be_bytes().into())
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn addr_tx_count<Host, Signet>(
    hctx: HandlerCtx,
    AddrWithBlock(address, block): AddrWithBlock,
    ctx: RpcCtx<Host, Signet>,
) -> Result<U64, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let block = block.unwrap_or(BlockId::latest());
    let task = async move {
        let state = ctx.signet().provider().state_by_block_id(block).map_err(|e| e.to_string())?;
        let count = state.account_nonce(&address).map_err(|e| e.to_string())?;
        Ok(U64::from(count.unwrap_or_default()))
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn code_at<Host, Signet>(
    hctx: HandlerCtx,
    AddrWithBlock(address, block): AddrWithBlock,
    ctx: RpcCtx<Host, Signet>,
) -> Result<alloy::primitives::Bytes, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let block = block.unwrap_or(BlockId::latest());
    let task = async move {
        let state = ctx.signet().provider().state_by_block_id(block).map_err(|e| e.to_string())?;
        let code = state.account_code(&address).map_err(|e| e.to_string())?;
        Ok(code.unwrap_or_default().original_bytes())
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn header_by<T, Host, Signet>(
    hctx: HandlerCtx,
    (t,): (T,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<Option<RpcHeader<Ethereum>>, String>
where
    T: Into<BlockId>,
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let id = t.into();

    await_jh_option!(hctx.spawn_blocking_with_ctx(|hctx| async move {
        Ok(block(hctx, BlockParams(id, None), ctx).await?.map(|block| block.header))
    }))
}

/// Normalize transaction request gas, without making DB reads
///
/// Does the following:
/// - If the gas is below `MIN_TRANSACTION_GAS`, set it to `None`
/// - If the gas is above the `rpc_gas_cap`, set it to the `rpc_gas_cap`
/// - Otherwise, do nothing
fn normalize_gas_stateless(request: &mut TransactionRequest, max_gas: u64) {
    match request.gas {
        Some(..trevm::MIN_TRANSACTION_GAS) => request.gas = None,
        Some(val) if val > max_gas => request.gas = Some(max_gas),
        _ => {}
    }
}

/// We want to ensure that req.gas is not less than `MIN_TRANSACTION_GAS`
/// coming into this.
pub(super) async fn run_call<Host, Signet>(
    hctx: HandlerCtx,
    TxParams(request, block, state_overrides, block_overrides): TxParams,
    ctx: RpcCtx<Host, Signet>,
) -> ResponsePayload<ExecutionResult, CallErrorData>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let id = block.unwrap_or(BlockId::latest());

    // this span is verbose yo.
    let span = trace_span!(
        "run_call",
        ?request,
        block_id = %id,
        state_overrides = ?state_overrides.as_ref().map(StateOverride::len).unwrap_or_default(),
        block_overrides = ?block_overrides.is_some(),
        block_cfg = tracing::field::Empty,
    );

    let task = async move {
        let block_cfg = match ctx.signet().block_cfg(id).await {
            Ok(block_cfg) => block_cfg,
            Err(e) => {
                return ResponsePayload::internal_error_with_message_and_obj(
                    "error while loading block cfg".into(),
                    e.to_string().into(),
                )
            }
        };

        tracing::span::Span::current().record("block_cfg", format!("{:?}", &block_cfg));

        // Set up trevm

        let trevm = response_tri!(ctx.trevm(id, &block_cfg));

        let mut trevm = response_tri!(trevm.maybe_apply_state_overrides(state_overrides.as_ref()))
            .maybe_apply_block_overrides(block_overrides.as_deref())
            .fill_tx(&request);

        // AFTER applying overrides and filling the tx, we want to statefully
        // modify the gas cap.
        let new_gas = response_tri!(trevm.cap_tx_gas());
        if Some(new_gas) != request.gas {
            tracing::span::Span::current().record("request", format!("{:?}", &request));
        }

        let execution_result = response_tri!(trevm.call().map_err(EvmErrored::into_error)).0;

        ResponsePayload::Success(execution_result)
    }
    .instrument(span);

    await_jh_option_response!(hctx.spawn_blocking(task))
}

pub(super) async fn call<Host, Signet>(
    hctx: HandlerCtx,
    mut params: TxParams,
    ctx: RpcCtx<Host, Signet>,
) -> ResponsePayload<alloy::primitives::Bytes, CallErrorData>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    // Stateless gas normalization. We will do stateful gas normalization later
    // in [`run_call`].
    //
    // This check is done greedily, as it is a simple comparison.
    let max_gas = ctx.signet().config().rpc_gas_cap;
    normalize_gas_stateless(&mut params.0, max_gas);

    await_jh_option_response!(hctx.spawn_with_ctx(|hctx| async move {
        let res = match run_call(hctx, params, ctx).await {
            ResponsePayload::Success(res) => res,
            ResponsePayload::Failure(err) => return ResponsePayload::Failure(err),
        };

        match res {
            ExecutionResult::Success { output, .. } => {
                ResponsePayload::Success(output.data().clone())
            }
            ExecutionResult::Revert { output, .. } => {
                ResponsePayload::internal_error_with_message_and_obj(
                    "execution reverted".into(),
                    output.clone().into(),
                )
            }
            ExecutionResult::Halt { reason, .. } => {
                ResponsePayload::internal_error_with_message_and_obj(
                    "execution halted".into(),
                    format!("{reason:?}").into(),
                )
            }
        }
    }))
}

/// Estimate the gas cost of a transaction.
pub(super) async fn estimate_gas<Host, Signet>(
    hctx: HandlerCtx,
    TxParams(mut request, block, state_overrides, block_overrides): TxParams,
    ctx: RpcCtx<Host, Signet>,
) -> ResponsePayload<U64, CallErrorData>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let id = block.unwrap_or(BlockId::pending());

    // this span is verbose yo.
    let span = trace_span!(
        "estimate_gas",
        ?request,
        block_id = %id,
        state_overrides = ?state_overrides.as_ref().map(StateOverride::len).unwrap_or_default(),
        block_overrides = ?block_overrides.is_some(),
        block_cfg = tracing::field::Empty,
    );

    // Stateless gas normalization.
    let max_gas = ctx.signet().config().rpc_gas_cap;
    normalize_gas_stateless(&mut request, max_gas);

    tracing::span::Span::current().record("normalized_gas", format!("{:?}", request.gas));

    let task = async move {
        // Get the block cfg from backend, erroring if it fails
        let block_cfg = match ctx.signet().block_cfg(id).await {
            Ok(block_cfg) => block_cfg,
            Err(e) => {
                return ResponsePayload::internal_error_with_message_and_obj(
                    "error while loading block cfg".into(),
                    e.to_string().into(),
                )
            }
        };

        tracing::span::Span::current().record("block_cfg", format!("{:?}", &block_cfg));

        let trevm = response_tri!(ctx.trevm(id, &block_cfg));

        // Apply state and block overrides (state overrides are fallible as
        // they require DB access)
        let trevm = response_tri!(trevm.maybe_apply_state_overrides(state_overrides.as_ref()))
            .maybe_apply_block_overrides(block_overrides.as_deref())
            .fill_tx(&request);

        // in eth_call we cap gas here. in eth_estimate gas it is done by
        // trevm

        let (estimate, _) = response_tri!(trevm.estimate_gas().map_err(EvmErrored::into_error));

        match estimate {
            trevm::EstimationResult::Success { estimation, .. } => {
                ResponsePayload::Success(U64::from(estimation))
            }
            trevm::EstimationResult::Revert { reason, .. } => {
                ResponsePayload::internal_error_with_message_and_obj(
                    "execution reverted".into(),
                    reason.clone().into(),
                )
            }
            trevm::EstimationResult::Halt { reason, .. } => {
                ResponsePayload::internal_error_with_message_and_obj(
                    "execution halted".into(),
                    format!("{reason:?}").into(),
                )
            }
        }
    }
    .instrument(span);

    await_jh_option_response!(hctx.spawn_blocking(task))
}

pub(super) async fn gas_price<Host, Signet>(
    hctx: HandlerCtx,
    ctx: RpcCtx<Host, Signet>,
) -> Result<U256, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move {
        let (block, suggested) = tokio::try_join!(
            ctx.signet().raw_block(BlockId::latest()),
            ctx.signet().gas_oracle().suggest_tip_cap(),
        )
        .map_err(|e| e.to_string())?;

        let base_fee = block.and_then(|b| b.1.header().base_fee_per_gas()).unwrap_or_default();
        Ok(suggested + U256::from(base_fee))
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn max_priority_fee_per_gas<Host, Signet>(
    hctx: HandlerCtx,
    ctx: RpcCtx<Host, Signet>,
) -> Result<U256, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task =
        async move { ctx.signet().gas_oracle().suggest_tip_cap().await.map_err(|e| e.to_string()) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn fee_history<Host, Signet>(
    hctx: HandlerCtx,
    FeeHistoryArgs(block_count, newest, reward_percentiles): FeeHistoryArgs,
    ctx: RpcCtx<Host, Signet>,
) -> Result<alloy::rpc::types::FeeHistory, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move {
        ctx.signet()
            .fee_history(block_count.to::<u64>(), newest, reward_percentiles)
            .await
            .map_err(|e| e.to_string())
    };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn send_raw_transaction<Host, Signet>(
    hctx: HandlerCtx,
    (tx,): (alloy::primitives::Bytes,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<B256, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = |hctx: HandlerCtx| async move {
        let Some(tx_cache) = ctx.signet().tx_cache() else {
            return Err("tx-cache URL not provided".to_string());
        };

        let envelope = match TxEnvelope::decode_2718(&mut tx.as_ref()) {
            Ok(envelope) => envelope,
            Err(e) => return Err(e.to_string()),
        };

        let hash = *envelope.tx_hash();
        hctx.spawn(async move {
            tx_cache.forward_raw_transaction(envelope).await.map_err(|e| e.to_string())
        });

        Ok(hash)
    };

    await_jh_option!(hctx.spawn_blocking_with_ctx(task))
}

pub(super) async fn get_logs<Host, Signet>(
    hctx: HandlerCtx,
    (filter,): (alloy::rpc::types::Filter,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<Vec<alloy::rpc::types::Log>, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move { ctx.signet().logs(&filter).await.map_err(EthError::into_string) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn new_filter<Host, Signet>(
    hctx: HandlerCtx,
    (filter,): (alloy::rpc::types::Filter,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<U64, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task =
        async move { ctx.signet().install_log_filter(filter).map_err(EthError::into_string) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn new_block_filter<Host, Signet>(
    hctx: HandlerCtx,
    ctx: RpcCtx<Host, Signet>,
) -> Result<U64, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move { ctx.signet().install_block_filter().map_err(EthError::into_string) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn uninstall_filter<Host, Signet>(
    hctx: HandlerCtx,
    (id,): (U64,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<bool, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move { Ok(ctx.signet().uninstall_filter(id)) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn get_filter_changes<Host, Signet>(
    hctx: HandlerCtx,
    (id,): (U64,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<FilterOutput, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move { ctx.signet().filter_changes(id).await.map_err(EthError::into_string) };

    await_jh_option!(hctx.spawn_blocking(task))
}

pub(super) async fn subscribe<Host, Signet>(
    hctx: HandlerCtx,
    sub: SubscribeArgs,
    ctx: RpcCtx<Host, Signet>,
) -> Result<U64, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let kind = sub.try_into()?;

    let task = |hctx| async move {
        ctx.signet()
            .subscriptions()
            .subscribe(&hctx, kind)
            .ok_or_else(|| "pubsub not enabled".to_string())
    };

    await_jh_option!(hctx.spawn_blocking_with_ctx(task))
}

pub(super) async fn unsubscribe<Host, Signet>(
    hctx: HandlerCtx,
    (id,): (U64,),
    ctx: RpcCtx<Host, Signet>,
) -> Result<bool, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = async move { Ok(ctx.signet().subscriptions().unsubscribe(id)) };

    await_jh_option!(hctx.spawn_blocking(task))
}
