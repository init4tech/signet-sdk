use crate::{
    ctx::RpcCtx,
    signet::error::SignetError,
    util::{await_jh_option, await_jh_option_response, response_tri},
    Pnt,
};
use ajj::{HandlerCtx, ResponsePayload};
use reth_node_api::FullNodeComponents;
use signet_bundle::{
    SignetBundleDriver, SignetCallBundle, SignetCallBundleResponse, SignetEthBundle,
};
use signet_zenith::SignedOrder;
use std::time::Duration;
use tokio::select;

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
        let Some(tx_cache) = ctx.signet().tx_cache() else {
            return Err(SignetError::TxCacheUrlNotProvided.into_string());
        };

        hctx.spawn(async move {
            tx_cache
                .forward_bundle(bundle)
                .await
                .map_err(|e| SignetError::EthApiError(e).into_string())
        });

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
        let Some(tx_cache) = ctx.signet().tx_cache() else {
            return Err(SignetError::TxCacheUrlNotProvided.into_string());
        };

        hctx.spawn(async move {
            tx_cache
                .forward_order(order)
                .await
                .map_err(|e| SignetError::EthApiError(e).into_string())
        });

        Ok(())
    };

    await_jh_option!(hctx.spawn_blocking_with_ctx(task))
}

pub(super) async fn call_bundle<Host, Signet>(
    hctx: HandlerCtx,
    bundle: SignetCallBundle,
    ctx: RpcCtx<Host, Signet>,
) -> ResponsePayload<SignetCallBundleResponse, String>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    let timeout = bundle.bundle.timeout.unwrap_or(1000);

    let task = async move {
        let id = bundle.state_block_number();
        let block_cfg = match ctx.signet().block_cfg(id.into()).await {
            Ok(block_cfg) => block_cfg,
            Err(e) => {
                return ResponsePayload::internal_error_with_message_and_obj(
                    "error while loading block cfg".into(),
                    e.to_string(),
                )
            }
        };

        let mut driver = SignetBundleDriver::from(&bundle);

        let trevm = response_tri!(ctx.trevm(id.into(), &block_cfg));

        response_tri!(trevm.drive_bundle(&mut driver).map_err(|e| e.into_error()));

        ResponsePayload::Success(driver.into_response())
    };

    let task = async move {
        select! {
            _ = tokio::time::sleep(Duration::from_millis(timeout)) => {
                ResponsePayload::internal_error_message(
                    "timeout during bundle simulation".into(),
                )
            }
            result = task => {
                result
            }
        }
    };

    await_jh_option_response!(hctx.spawn_blocking(task))
}
