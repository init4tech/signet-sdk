mod endpoints;
use endpoints::*;

use crate::{ctx::RpcCtx, Pnt};
use reth_node_api::FullNodeComponents;

/// Instantiate the `trace` API router.
pub fn trace<Host, Signet>() -> ajj::Router<RpcCtx<Host, Signet>>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ajj::Router::new()
        .route("block", trace_block)
        .route("replayBlockTransactions", trace_replay_block_transactions)
}
