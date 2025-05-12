use crate::{Pnt, RpcCtx};
use reth_node_api::FullNodeComponents;

pub(super) async fn trace_transaction<Host, Signet>(
    _ctx: RpcCtx<Host, Signet>,
) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(())
}

pub(super) async fn trace_block_by_number<Host, Signet>(
    _ctx: RpcCtx<Host, Signet>,
) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    Ok(())
}
