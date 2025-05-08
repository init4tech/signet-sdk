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
    pub const fn from_bundle_and_id(bundle: SignetEthBundle, id: uuid::Uuid) -> Self {
        Self { id, bundle }
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
    /// The bundle
    pub bundle: TxCacheBundle,
}

impl TxCacheBundleResponse {
    /// Create a new bundle response from a bundle.
    pub const fn from_bundle(bundle: TxCacheBundle) -> Self {
        Self { bundle }
    }

    /// Convert the bundle response to a [`SignetEthBundle`].
    pub fn into_bundle(self) -> TxCacheBundle {
        self.bundle
    }
}

impl From<TxCacheBundle> for TxCacheBundleResponse {
    fn from(bundle: TxCacheBundle) -> Self {
        Self { bundle }
    }
}

/// Response from the transaction cache `bundles` endpoint, containing a list of bundles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxCacheBundlesResponse {
    /// the list of bundles
    pub bundles: Vec<TxCacheBundle>,
}

impl TxCacheBundlesResponse {
    /// Create a new bundle response from a list of bundles.
    pub const fn from_bundles(bundles: Vec<TxCacheBundle>) -> Self {
        Self { bundles }
    }

    /// Convert the bundle response to a list of [`SignetEthBundle`].
    pub fn into_bundles(self) -> Vec<TxCacheBundle> {
        self.bundles
    }
}

/// Represents a response to successfully adding or updating a bundle in the transaction cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxCacheSendBundleResponse {
    /// The bundle id (a UUID)
    pub id: uuid::Uuid,
}

/// Response from the transaction cache `transactions` endpoint, containing a list of transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxCacheTransactionsResponse {
    pub transactions: Vec<TxEnvelope>,
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
pub struct TxCacheOrdersResponse {
    pub orders: Vec<SignedOrder>,
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
