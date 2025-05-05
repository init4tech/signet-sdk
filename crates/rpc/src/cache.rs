use alloy::{consensus::TxEnvelope, primitives::B256};
use eyre::Error;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
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

/// A bundle response from the transaction cache, containing a UUID and a
/// [`SignetEthBundle`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignetEthBundleResponse {
    /// The bundle id (a UUID)
    pub id: uuid::Uuid,
    /// The bundle itself
    pub bundle: SignetEthBundle,
}

impl SignetEthBundleResponse {
    /// Create a new bundle response from a bundle and an id.
    pub const fn from_bundle_and_id(bundle: SignetEthBundle, id: uuid::Uuid) -> Self {
        Self { id, bundle }
    }

    /// Convert the bundle response to a [`SignetEthBundle`].
    pub fn into_bundle(self) -> SignetEthBundle {
        self.bundle
    }

    /// The bundle id.
    pub const fn id(&self) -> uuid::Uuid {
        self.id
    }

    /// The bundle itself.
    pub const fn bundle(&self) -> &SignetEthBundle {
        &self.bundle
    }
}

/// A response from the transaction cache, containing a single bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxCacheBundleResponse {
    /// The bundle
    pub bundle: SignetEthBundleResponse,
}

impl TxCacheBundleResponse {
    /// Create a new bundle response from a bundle.
    pub const fn from_bundle(bundle: SignetEthBundleResponse) -> Self {
        Self { bundle }
    }

    /// Convert the bundle response to a [`SignetEthBundle`].
    pub fn into_bundle(self) -> SignetEthBundleResponse {
        self.bundle
    }
}

impl From<SignetEthBundleResponse> for TxCacheBundleResponse {
    fn from(bundle: SignetEthBundleResponse) -> Self {
        Self { bundle }
    }
}

/// Response from the transaction cache `bundles` endpoint, containing a list of bundles.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxCacheBundlesResponse {
    /// the list of bundles
    pub bundles: Vec<SignetEthBundleResponse>,
}

impl TxCacheBundlesResponse {
    /// Create a new bundle response from a list of bundles.
    pub const fn from_bundles(bundles: Vec<SignetEthBundleResponse>) -> Self {
        Self { bundles }
    }

    /// Convert the bundle response to a list of [`SignetEthBundle`].
    pub fn into_bundles(self) -> Vec<SignetEthBundleResponse> {
        self.bundles
    }
}

/// Response from the transaction cache `transactions` endpoint, containing a list of transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxCacheTransactionsResponse {
    transactions: Vec<TxEnvelope>,
}

impl TxCacheTransactionsResponse {
    /// Create a new transaction response from a list of transactions.
    pub const fn from_transactions(transactions: Vec<TxEnvelope>) -> Self {
        Self { transactions }
    }

    /// Convert the transaction response to a list of [`TxEnvelope`].
    pub fn into_transactions(self) -> Vec<TxEnvelope> {
        self.transactions
    }
}

/// A response from the transaction cache, containing a transaction hash.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TxCacheTransactionResponse {
    /// The transaction hash
    tx_hash: B256,
}

impl TxCacheTransactionResponse {
    /// Create a new transaction response from a transaction hash.
    pub const fn from_tx_hash(tx_hash: B256) -> Self {
        Self { tx_hash }
    }

    /// Convert the transaction response to a transaction hash.
    pub const fn into_tx_hash(self) -> B256 {
        self.tx_hash
    }
}

/// Response from the transaction cache `orders` endpoint, containing a list of signed orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxCacheOrdersResponse {
    orders: Vec<SignedOrder>,
}

impl TxCacheOrdersResponse {
    /// Create a new order response from a list of orders.
    pub const fn from_orders(orders: Vec<SignedOrder>) -> Self {
        Self { orders }
    }

    /// Convert the order response to a list of [`SignedOrder`].
    pub fn into_orders(self) -> Vec<SignedOrder> {
        self.orders
    }
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

    async fn forward_inner<T: Serialize + Send>(
        &self,
        join: &'static str,
        obj: T,
    ) -> Result<(), Error> {
        // Append the path to the URL.
        let url = self
            .url
            .join(join)
            .inspect_err(|e| warn!(%e, "Failed to join URL. Not forwarding transaction."))?;

        // Send the object.
        let _ = self
            .client
            .post(url)
            .json(&obj)
            .send()
            .await
            .inspect_err(|e| warn!(%e, "Failed to forward object"))?;

        Ok(())
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
    pub async fn forward_raw_transaction(&self, tx: TxEnvelope) -> Result<(), Error> {
        self.forward_inner(TRANSACTIONS, tx).await
    }

    /// Forward a bundle to the URL.
    #[instrument(skip_all)]
    pub async fn forward_bundle(&self, bundle: SignetEthBundle) -> Result<(), Error> {
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
    pub async fn get_bundles(&self) -> Result<Vec<SignetEthBundleResponse>, Error> {
        let response: TxCacheBundlesResponse =
            self.get_inner::<TxCacheBundlesResponse>(BUNDLES).await?;
        Ok(response.bundles)
    }

    /// Get a bundle from the URL.
    #[instrument(skip_all)]
    pub async fn get_bundle(&self) -> Result<SignetEthBundleResponse, Error> {
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
