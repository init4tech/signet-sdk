use alloy::consensus::TxEnvelope;
use reth::rpc::server_types::eth::EthResult;
use serde::Serialize;
use signet_bundle::SignetEthBundle;
use tracing::warn;
use zenith_types::SignedOrder;

/// Signet's Transaction Cache forwarder. Forwards transactions to a URL via
/// simple post.
#[derive(Debug, Clone)]
pub struct TxCacheForwarder {
    url: reqwest::Url,
    client: reqwest::Client,
}

impl TxCacheForwarder {
    /// Create a new forwarder with the given URL and client.
    pub const fn new_with_client(url: reqwest::Url, client: reqwest::Client) -> Self {
        Self { url, client }
    }

    /// Instantiate a new forwarder with the given URL and a new reqwest client.
    pub fn new(url: reqwest::Url) -> Self {
        Self { url, client: reqwest::Client::new() }
    }

    async fn forward_inner<T: Serialize + Send>(
        &self,
        join: &'static str,
        obj: T,
    ) -> EthResult<()> {
        // Append the path to the URL and send the object.
        let url = match self.url.join(join) {
            Ok(url) => url,
            Err(e) => {
                warn!(%e, "Failed to join URL. Not forwarding transaction.");
                return Ok(());
            }
        };

        let _ = self
            .client
            .post(url)
            .json(&obj)
            .send()
            .await
            .inspect_err(|e| warn!(%e, "Failed to forward object"));

        Ok(())
    }

    /// Forwards a raw transaction to the URL.
    #[tracing::instrument(skip_all)]
    pub async fn forward_raw_transaction(&self, tx: TxEnvelope) -> EthResult<()> {
        self.forward_inner("transactions", tx).await
    }

    /// Forward a bundle to the URL.
    #[tracing::instrument(skip_all)]
    pub async fn forward_bundle(&self, bundle: SignetEthBundle) -> EthResult<()> {
        self.forward_inner("bundles", bundle).await
    }

    /// Forward an order to the URL.
    #[tracing::instrument(skip_all)]
    pub async fn forward_order(&self, order: SignedOrder) -> EthResult<()> {
        self.forward_inner("orders", order).await
    }
}
