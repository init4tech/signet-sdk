use alloy::consensus::TxEnvelope;
use eyre::Error;
use reth::rpc::server_types::eth::EthResult;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use signet_bundle::SignetEthBundle;
use signet_zenith::SignedOrder;
use tracing::{instrument, warn};

/// The endpoints for the transaction cache.
const TRANSACTIONS: &str = "transactions";
const BUNDLES: &str = "bundles";
const ORDERS: &str = "orders";

/// Signet's Transaction Cache helper.
/// Forwards GET and POST requests to a tx cache URL.
#[derive(Debug, Clone)]
pub struct TxCache {
    url: reqwest::Url,
    client: reqwest::Client,
}

/// A bundle response from the transaction cache, containing a UUID and a
/// [`SignetEthBundle`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignetEthBundleResponse {
    /// The bundle id (a UUID)
    pub id: String,
    /// The bundle itself
    pub bundle: SignetEthBundle,
}

/// Response from the transaction cache `bundles` endpoint, containing a list of bundles.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxCacheBundleResponse {
    /// the list of bundles
    pub bundles: Vec<SignetEthBundleResponse>,
}

/// Response from the transaction cache `transactions` endpoint, containing a list of transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxCacheTransactionsResponse {
    transactions: Vec<TxEnvelope>,
}

/// Response from the transaction cache `orders` endpoint, containing a list of signed orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TxCacheOrderResponse {
    orders: Vec<SignedOrder>,
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
    ) -> EthResult<()> {
        // Append the path to the URL.
        let url = match self.url.join(join) {
            Ok(url) => url,
            Err(e) => {
                warn!(%e, "Failed to join URL. Not forwarding transaction.");
                return Ok(());
            }
        };

        // Send the object.
        let _ = self
            .client
            .post(url)
            .json(&obj)
            .send()
            .await
            .inspect_err(|e| warn!(%e, "Failed to forward object"));

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
    pub async fn forward_raw_transaction(&self, tx: TxEnvelope) -> EthResult<()> {
        self.forward_inner(TRANSACTIONS, tx).await
    }

    /// Forward a bundle to the URL.
    #[instrument(skip_all)]
    pub async fn forward_bundle(&self, bundle: SignetEthBundle) -> EthResult<()> {
        self.forward_inner(BUNDLES, bundle).await
    }

    /// Forward an order to the URL.
    #[instrument(skip_all)]
    pub async fn forward_order(&self, order: SignedOrder) -> EthResult<()> {
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
        let response: TxCacheBundleResponse =
            self.get_inner::<TxCacheBundleResponse>(BUNDLES).await?;
        Ok(response.bundles)
    }

    /// Get signed orders from the URL.
    #[instrument(skip_all)]
    pub async fn get_orders(&self) -> Result<Vec<SignedOrder>, Error> {
        let response: TxCacheOrderResponse =
            self.get_inner::<TxCacheOrderResponse>(ORDERS).await?;
        Ok(response.orders)
    }
}
