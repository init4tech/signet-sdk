//! The endpoints for the transaction cache.
use alloy::{consensus::TxEnvelope, primitives::B256};
use serde::{Deserialize, Serialize};
use signet_bundle::SignetEthBundle;
use signet_types::SignedOrder;
use std::collections::HashMap;
use uuid::Uuid;

/// A trait for allowing crusor keys to be converted into an URL query object.
pub trait CursorKey {
    /// Convert the cursor key into a URL query object.
    fn to_query_object(&self) -> HashMap<String, String>;
}

/// A trait for types that can be used as a cache object.
pub trait CacheObject {
    /// The cursor key type for the cache object.
    type Key: CursorKey;
}

/// A response from the transaction cache, containing an item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CacheResponse<T: CacheObject>
where
    T::Key: Serialize + for<'a> Deserialize<'a>,
{
    /// A paginated response, containing the inner item and a pagination info.
    Paginated {
        /// The actual item.
        #[serde(flatten)]
        inner: T,
        /// The pagination info.
        pagination: PaginationInfo<T::Key>,
    },
    /// An unpaginated response, containing the actual item.
    Unpaginated {
        /// The actual item.
        #[serde(flatten)]
        inner: T,
    },
}

impl<T: CacheObject> CacheObject for CacheResponse<T>
where
    T::Key: Serialize + for<'a> Deserialize<'a>,
{
    type Key = T::Key;
}

impl<T: CacheObject> CacheResponse<T>
where
    T::Key: Serialize + for<'a> Deserialize<'a>,
{
    /// Create a new paginated response from a list of items and a pagination info.
    pub const fn paginated(inner: T, pagination: PaginationInfo<T::Key>) -> Self {
        Self::Paginated { inner, pagination }
    }

    /// Create a new unpaginated response from a list of items.
    pub const fn unpaginated(inner: T) -> Self {
        Self::Unpaginated { inner }
    }

    /// Return a reference to the inner value.
    pub const fn inner(&self) -> &T {
        match self {
            Self::Paginated { inner, .. } => inner,
            Self::Unpaginated { inner } => inner,
        }
    }

    /// Return a mutable reference to the inner value.
    pub const fn inner_mut(&mut self) -> &mut T {
        match self {
            Self::Paginated { inner, .. } => inner,
            Self::Unpaginated { inner } => inner,
        }
    }

    /// Return the pagination info, if any.
    pub const fn pagination_info(&self) -> Option<&PaginationInfo<T::Key>> {
        match self {
            Self::Paginated { pagination, .. } => Some(pagination),
            Self::Unpaginated { .. } => None,
        }
    }

    /// Check if the response is paginated.
    pub const fn is_paginated(&self) -> bool {
        matches!(self, Self::Paginated { .. })
    }

    /// Check if the response is unpaginated.
    pub const fn is_unpaginated(&self) -> bool {
        matches!(self, Self::Unpaginated { .. })
    }

    /// Get the inner value.
    pub fn into_inner(self) -> T {
        match self {
            Self::Paginated { inner, .. } => inner,
            Self::Unpaginated { inner } => inner,
        }
    }

    /// Consume the response and return the parts.
    pub fn into_parts(self) -> (T, Option<PaginationInfo<T::Key>>) {
        match self {
            Self::Paginated { inner, pagination } => (inner, Some(pagination)),
            Self::Unpaginated { inner } => (inner, None),
        }
    }

    /// Consume the response and return the pagination info, if any.
    pub fn into_pagination_info(self) -> Option<PaginationInfo<T::Key>> {
        match self {
            Self::Paginated { pagination, .. } => Some(pagination),
            Self::Unpaginated { .. } => None,
        }
    }
}

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

impl CacheObject for TxCacheBundleResponse {
    type Key = BundleKey;
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

impl CacheObject for TxCacheBundlesResponse {
    type Key = BundleKey;
}

impl TxCacheBundlesResponse {
    /// Create a new bundle response from a list of bundles.
    pub const fn new(bundles: Vec<TxCacheBundle>) -> Self {
        Self { bundles }
    }

    /// Create a new bundle response from a list of bundles.
    #[deprecated = "Use `From::from` instead, `Self::new` in const contexts"]
    pub const fn from_bundles(bundles: Vec<TxCacheBundle>) -> Self {
        Self { bundles }
    }

    /// Convert the bundle response to a list of [`SignetEthBundle`].
    #[deprecated = "Use `this.bundles` instead."]
    pub fn into_bundles(self) -> Vec<TxCacheBundle> {
        self.bundles
    }

