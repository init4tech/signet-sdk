use crate::{Pnt, RpcCtx};
use ajj::HandlerCtx;
use alloy::{eips::BlockNumberOrTag, primitives::B256};
use reth::rpc::types::trace::geth::GethDebugTracingOptions;
use reth_node_api::FullNodeComponents;

pub(super) async fn trace_transaction<Host, Signet>(
    _hctx: HandlerCtx,
    (_tx_hash, _opts): (B256, GethDebugTracingOptions),
    _ctx: RpcCtx<Host, Signet>,
) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(())
}

pub(super) async fn trace_block_by_number<Host, Signet>(
    _hctx: HandlerCtx,
    (_block, _opts): (BlockNumberOrTag, Option<GethDebugTracingOptions>),
    _ctx: RpcCtx<Host, Signet>,
) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(())
}
