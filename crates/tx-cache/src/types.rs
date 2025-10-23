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
    /// The pagination info.
    pub pagination: PaginationInfo,
}

impl From<Vec<TxCacheBundle>> for TxCacheBundlesResponse {
    fn from(bundles: Vec<TxCacheBundle>) -> Self {
        Self { bundles, pagination: PaginationInfo::empty() }
    }
}

impl From<TxCacheBundlesResponse> for Vec<TxCacheBundle> {
    fn from(response: TxCacheBundlesResponse) -> Self {
        response.bundles
    }
}

impl From<(Vec<TxCacheBundle>, PaginationInfo)> for TxCacheBundlesResponse {
    fn from((bundles, pagination): (Vec<TxCacheBundle>, PaginationInfo)) -> Self {
        Self { bundles, pagination }
    }
}

impl TxCacheBundlesResponse {
    /// Create a new bundle response from a list of bundles.
    pub const fn new(bundles: Vec<TxCacheBundle>) -> Self {
        Self { bundles, pagination: PaginationInfo::empty() }
    }

    /// Create a new bundle response from a list of bundles.
    #[deprecated = "Use `From::from` instead, `Self::new` in const contexts"]
    pub const fn from_bundles(bundles: Vec<TxCacheBundle>) -> Self {
        Self { bundles, pagination: PaginationInfo::empty() }
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

    /// Check if there is a next page in the response.
    pub const fn has_next_page(&self) -> bool {
        self.pagination.has_next_page()
    }

    /// Get the cursor for the next page.
    pub fn next_cursor(&self) -> Option<&str> {
        self.pagination.next_cursor()
    }

    /// Consume the response and return the next cursor.
    pub fn into_next_cursor(self) -> Option<String> {
        self.pagination.into_next_cursor()
    }

    /// Consume the response and return the parts.
    pub fn into_parts(self) -> (Vec<TxCacheBundle>, PaginationInfo) {
        (self.bundles, self.pagination)
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
    /// The pagination info.
    pub pagination: PaginationInfo,
}

impl From<Vec<TxEnvelope>> for TxCacheTransactionsResponse {
    fn from(transactions: Vec<TxEnvelope>) -> Self {
        Self { transactions, pagination: PaginationInfo::empty() }
    }
}

impl From<TxCacheTransactionsResponse> for Vec<TxEnvelope> {
    fn from(response: TxCacheTransactionsResponse) -> Self {
        response.transactions
    }
}

impl From<(Vec<TxEnvelope>, PaginationInfo)> for TxCacheTransactionsResponse {
    fn from((transactions, pagination): (Vec<TxEnvelope>, PaginationInfo)) -> Self {
        Self { transactions, pagination }
    }
}

impl TxCacheTransactionsResponse {
    /// Instantiate a new transaction response from a list of transactions.
    pub const fn new(transactions: Vec<TxEnvelope>) -> Self {
        Self { transactions, pagination: PaginationInfo::empty() }
    }

    /// Create a new transaction response from a list of transactions.
    #[deprecated = "Use `From::from` instead, or `Self::new` in const contexts"]
    pub const fn from_transactions(transactions: Vec<TxEnvelope>) -> Self {
        Self { transactions, pagination: PaginationInfo::empty() }
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

    /// Check if there is a next page in the response.
    pub const fn has_next_page(&self) -> bool {
        self.pagination.has_next_page()
    }

    /// Get the cursor for the next page.
    pub fn next_cursor(&self) -> Option<&str> {
        self.pagination.next_cursor()
    }

    /// Consume the response and return the next cursor.
    pub fn into_next_cursor(self) -> Option<String> {
        self.pagination.into_next_cursor()
    }

    /// Consume the response and return the parts.
    pub fn into_parts(self) -> (Vec<TxEnvelope>, PaginationInfo) {
        (self.transactions, self.pagination)
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
    /// The pagination info.
    pub pagination: PaginationInfo,
}

impl From<Vec<SignedOrder>> for TxCacheOrdersResponse {
    fn from(orders: Vec<SignedOrder>) -> Self {
        Self { orders, pagination: PaginationInfo::empty() }
    }
}

impl From<TxCacheOrdersResponse> for Vec<SignedOrder> {
    fn from(response: TxCacheOrdersResponse) -> Self {
        response.orders
    }
}

impl From<(Vec<SignedOrder>, PaginationInfo)> for TxCacheOrdersResponse {
    fn from((orders, pagination): (Vec<SignedOrder>, PaginationInfo)) -> Self {
        Self { orders, pagination }
    }
}

impl TxCacheOrdersResponse {
    /// Create a new order response from a list of orders.
    pub const fn new(orders: Vec<SignedOrder>) -> Self {
        Self { orders, pagination: PaginationInfo::empty() }
    }

    /// Create a new order response from a list of orders.
    #[deprecated = "Use `From::from` instead, `Self::new` in const contexts"]
    pub const fn from_orders(orders: Vec<SignedOrder>) -> Self {
        Self { orders, pagination: PaginationInfo::empty() }
    }

    /// Convert the order response to a list of [`SignedOrder`].
    #[deprecated = "Use `this.orders` instead."]
    pub fn into_orders(self) -> Vec<SignedOrder> {
        self.orders
    }

    /// Check if there is a next page in the response.
    pub const fn has_next_page(&self) -> bool {
        self.pagination.has_next_page()
    }

    /// Get the cursor for the next page.
    pub fn next_cursor(&self) -> Option<&str> {
        self.pagination.next_cursor()
    }

    /// Consume the response and return the next cursor.
    pub fn into_next_cursor(self) -> Option<String> {
        self.pagination.into_next_cursor()
    }

    /// Consume the response and return the parts.
    pub fn into_parts(self) -> (Vec<SignedOrder>, PaginationInfo) {
        (self.orders, self.pagination)
    }
}

/// Represents the pagination information from a transaction cache response.
/// This applies to all GET endpoints that return a list of items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationInfo {
    next_cursor: Option<String>,
    has_next_page: bool,
}

impl PaginationInfo {
    /// Create a new [`PaginationInfo`].
    pub const fn new(next_cursor: Option<String>, has_next_page: bool) -> Self {
        Self { next_cursor, has_next_page }
    }

    /// Create an empty [`PaginationInfo`].
    pub const fn empty() -> Self {
        Self { next_cursor: None, has_next_page: false }
    }

    /// Get the next cursor.
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }

    /// Consume the [`PaginationInfo`] and return the next cursor.
    pub fn into_next_cursor(self) -> Option<String> {
        self.next_cursor
    }

    /// Check if there is a next page in the response.
    pub const fn has_next_page(&self) -> bool {
        self.has_next_page
    }
}

/// A query for pagination.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PaginationParams {
    /// The cursor to start from.
    cursor: Option<String>,
    /// The number of items to return.
    limit: Option<u32>,
}

impl From<PaginationInfo> for PaginationParams {
    fn from(info: PaginationInfo) -> Self {
        Self { cursor: info.into_next_cursor(), limit: None }
    }
}

impl PaginationParams {
    /// Creates a new instance of [`PaginationParams`].
    pub const fn new(cursor: Option<String>, limit: Option<u32>) -> Self {
        Self { cursor, limit }
    }

    /// Get the cursor to start from.
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    /// Consumes the [`PaginationParams`] and returns the cursor.
    pub fn into_cursor(self) -> Option<String> {
        self.cursor
    }

    /// Get the number of items to return.
    pub const fn limit(&self) -> Option<u32> {
        self.limit
    }

    /// Check if the query has a cursor.
    pub const fn has_cursor(&self) -> bool {
        self.cursor.is_some()
    }

    /// Check if the query has a limit.
    pub const fn has_limit(&self) -> bool {
        self.limit.is_some()
    }

    /// Check if the query is empty (has no cursor and no limit).
    pub const fn is_empty(&self) -> bool {
        !self.has_cursor() && !self.has_limit()
    }
}
