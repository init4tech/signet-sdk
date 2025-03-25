//! Signet RPC methods and related code.
use crate::util::await_jh_option;
use crate::{ctx::RpcCtx, Pnt};
use ajj::HandlerCtx;
use reth_node_api::FullNodeComponents;
use signet_bundle::SignetEthBundle;
use signet_zenith::SignedOrder;

/// Instantiate a `signet` API router.
pub fn signet<Host, Signet>() -> ajj::Router<RpcCtx<Host, Signet>>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ajj::Router::new().route("sendBundle", send_bundle).route("sendOrder", send_order)
}

pub(super) async fn send_bundle<Host, Signet>(
    hctx: HandlerCtx,
    bundle: SignetEthBundle,
    ctx: RpcCtx<Host, Signet>,
) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = |hctx: HandlerCtx| async move {
        let Some(forwarder) = ctx.signet().forwarder() else {
            return Err("tx-cache URL not provided".to_string());
        };

        hctx.spawn(
            async move { forwarder.forward_bundle(bundle).await.map_err(|e| e.to_string()) },
        );

        Ok(())
    };

    await_jh_option!(hctx.spawn_blocking_with_ctx(task))
}

pub(super) async fn send_order<Host, Signet>(
    hctx: HandlerCtx,
    order: SignedOrder,
    ctx: RpcCtx<Host, Signet>,
) -> Result<(), String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let task = |hctx: HandlerCtx| async move {
        let Some(forwarder) = ctx.signet().forwarder() else {
            return Err("tx-cache URL not provided".to_string());
        };

        hctx.spawn(async move { forwarder.forward_order(order).await.map_err(|e| e.to_string()) });

        Ok(())
    };

    await_jh_option!(hctx.spawn_blocking_with_ctx(task))
}
