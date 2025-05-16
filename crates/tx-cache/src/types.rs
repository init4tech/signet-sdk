//! The endpoints for the transaction cache.
use alloy::{consensus::TxEnvelope, primitives::B256};
use serde::{Deserialize, Serialize};
use signet_bundle::SignetEthBundle;
use signet_types::SignedOrder;

/// A bundle response from the transaction cache, containing a UUID and a
/// [`SignetEthBundle`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxCacheBundle {
    /// The bundle id (a UUID)
    pub id: uuid::Uuid,
    /// The bundle itself
    pub bundle: SignetEthBundle,
}

impl TxCacheBundle {
    /// Create a new bundle response from a bundle and an id.
    pub const fn new(bundle: SignetEthBundle, id: uuid::Uuid) -> Self {
        Self { id, bundle }
    }

    /// Create a new bundle response from a bundle and an id.
    #[deprecated = "Use `Self::new` instead"]
    pub const fn from_bundle_and_id(bundle: SignetEthBundle, id: uuid::Uuid) -> Self {
        Self::new(bundle, id)
    }

    /// Convert the bundle response to a [`SignetEthBundle`].
    pub fn into_bundle(self) -> SignetEthBundle {
        self.bundle
    }

    /// Convert the bundle response to a [uuid::Uuid].
    pub fn into_id(self) -> uuid::Uuid {
        self.id
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
pub struct TxCacheBundleResponse {
    /// The bundle.
    pub bundle: TxCacheBundle,
}

impl From<TxCacheBundle> for TxCacheBundleResponse {
    fn from(bundle: TxCacheBundle) -> Self {
        Self { bundle }
    }
}

impl From<TxCacheBundleResponse> for TxCacheBundle {
    fn from(response: TxCacheBundleResponse) -> Self {
        response.bundle
    }
}

impl TxCacheBundleResponse {
    /// Create a new bundle response from a bundle.
    pub const fn new(bundle: TxCacheBundle) -> Self {
        Self { bundle }
    }

    /// Create a new bundle response from a bundle.
    #[deprecated = "Use `From::from` instead, `Self::new` in const contexts"]
    pub const fn from_bundle(bundle: TxCacheBundle) -> Self {
        Self::new(bundle)
    }

    /// Convert the bundle response to a [`SignetEthBundle`].
    #[deprecated = "Use `this.bundle` instead."]
    pub fn into_bundle(self) -> TxCacheBundle {
        self.bundle
    }
}

/// Response from the transaction cache `bundles` endpoint, containing a list of bundles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxCacheBundlesResponse {
    /// the list of bundles
    pub bundles: Vec<TxCacheBundle>,
}

impl From<Vec<TxCacheBundle>> for TxCacheBundlesResponse {
    fn from(bundles: Vec<TxCacheBundle>) -> Self {
        Self { bundles }
    }
}

impl From<TxCacheBundlesResponse> for Vec<TxCacheBundle> {
    fn from(response: TxCacheBundlesResponse) -> Self {
        response.bundles
    }
}

impl TxCacheBundlesResponse {
    /// Create a new bundle response from a list of bundles.
    pub const fn new(bundles: Vec<TxCacheBundle>) -> Self {
        Self { bundles }
    }

    /// Create a new bundle response from a list of bundles.
    #[deprecated = "Use `From::from` instead, `Self::new` in const contexts"]
    pub const fn from_bundles(bundles: Vec<TxCacheBundle>) -> Self {
        Self::new(bundles)
    }

    /// Convert the bundle response to a list of [`SignetEthBundle`].
    #[deprecated = "Use `this.bundles` instead."]
    pub fn into_bundles(self) -> Vec<TxCacheBundle> {
        self.bundles
    }
}

/// Represents a response to successfully adding or updating a bundle in the transaction cache.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct TxCacheSendBundleResponse {
    /// The bundle id (a UUID)
    pub id: uuid::Uuid,
}

