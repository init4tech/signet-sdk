use ajj::{
    pubsub::{Connect, ServerShutdown},
    Router,
};
use axum::http::HeaderValue;
use interprocess::local_socket as ls;
use reqwest::Method;
use reth::{
    primitives::EthPrimitives, providers::providers::ProviderNodeTypes,
    rpc::builder::CorsDomainError, tasks::TaskExecutor,
};
use reth_chainspec::ChainSpec;
use std::{future::IntoFuture, iter::StepBy, net::SocketAddr, ops::RangeInclusive};
use tokio::task::JoinHandle;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::error;

macro_rules! await_jh_option {
    ($h:expr) => {
        match $h.await {
            Ok(Some(res)) => res,
            _ => return Err("task panicked or cancelled".to_string()),
        }
    };
}
pub(crate) use await_jh_option;

macro_rules! await_jh_option_response {
    ($h:expr) => {
        match $h.await {
            Ok(Some(res)) => res,
            _ => {
                return ResponsePayload::internal_error_message(std::borrow::Cow::Borrowed(
                    "task panicked or cancelled",
                ))
            }
        }
    };
}
pub(crate) use await_jh_option_response;

macro_rules! response_tri {
    ($h:expr) => {
        match $h {
            Ok(res) => res,
            Err(err) => return ResponsePayload::internal_error_message(err.to_string().into()),
        }
    };

    ($h:expr, $msg:literal) => {
        match $h {
            Ok(res) => res,
            Err(_) => return ResponsePayload::internal_error_message($msg.into()),
        }
    };

    ($h:expr, $obj:expr) => {
        match $h {
            Ok(res) => res,
            Err(err) => returnResponsePayload::internal_error_with_message_and_obj(
                err.to_string().into(),
                $obj,
            ),
        }
    };

    ($h:expr, $msg:literal, $obj:expr) => {
        match $h {
            Ok(res) => res,
            Err(err) => {
                return ResponsePayload::internal_error_with_message_and_obj($msg.into(), $obj)
            }
        }
    };
}
pub(crate) use response_tri;

/// Convenience trait for specifying the [`ProviderNodeTypes`] implementation
/// required for Signet RPC functionality.
pub trait Pnt: ProviderNodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives> {}

impl<T> Pnt for T where T: ProviderNodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives> {}

/// An iterator that yields _inclusive_ block ranges of a given step size
#[derive(Debug)]
pub(crate) struct BlockRangeInclusiveIter {
    iter: StepBy<RangeInclusive<u64>>,
    step: u64,
    end: u64,
}

impl BlockRangeInclusiveIter {
    pub(crate) fn new(range: RangeInclusive<u64>, step: u64) -> Self {
        Self { end: *range.end(), iter: range.step_by(step as usize + 1), step }
    }
}

impl Iterator for BlockRangeInclusiveIter {
    type Item = (u64, u64);

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.iter.next()?;
        let end = (start + self.step).min(self.end);
        if start > end {
            return None;
        }
        Some((start, end))
    }
}

fn make_cors(cors: Option<&str>) -> Result<CorsLayer, CorsDomainError> {
    let origins = match cors {
        None => AllowOrigin::any(),
        Some(cors) => {
            if cors.split(',').any(|o| o == "*") {
                return Err(CorsDomainError::WildCardNotAllowed { input: cors.to_string() });
            }
            cors.split(',')
                .map(|domain| {
                    domain
                        .parse::<HeaderValue>()
                        .map_err(|_| CorsDomainError::InvalidHeader { domain: domain.to_string() })
                })
                .collect::<Result<Vec<_>, _>>()?
                .into()
        }
    };

    Ok(CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_origin(origins)
        .allow_headers(Any))
}

/// Serve the axum router on the specified addresses.
async fn serve(
    tasks: &TaskExecutor,
    addrs: &[SocketAddr],
    service: axum::Router,
) -> Result<JoinHandle<()>, eyre::Error> {
    let listener = tokio::net::TcpListener::bind(addrs).await?;

    let fut = async move {
        match axum::serve(listener, service).into_future().await {
            Ok(_) => (),
            Err(err) => error!(%err, "Error serving RPC via axum"),
        }
    };

    Ok(tasks.spawn(fut))
}

/// Serve the router on the given addresses using axum.
pub async fn serve_axum(
    tasks: &TaskExecutor,
    router: Router<()>,
    addrs: &[SocketAddr],
    cors: Option<&str>,
) -> eyre::Result<JoinHandle<()>> {
    let handle = tasks.handle().clone();
    let cors = make_cors(cors)?;

    let service = router.into_axum_with_handle("/", handle).layer(cors);

    serve(tasks, addrs, service).await
}

/// Serve the router on the given address using a Websocket.
pub async fn serve_ws(
    tasks: &TaskExecutor,
    router: Router<()>,
    addrs: &[SocketAddr],
    cors: Option<&str>,
) -> eyre::Result<JoinHandle<()>> {
    let handle = tasks.handle().clone();
    let cors = make_cors(cors)?;

    let service = router.into_axum_with_ws_and_handle("/rpc", "/", handle).layer(cors);

    serve(tasks, addrs, service).await
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

// Some code in this file has been copied and modified from reth
// <https://github.com/paradigmxyz/reth>
// The original license is included below:
//
// The MIT License (MIT)
//
// Copyright (c) 2022-2025 Reth Contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//.
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
