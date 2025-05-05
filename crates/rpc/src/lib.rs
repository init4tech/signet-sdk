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
//! use signet_rpc::{router, ServeConfig};
//!
//! # pub async fn f<Host, Signet>(ctx: RpcCtx<Host, Signet>, tasks: &TaskExecutor) -> eyre::Result<()>
//! # where
//! #   Host: FullNodeComponents,
//! #   Signet: Pnt,
//! # {
//! let router = signet_rpc::router().with_state(ctx);
//!
//! let cfg = ServeConfig {
//!     http: vec!["localhost:8080".parse()?],
//!     http_cors: None,
//!     ws: vec![],
//!     ws_cors: None,
//!     ipc: None,
//! };
//!
//! // Spawn the server on the given addresses, the shutdown guard
//! // will shutdown the server(s) when dropped.
//! let shutdown_guard = cfg.serve(tasks, router).await?;
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

mod config;
pub use config::{RpcServerGuard, ServeConfig};

mod ctx;
pub use ctx::RpcCtx;

mod eth;
pub use eth::{eth, CallErrorData, EthError};

mod signet;
pub use signet::{error::SignetError, signet};

mod interest;

pub mod receipts;

/// Utils and simple serve functions.
pub mod util;
pub use util::Pnt;

/// Re-exported for convenience
pub use ::ajj;

use ajj::Router;
use reth_node_api::FullNodeComponents;

/// Create a new router with the given host and signet types.
pub fn router<Host, Signet>() -> Router<ctx::RpcCtx<Host, Signet>>
where
    Host: FullNodeComponents,
    Signet: Pnt,
{
    ajj::Router::new().nest("eth", eth::<Host, Signet>()).nest("signet", signet::<Host, Signet>())
}