impl TxCacheSendBundleResponse {
    /// Create a new bundle response from a bundle id.
    pub const fn new(id: uuid::Uuid) -> Self {
        Self { id }
    }
}

impl From<uuid::Uuid> for TxCacheSendBundleResponse {
    fn from(id: uuid::Uuid) -> Self {
        Self { id }
    }
}

impl From<TxCacheSendBundleResponse> for uuid::Uuid {
    fn from(response: TxCacheSendBundleResponse) -> Self {
        response.id
    }
}

/// Response from the transaction cache `transactions` endpoint, containing a list of transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxCacheTransactionsResponse {
    /// The list of transactions.
    pub transactions: Vec<TxEnvelope>,
}

impl From<Vec<TxEnvelope>> for TxCacheTransactionsResponse {
    fn from(transactions: Vec<TxEnvelope>) -> Self {
        Self { transactions }
    }
}

impl From<TxCacheTransactionsResponse> for Vec<TxEnvelope> {
    fn from(response: TxCacheTransactionsResponse) -> Self {
        response.transactions
    }
}

impl TxCacheTransactionsResponse {
    /// Instantiate a new transaction response from a list of transactions.
    pub const fn new(transactions: Vec<TxEnvelope>) -> Self {
        Self { transactions }
    }

    /// Create a new transaction response from a list of transactions.
    #[deprecated = "Use `From::from` instead, or `Self::new` in const contexts"]
    pub const fn from_transactions(transactions: Vec<TxEnvelope>) -> Self {
        Self::new(transactions)
    }

    /// Convert the transaction response to a list of [`TxEnvelope`].
    #[deprecated = "Use `this.transactions` instead."]
    pub fn into_transactions(self) -> Vec<TxEnvelope> {
        self.transactions
    }
}

/// Response from the transaction cache to successfully adding a transaction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TxCacheSendTransactionResponse {
    /// The transaction hash
    pub tx_hash: B256,
}

impl From<B256> for TxCacheSendTransactionResponse {
    fn from(tx_hash: B256) -> Self {
        Self { tx_hash }
    }
}

impl From<TxCacheSendTransactionResponse> for B256 {
    fn from(response: TxCacheSendTransactionResponse) -> Self {
        response.tx_hash
    }
}

impl TxCacheSendTransactionResponse {
    /// Create a new transaction response from a transaction hash.
    pub const fn new(tx_hash: B256) -> Self {
        Self { tx_hash }
    }

    /// Create a new transaction response from a transaction hash.
    #[deprecated = "Use `From::from` instead, or `Self::new` in const contexts"]
    pub const fn from_tx_hash(tx_hash: B256) -> Self {
        Self { tx_hash }
    }

    /// Convert the transaction response to a transaction hash.
    #[deprecated = "Use `this.tx_hash` instead."]
    pub const fn into_tx_hash(self) -> B256 {
        self.tx_hash
    }
}

/// Response from the transaction cache `orders` endpoint, containing a list of signed orders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxCacheOrdersResponse {
    /// The list of signed orders.
    pub orders: Vec<SignedOrder>,
}

impl From<Vec<SignedOrder>> for TxCacheOrdersResponse {
    fn from(orders: Vec<SignedOrder>) -> Self {
        Self { orders }
    }
}

impl From<TxCacheOrdersResponse> for Vec<SignedOrder> {
    fn from(response: TxCacheOrdersResponse) -> Self {
        response.orders
    }
}

impl TxCacheOrdersResponse {
    /// Create a new order response from a list of orders.
    pub const fn new(orders: Vec<SignedOrder>) -> Self {
        Self { orders }
    }

    /// Create a new order response from a list of orders.
    #[deprecated = "Use `From::from` instead, `Self::new` in const contexts"]
    pub const fn from_orders(orders: Vec<SignedOrder>) -> Self {
        Self { orders }
    }

    /// Convert the order response to a list of [`SignedOrder`].
    #[deprecated = "Use `this.orders` instead."]
    pub fn into_orders(self) -> Vec<SignedOrder> {
        self.orders
    }
}
