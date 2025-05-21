use crate::{Pnt, RpcCtx};
use ajj::HandlerCtx;
use alloy::{eips::BlockId, primitives::map::foldhash::HashSet};
use reth::rpc::types::trace::parity::TraceType;
use reth_node_api::FullNodeComponents;

pub(super) async fn trace_block<Host, Signet>(
    _hctx: HandlerCtx,
    _block_id: BlockId,
    _ctx: RpcCtx<Host, Signet>,
) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(())
}

pub(super) async fn trace_replay_block_transactions<Host, Signet>(
    _hctx: HandlerCtx,
    (_block_id, _trace_types): (BlockId, HashSet<TraceType>),
    _ctx: RpcCtx<Host, Signet>,
) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(())
}
