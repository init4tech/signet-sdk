use ajj::{pubsub::ServerShutdown, Router};
use reth::tasks::TaskExecutor;
use std::net::SocketAddr;
use tokio::task::JoinHandle;

use crate::util::{serve_axum, serve_ipc, serve_ws};

/// Guard to shutdown the RPC servers. When dropped, this will shutdown all
/// running servers
#[derive(Default)]
pub struct RpcServerGuard {
    http: Option<tokio::task::JoinHandle<()>>,
    ws: Option<tokio::task::JoinHandle<()>>,
    ipc: Option<ServerShutdown>,
}

impl core::fmt::Debug for RpcServerGuard {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RpcServerGuard")
            .field("http", &self.http.is_some())
            .field("ipc", &self.ipc.is_some())
            .field("ws", &self.ws.is_some())
            .finish()
    }
}

impl Drop for RpcServerGuard {
    fn drop(&mut self) {
        if let Some(http) = self.http.take() {
            http.abort();
        }
        if let Some(ws) = self.ws.take() {
            ws.abort();
        }
        // IPC is handled by its own drop guards.
    }
}

/// Configuration for the RPC server.
#[derive(Clone, Debug)]
pub struct ServeConfig {
    /// HTTP server addresses.
    pub http: Vec<SocketAddr>,
    /// WS server addresses.
    pub ws: Vec<SocketAddr>,
    /// CORS header to be used for HTTP and WS servers (if any).
    pub cors: Option<String>,
    /// IPC name info.
    pub ipc: Option<String>,
}

impl ServeConfig {
    /// Serve the router on the given addresses.
    async fn serve_http(
        &self,
        tasks: &TaskExecutor,
        router: Router<()>,
    ) -> eyre::Result<Option<JoinHandle<()>>> {
        if self.http.is_empty() {
            return Ok(None);
        }
        serve_axum(tasks, router, &self.http, self.cors.as_deref()).await.map(Some)
    }

    /// Serve the router on the given addresses.
    async fn serve_ws(
        &self,
        tasks: &TaskExecutor,
        router: Router<()>,
    ) -> eyre::Result<Option<JoinHandle<()>>> {
        if self.ws.is_empty() {
            return Ok(None);
        }
        serve_ws(tasks, router, &self.ws, self.cors.as_deref()).await.map(Some)
    }

    /// Serve the router on the given ipc path.
    async fn serve_ipc(
        &self,
        tasks: &TaskExecutor,
        router: &Router<()>,
    ) -> eyre::Result<Option<ServerShutdown>> {
        let Some(endpoint) = &self.ipc else { return Ok(None) };
        let shutdown = serve_ipc(tasks, router, endpoint).await?;
        Ok(Some(shutdown))
    }

    /// Serve the router.
    pub async fn serve(
        &self,
        tasks: &TaskExecutor,
        router: Router<()>,
    ) -> eyre::Result<RpcServerGuard> {
        let (http, ws, ipc) = tokio::try_join!(
            self.serve_http(tasks, router.clone()),
            self.serve_ws(tasks, router.clone()),
            self.serve_ipc(tasks, &router),
        )?;
        Ok(RpcServerGuard { http, ws, ipc })
    }
}
