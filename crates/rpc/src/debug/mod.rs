mod endpoints;
use endpoints::*;
use reth_node_api::FullNodeComponents;

use crate::{Pnt, RpcCtx};

/// Instantiate the debug router.
pub fn debug<Host, Signet>() -> ajj::Router<RpcCtx<Host, Signet>>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ajj::Router::new()
        .route("traceTransaction", trace_transaction)
        .route("traceBlockByNumber", trace_block_by_number)
}
