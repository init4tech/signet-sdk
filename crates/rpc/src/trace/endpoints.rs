use crate::{Pnt, RpcCtx};
use reth_node_api::FullNodeComponents;

pub(super) async fn trace_block<Host, Signet>(_ctx: RpcCtx<Host, Signet>) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(())
}

pub(super) async fn trace_replay_block_transactions<Host, Signet>(
    _ctx: RpcCtx<Host, Signet>,
) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(())
}
