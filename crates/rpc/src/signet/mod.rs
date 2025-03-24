//! Signet RPC methods and related code.
use reth_node_api::FullNodeComponents;

use crate::{ctx::RpcCtx, util::not_supported, Pnt};

/// Instantiate a `signet` API router.
pub fn signet<Host, Signet>() -> ajj::Router<RpcCtx<Host, Signet>>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ajj::Router::new()
        .route("sendBundle", not_supported)
        .route("sendOrder", not_supported)
}