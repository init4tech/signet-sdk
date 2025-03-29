//! Signet RPC.
//!
//! This crate provides RPC endpoint definitions for the Signet node, as well
//! as the glue between the node and the RPC server. This RPC server is deeply
//! integrated with `reth`, and expects a variety of `reth`-specific types to be
//! passed in. As such, it is mostly useful within the context of a `signet`
//! node.
//!
//! ## Usage Example
//!
//! ```rust
//! # use signet_rpc::{Pnt, RpcCtx};
//! # use reth_node_api::FullNodeComponents;
//! # use reth::tasks::TaskExecutor;
//! use signet_rpc::{router, serve_axum};
//!
//! # pub async fn f<Host, Signet>(ctx: RpcCtx<Host, Signet>, tasks: &TaskExecutor) -> eyre::Result<()>
//! # where
//! #   Host: FullNodeComponents,
//! #   Signet: Pnt,
//! # {
//! let router = signet_rpc::router().with_state(ctx);
//! // Spawn the server on the given addresses.
//! let _ = serve_axum(
//!     tasks,
//!     &router,
//!     &["localhost:8080".parse()?],
//!     None
//! ).await?;
//! # Ok(())
//! # }
//! ```

#![warn(
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    clippy::missing_const_for_fn,
    rustdoc::all
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod ctx;
pub use ctx::RpcCtx;

mod eth;
pub use eth::{eth, CallErrorData, EthError};

mod signet;
pub use signet::{error::SignetError, signet};

mod interest;

mod forwarder;
use forwarder::TxCacheForwarder;

pub(crate) mod util;
pub use util::Pnt;

/// Re-exported for convenience
pub use ::ajj;

use ajj::{
    pubsub::{Connect, ServerShutdown},
    Router,
};
use axum::http::{HeaderValue, Method};
use interprocess::local_socket as ls;
use reth::tasks::TaskExecutor;
use reth_node_api::FullNodeComponents;
use std::{future::IntoFuture, net::SocketAddr};
use tokio::task::JoinHandle;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::error;

/// Create a new router with the given host and signet types.
pub fn router<Host, Signet>() -> Router<ctx::RpcCtx<Host, Signet>>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ajj::Router::new().nest("eth", eth::<Host, Signet>()).nest("signet", signet::<Host, Signet>())
}

/// Serve the router on the given addresses using axum.
pub async fn serve_axum(
    tasks: &TaskExecutor,
    router: &Router<()>,
    addrs: &[SocketAddr],
    cors: Option<&str>,
) -> eyre::Result<JoinHandle<()>> {
    let cors = cors
        .unwrap_or("*")
        .parse::<HeaderValue>()
        .map(Into::<AllowOrigin>::into)
        .unwrap_or_else(|_| AllowOrigin::any());

    let cors = CorsLayer::new().allow_methods([Method::GET, Method::POST]).allow_origin(cors);

    let service = router.clone().into_axum("/").layer(cors);

    let listener = tokio::net::TcpListener::bind(addrs).await?;

    let fut = async move {
        match axum::serve(listener, service).into_future().await {
            Ok(_) => (),
            Err(err) => error!(%err, "Error serving RPC via axum"),
        }
    };

    Ok(tasks.spawn(fut))
}

/// Serve the router on the given address using a Websocket.
pub const fn serve_ws(_tasks: &TaskExecutor, _router: &Router<()>, _addr: SocketAddr) {
    // TODO: ENG-826
}

fn to_name(path: &std::ffi::OsStr) -> std::io::Result<ls::Name<'_>> {
    if cfg!(windows) && !path.as_encoded_bytes().starts_with(br"\\.\pipe\") {
        ls::ToNsName::to_ns_name::<ls::GenericNamespaced>(path)
    } else {
        ls::ToFsName::to_fs_name::<ls::GenericFilePath>(path)
    }
}

/// Serve the router on the given address using IPC.
pub async fn serve_ipc(
    tasks: &TaskExecutor,
    router: &Router<()>,
    endpoint: &str,
) -> eyre::Result<ServerShutdown> {
    let name = std::ffi::OsStr::new(endpoint);
    let name = to_name(name).expect("invalid name");
    ls::ListenerOptions::new()
        .name(name)
        .serve_with_handle(router.clone(), tasks.handle().clone())
        .await
        .map_err(Into::into)
}