    /// Check if the response is empty (has no bundles).
    pub fn is_empty(&self) -> bool {
        self.bundles.is_empty()
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

impl CacheObject for TxCacheSendBundleResponse {
    type Key = BundleKey;
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

impl CacheObject for TxCacheTransactionsResponse {
    type Key = TxKey;
}

impl TxCacheTransactionsResponse {
    /// Instantiate a new transaction response from a list of transactions.
    pub const fn new(transactions: Vec<TxEnvelope>) -> Self {
        Self { transactions }
    }

    /// Create a new transaction response from a list of transactions.
    #[deprecated = "Use `From::from` instead, or `Self::new` in const contexts"]
    pub const fn from_transactions(transactions: Vec<TxEnvelope>) -> Self {
        Self { transactions }
    }

    /// Convert the transaction response to a list of [`TxEnvelope`].
    #[deprecated = "Use `this.transactions` instead."]
    pub fn into_transactions(self) -> Vec<TxEnvelope> {
        self.transactions
    }

    /// Check if the response is empty (has no transactions).
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
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

impl CacheObject for TxCacheSendTransactionResponse {
    type Key = TxKey;
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

impl CacheObject for TxCacheOrdersResponse {
    type Key = OrderKey;
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

/// Response from the transaction cache to successfully adding an order.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TxCacheSendOrderResponse {
    /// The order id
    pub id: B256,
}

impl From<B256> for TxCacheSendOrderResponse {
    fn from(id: B256) -> Self {
        Self { id }
    }
}

impl From<TxCacheSendOrderResponse> for B256 {
    fn from(response: TxCacheSendOrderResponse) -> Self {
        response.id
    }
}

impl CacheObject for TxCacheSendOrderResponse {
    type Key = OrderKey;
}

impl TxCacheSendOrderResponse {
    /// Create a new order response from an order id.
    pub const fn new(id: B256) -> Self {
        Self { id }
    }
}

/// Represents the pagination information from a transaction cache response.
/// This applies to all GET endpoints that return a list of items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo<C: CursorKey> {
    /// The next cursor.
    next_cursor: Option<C>,
    /// Whether there is a next page.
    has_next_page: bool,
}

impl<C: CursorKey> PaginationInfo<C> {
    /// Create a new [`PaginationInfo`].
    pub const fn new(next_cursor: Option<C>, has_next_page: bool) -> Self {
        Self { next_cursor, has_next_page }
    }

    /// Create an empty [`PaginationInfo`].
    pub const fn empty() -> Self {
        Self { next_cursor: None, has_next_page: false }
    }

    /// Get the next cursor.
    pub const fn next_cursor(&self) -> Option<&C> {
        self.next_cursor.as_ref()
    }

    /// Consume the [`PaginationInfo`] and return the next cursor.
    pub fn into_next_cursor(self) -> Option<C> {
        self.next_cursor
    }

    /// Check if there is a next page in the response.
    pub const fn has_next_page(&self) -> bool {
        self.has_next_page
    }
}

/// The query object keys for the transaction GET endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxKey {
    /// The transaction hash    
    pub txn_hash: B256,
    /// The transaction score
    pub score: u64,
    /// The global transaction score key
    pub global_transaction_score_key: String,
}

impl CursorKey for TxKey {
    fn to_query_object(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("txn_hash".to_string(), self.txn_hash.to_string());
        map.insert("score".to_string(), self.score.to_string());
        map.insert(
            "global_transaction_score_key".to_string(),
            self.global_transaction_score_key.to_string(),
        );
        map
    }
}

/// The query object keys for the bundle GET endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleKey {
    /// The bundle id
    pub id: Uuid,
    /// The bundle score
    pub score: u64,
    /// The global bundle score key
    pub global_bundle_score_key: String,
}

impl CursorKey for BundleKey {
    fn to_query_object(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("id".to_string(), self.id.to_string());
        map.insert("score".to_string(), self.score.to_string());
        map.insert("global_bundle_score_key".to_string(), self.global_bundle_score_key.to_string());
        map
    }
}

/// The query object keys for the order GET endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderKey {
    /// The order id
    pub id: String,
}

impl CursorKey for OrderKey {
    fn to_query_object(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("id".to_string(), self.id.to_string());
        map
    }
}

/// A query for pagination.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PaginationParams<C: CursorKey> {
    /// The cursor to start from.
    cursor: C,
}

impl<C: CursorKey> PaginationParams<C> {
    /// Creates a new instance of [`PaginationParams`].
    pub const fn new(cursor: C) -> Self {
        Self { cursor }
    }

    /// Get the cursor to start from.
    pub const fn cursor(&self) -> &C {
        &self.cursor
    }

    /// Consumes the [`PaginationParams`] and returns the cursor.
    pub fn into_cursor(self) -> C {
        self.cursor
    }
}

impl<C: CursorKey> CursorKey for PaginationParams<C> {
    fn to_query_object(&self) -> HashMap<String, String> {
        self.cursor.to_query_object()
    }
}
