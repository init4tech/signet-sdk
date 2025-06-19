use crate::types::{
    TxCacheOrdersResponse, TxCacheSendBundleResponse, TxCacheSendTransactionResponse,
    TxCacheTransactionsResponse,
};
use alloy::consensus::TxEnvelope;
use eyre::Error;
use serde::{de::DeserializeOwned, Serialize};
use signet_bundle::SignetEthBundle;
use signet_constants::pecorino;
use signet_types::SignedOrder;
use tracing::{instrument, warn};

/// The endpoints for the transaction cache.
const TRANSACTIONS: &str = "transactions";
const BUNDLES: &str = "bundles";
const ORDERS: &str = "orders";

/// Signet's Transaction Cache helper.
/// Forwards GET and POST requests to a tx cache URL.
#[derive(Debug, Clone)]
pub struct TxCache {
    /// The URL of the transaction cache.
    url: reqwest::Url,
    /// The reqwest client used to send requests.
    client: reqwest::Client,
}

impl TxCache {
    /// Create a new cache with the given URL and client.
    pub const fn new_with_client(url: reqwest::Url, client: reqwest::Client) -> Self {
        Self { url, client }
    }

    /// Instantiate a new cache with the given URL and a new reqwest client.
    pub fn new(url: reqwest::Url) -> Self {
        Self { url, client: reqwest::Client::new() }
    }

    /// Create a new cache given a string URL.
    pub fn new_from_string(url: &str) -> Result<Self, Error> {
        let url = reqwest::Url::parse(url)?;
        Ok(Self::new(url))
    }

    /// Connect to the Pecorino tx cache.
    pub fn pecorino() -> Self {
        Self::new_from_string(pecorino::TX_CACHE_URL).expect("pecorino tx cache URL invalid")
    }

    /// Connect to the Pecornio tx cache, using a specific [`Client`].
    pub fn pecorino_with_client(client: reqwest::Client) -> Self {
        let url =
            reqwest::Url::parse(pecorino::TX_CACHE_URL).expect("pecorino tx cache URL invalid");
        Self::new_with_client(url, client)
    }

    /// Get the client used to send requests
    pub const fn client(&self) -> &reqwest::Client {
        &self.client
    }

    async fn forward_inner<T: Serialize + Send, R: DeserializeOwned>(
        &self,
        join: &'static str,
        obj: T,
    ) -> Result<R, Error> {
        self.forward_inner_raw(join, obj)
            .await?
            .json::<R>()
            .await
            .inspect_err(|e| warn!(%e, "Failed to parse response from transaction cache"))
            .map_err(Into::into)
    }

    async fn forward_inner_raw<T: Serialize + Send>(
        &self,
        join: &'static str,
        obj: T,
    ) -> Result<reqwest::Response, Error> {
        // Append the path to the URL.
        let url = self
            .url
            .join(join)
            .inspect_err(|e| warn!(%e, "Failed to join URL. Not forwarding transaction."))?;

        // Send the object and check for success.
        self.client.post(url).json(&obj).send().await?.error_for_status().map_err(Into::into)
    }

    async fn get_inner<T>(&self, join: &'static str) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        // Append the path to the URL.
        let url = self
            .url
            .join(join)
            .inspect_err(|e| warn!(%e, "Failed to join URL. Not querying transaction cache."))?;

        // Get the result.
        self.client
            .get(url)
            .send()
            .await
            .inspect_err(|e| warn!(%e, "Failed to get object from transaction cache"))?
            .json::<T>()
            .await
            .map_err(Into::into)
    }

    /// Forwards a raw transaction to the URL.
    #[instrument(skip_all)]
    pub async fn forward_raw_transaction(
        &self,
        tx: TxEnvelope,
    ) -> Result<TxCacheSendTransactionResponse, Error> {
        self.forward_inner(TRANSACTIONS, tx).await
    }

    /// Forward a bundle to the URL.
    #[instrument(skip_all)]
    pub async fn forward_bundle(
        &self,
        bundle: SignetEthBundle,
    ) -> Result<TxCacheSendBundleResponse, Error> {
        self.forward_inner(BUNDLES, bundle).await
    }

    /// Forward an order to the URL.
    #[instrument(skip_all)]
    pub async fn forward_order(&self, order: SignedOrder) -> Result<(), Error> {
        self.forward_inner_raw(ORDERS, order).await.map(drop)
    }

    /// Get transactions from the URL.
    #[instrument(skip_all)]
    pub async fn get_transactions(&self) -> Result<Vec<TxEnvelope>, Error> {
        let response: TxCacheTransactionsResponse =
            self.get_inner::<TxCacheTransactionsResponse>(TRANSACTIONS).await?;
        Ok(response.transactions)
    }

    /// Get signed orders from the URL.
    #[instrument(skip_all)]
    pub async fn get_orders(&self) -> Result<Vec<SignedOrder>, Error> {
        let response: TxCacheOrdersResponse =
            self.get_inner::<TxCacheOrdersResponse>(ORDERS).await?;
        Ok(response.orders)
    }
}
