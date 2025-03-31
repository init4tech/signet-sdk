//! Signet RPC methods and related code.

mod endpoints;
use endpoints::*;

pub(crate) mod error;

use crate::{ctx::RpcCtx, Pnt};
use reth_node_api::FullNodeComponents;

/// Instantiate a `signet` API router.
pub fn signet<Host, Signet>() -> ajj::Router<RpcCtx<Host, Signet>>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ajj::Router::new()
        .route("sendBundle", send_bundle)
        .route("sendOrder", send_order)
        .route("callBundle", call_bundle)
}
