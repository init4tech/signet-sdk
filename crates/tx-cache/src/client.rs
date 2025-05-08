use crate::types::{
    TxCacheBundle, TxCacheBundleResponse, TxCacheBundlesResponse, TxCacheOrdersResponse, TxCacheSendBundleResponse, TxCacheSendTransactionResponse, TxCacheTransactionsResponse
};
use alloy::consensus::TxEnvelope;
use eyre::Error;
use serde::{de::DeserializeOwned, Serialize};
use signet_bundle::SignetEthBundle;
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

    async fn forward_inner<T: Serialize + Send, R: DeserializeOwned>(
        &self,
        join: &'static str,
        obj: T,
    ) -> Result<R, Error> {
        // Append the path to the URL.
        let url = self
            .url
            .join(join)
            .inspect_err(|e| warn!(%e, "Failed to join URL. Not forwarding transaction."))?;

        // Send the object.
        self
            .client
            .post(url)
            .json(&obj)
            .send()
            .await
            .inspect_err(|e| warn!(%e, "Failed to forward object"))?
            .json::<R>()
            .await
            .map_err(Into::into)
            .inspect_err(|e| warn!(%e, "Failed to parse response from transaction cache"))
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
    pub async fn forward_raw_transaction(&self, tx: TxEnvelope) -> Result<TxCacheSendTransactionResponse, Error> {
        self.forward_inner(TRANSACTIONS, tx).await
    }

    /// Forward a bundle to the URL.
    #[instrument(skip_all)]
    pub async fn forward_bundle(&self, bundle: SignetEthBundle) -> Result<TxCacheSendBundleResponse, Error> {
        self.forward_inner(BUNDLES, bundle).await
    }

    /// Forward an order to the URL.
    #[instrument(skip_all)]
    pub async fn forward_order(&self, order: SignedOrder) -> Result<(), Error> {
        self.forward_inner(ORDERS, order).await
    }

    /// Get transactions from the URL.
    #[instrument(skip_all)]
    pub async fn get_transactions(&self) -> Result<Vec<TxEnvelope>, Error> {
        let response: TxCacheTransactionsResponse =
            self.get_inner::<TxCacheTransactionsResponse>(TRANSACTIONS).await?;
        Ok(response.transactions)
    }

    /// Get bundles from the URL.
    #[instrument(skip_all)]
    pub async fn get_bundles(&self) -> Result<Vec<TxCacheBundle>, Error> {
        let response: TxCacheBundlesResponse =
            self.get_inner::<TxCacheBundlesResponse>(BUNDLES).await?;
        Ok(response.bundles)
    }

    /// Get a bundle from the URL.
    #[instrument(skip_all)]
    pub async fn get_bundle(&self) -> Result<TxCacheBundle, Error> {
        let response: TxCacheBundleResponse =
            self.get_inner::<TxCacheBundleResponse>(BUNDLES).await?;
        Ok(response.bundle)
    }

    /// Get signed orders from the URL.
    #[instrument(skip_all)]
    pub async fn get_orders(&self) -> Result<Vec<SignedOrder>, Error> {
        let response: TxCacheOrdersResponse =
            self.get_inner::<TxCacheOrdersResponse>(ORDERS).await?;
        Ok(response.orders)
    }
}
