//! The endpoints for the transaction cache.
use alloy::{consensus::TxEnvelope, primitives::B256};
use core::ops::{Deref, DerefMut};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use signet_bundle::SignetEthBundle;
use signet_types::SignedOrder;
use uuid::Uuid;

/// A trait for types that can be used as a cache object.
pub trait CacheObject {
    /// The cursor key type for the cache object.
    type Key: Serialize + DeserializeOwned;
}

/// A response from the transaction cache, containing an item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CacheResponse<T: CacheObject> {
    /// The response.
    #[serde(flatten)]
    inner: T,
    /// The next cursor for pagination, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    next_cursor: Option<T::Key>,
}

impl<T: CacheObject> CacheObject for CacheResponse<T> {
    type Key = T::Key;
}

impl<T: CacheObject> Deref for CacheResponse<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T: CacheObject> DerefMut for CacheResponse<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: CacheObject> CacheResponse<T> {
    /// Create a new paginated response from a list of items and a pagination info.
    pub const fn paginated(inner: T, pagination: T::Key) -> Self {
        Self { inner, next_cursor: Some(pagination) }
    }

    /// Create a new unpaginated response from a list of items.
    pub const fn unpaginated(inner: T) -> Self {
        Self { inner, next_cursor: None }
    }

    /// Return a reference to the inner value.
    #[deprecated = "use deref instead"]
    pub const fn inner(&self) -> &T {
        match self {
            Self { inner, .. } => inner,
        }
    }

    /// Return a mutable reference to the inner value.
    #[deprecated = "use deref_mut instead"]
    pub const fn inner_mut(&mut self) -> &mut T {
        match self {
            Self { inner, .. } => inner,
        }
    }

    /// Return the next cursor for pagination, if any.
    pub const fn next_cursor(&self) -> Option<&T::Key> {
        match self {
            Self { next_cursor, .. } => next_cursor.as_ref(),
        }
    }

    /// Check if the response has more items to fetch.
    pub const fn has_more(&self) -> bool {
        self.next_cursor().is_some()
    }

    /// Check if the response is paginated.
    pub const fn is_paginated(&self) -> bool {
        self.next_cursor.is_some()
    }

    /// Check if the response is unpaginated.
    pub const fn is_unpaginated(&self) -> bool {
        self.next_cursor.is_none()
    }

    /// Get the inner value.
    pub fn into_inner(self) -> T {
        match self {
            Self { inner, .. } => inner,
        }
    }

    /// Consume the response and return the parts.
    pub fn into_parts(self) -> (T, Option<T::Key>) {
        match self {
            Self { inner, next_cursor } => (inner, next_cursor),
        }
    }

    /// Consume the response and return the next cursor for pagination, if any.
    pub fn into_next_cursor(self) -> Option<T::Key> {
        self.into_parts().1
    }
}

/// A bundle response from the transaction cache, containing a UUID and a
/// [`SignetEthBundle`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub const fn is_empty(&self) -> bool {
        self.bundles.is_empty()
    }
}

/// Represents a response to successfully adding or updating a bundle in the transaction cache.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    pub const fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

/// Response from the transaction cache to successfully adding a transaction.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

/// The query object keys for the transaction GET endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TxKey {
    /// The transaction hash
    pub txn_hash: B256,
    /// The transaction score
    pub score: u64,
    /// The global transaction score key
    pub global_transaction_score_key: String,
}

/// The query object keys for the bundle GET endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleKey {
    /// The bundle id
    pub id: Uuid,
    /// The bundle score
    pub score: u64,
    /// The global bundle score key
    pub global_bundle_score_key: String,
}

/// The query object keys for the order GET endpoint.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderKey {
    /// The order id
    pub id: B256,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dummy_bundle_with_id(id: Uuid) -> TxCacheBundle {
        TxCacheBundle {
            id,
            bundle: SignetEthBundle {
                bundle: alloy::rpc::types::mev::EthSendBundle {
                    txs: vec![],
                    block_number: 0,
                    min_timestamp: None,
                    max_timestamp: None,
                    reverting_tx_hashes: vec![],
                    replacement_uuid: Some(id.to_string()),
                    dropping_tx_hashes: vec![],
                    refund_percent: None,
                    refund_recipient: None,
                    refund_tx_hashes: vec![],
                    extra_fields: Default::default(),
                },
                host_txs: vec![],
            },
        }
    }

    #[test]
    fn test_unpaginated_cache_response_deser() {
        let cache_response =
            CacheResponse::unpaginated(TxCacheTransactionsResponse { transactions: vec![] });
        let expected_json = r#"{"transactions":[]}"#;
        let serialized = serde_json::to_string(&cache_response).unwrap();
        assert_eq!(serialized, expected_json);
        let deserialized =
            serde_json::from_str::<CacheResponse<TxCacheTransactionsResponse>>(&serialized)
                .unwrap();
        assert_eq!(deserialized, cache_response);
    }

    #[test]
    fn test_paginated_cache_response_deser() {
        let cache_response = CacheResponse::paginated(
            TxCacheTransactionsResponse { transactions: vec![] },
            TxKey {
                txn_hash: B256::repeat_byte(0xaa),
                score: 100,
                global_transaction_score_key: "gtsk".to_string(),
            },
        );
        let expected_json = r#"{"transactions":[],"nextCursor":{"txnHash":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","score":100,"globalTransactionScoreKey":"gtsk"}}"#;
        let serialized = serde_json::to_string(&cache_response).unwrap();
        assert_eq!(serialized, expected_json);
        let deserialized =
            serde_json::from_str::<CacheResponse<TxCacheTransactionsResponse>>(expected_json)
                .unwrap();
        assert_eq!(deserialized, cache_response);
    }

    // `serde_json` should be able to deserialize the old format, regardless if there's pagination information on the response.
    // This mimics the behavior of the types pre-pagination.
    #[test]
    fn test_backwards_compatibility_cache_response_deser() {
        let expected_json = r#"{"transactions":[],"nextCursor":{"txnHash":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","score":100,"globalTransactionScoreKey":"gtsk"}}"#;
        let deserialized =
            serde_json::from_str::<TxCacheTransactionsResponse>(expected_json).unwrap();
        assert_eq!(deserialized, TxCacheTransactionsResponse { transactions: vec![] });
    }

    // `serde_json` should be able to deserialize the old format, regardless if there's pagination information on the response.
    // This mimics the behavior of the types pre-pagination.
    #[test]
    fn test_backwards_compatibility_cache_bundle_response_deser() {
        let expected_json = r#"{"bundles":[{"id":"5932d4bb-58d9-41a9-851d-8dd7f04ccc33","bundle":{"txs":[],"blockNumber":"0x0","replacementUuid":"5932d4bb-58d9-41a9-851d-8dd7f04ccc33"}}]}"#;
        let uuid = Uuid::from_str("5932d4bb-58d9-41a9-851d-8dd7f04ccc33").unwrap();

        let deserialized = serde_json::from_str::<TxCacheBundlesResponse>(expected_json).unwrap();

        assert_eq!(
            deserialized,
            TxCacheBundlesResponse { bundles: vec![dummy_bundle_with_id(uuid)] }
        );
    }

    // `serde_json` should be able to deserialize the old format, regardless if there's pagination information on the response.
    // This mimics the behavior of the types pre-pagination.
    #[test]
    fn test_backwards_compatibility_cache_order_response_deser() {
        let expected_json = r#"{"orders":[{"permit":{"permitted":[{"token":"0x0b8bc5e60ee10957e0d1a0d95598fa63e65605e2","amount":"0xf4240"}],"nonce":"0x637253c1eb651","deadline":"0x6846fde6"},"owner":"0x492e9c316f073fe4de9d665221568cdad1a7e95b","signature":"0x73e31a7c80f02840c4e0671230c408a5cbc7cddefc780db4dd102eed8e87c5740fc89944eb8e5756edd368ed755415ed090b043d1740ee6869c20cb1676329621c","outputs":[{"token":"0x885f8db528dc8a38aa3ddad9d3f619746b4a6a81","amount":"0xf4240","recipient":"0x492e9c316f073fe4de9d665221568cdad1a7e95b","chainId":3151908}]}], "id":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#;
        let _ = serde_json::from_str::<TxCacheOrdersResponse>(expected_json).unwrap();
    }

    #[test]
    fn test_unpaginated_cache_bundle_response_deser() {
        let cache_response = CacheResponse::unpaginated(TxCacheBundlesResponse {
            bundles: vec![dummy_bundle_with_id(
                Uuid::from_str("5932d4bb-58d9-41a9-851d-8dd7f04ccc33").unwrap(),
            )],
        });
        let expected_json = r#"{"bundles":[{"id":"5932d4bb-58d9-41a9-851d-8dd7f04ccc33","bundle":{"txs":[],"blockNumber":"0x0","replacementUuid":"5932d4bb-58d9-41a9-851d-8dd7f04ccc33"}}]}"#;
        let serialized = serde_json::to_string(&cache_response).unwrap();
        assert_eq!(serialized, expected_json);
        let deserialized =
            serde_json::from_str::<CacheResponse<TxCacheBundlesResponse>>(expected_json).unwrap();
        assert_eq!(deserialized, cache_response);
    }

    #[test]
    fn test_paginated_cache_bundle_response_deser() {
        let uuid = Uuid::from_str("5932d4bb-58d9-41a9-851d-8dd7f04ccc33").unwrap();

        let cache_response = CacheResponse::paginated(
            TxCacheBundlesResponse { bundles: vec![dummy_bundle_with_id(uuid)] },
            BundleKey { id: uuid, score: 100, global_bundle_score_key: "gbsk".to_string() },
        );
        let expected_json = r#"{"bundles":[{"id":"5932d4bb-58d9-41a9-851d-8dd7f04ccc33","bundle":{"txs":[],"blockNumber":"0x0","replacementUuid":"5932d4bb-58d9-41a9-851d-8dd7f04ccc33"}}],"nextCursor":{"id":"5932d4bb-58d9-41a9-851d-8dd7f04ccc33","score":100,"globalBundleScoreKey":"gbsk"}}"#;
        let serialized = serde_json::to_string(&cache_response).unwrap();
        assert_eq!(serialized, expected_json);
        let deserialized =
            serde_json::from_str::<CacheResponse<TxCacheBundlesResponse>>(expected_json).unwrap();
        assert_eq!(deserialized, cache_response);
    }

    #[test]
    fn test_pagination_params_simple_deser() {
        let tx_key = TxKey {
            txn_hash: B256::repeat_byte(0xaa),
            score: 100,
            global_transaction_score_key: "gtsk".to_string(),
        };
        let params = tx_key.clone();
        let empty_params: Option<TxKey> = None;

        let serialized = serde_urlencoded::to_string(&params).unwrap();
        let empty_serialized = serde_urlencoded::to_string(&empty_params).unwrap();
        assert_eq!(serialized, "txnHash=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&score=100&globalTransactionScoreKey=gtsk");
        assert_eq!(empty_serialized, "");
    }

    #[test]
    fn test_cache_response_deref() {
        let uuid = Uuid::new_v4();
        let response =
            CacheResponse::unpaginated(TxCacheBundlesResponse::new(vec![dummy_bundle_with_id(
                uuid,
            )]));

        assert_eq!(response.bundles.len(), 1);
        assert_eq!(response.bundles[0].id, uuid);
    }

    #[test]
    fn test_cache_response_deref_mut() {
        let uuid = Uuid::new_v4();
        let mut response =
            CacheResponse::unpaginated(TxCacheBundlesResponse::new(vec![dummy_bundle_with_id(
                uuid,
            )]));

        response.bundles.clear();
        assert!(response.bundles.is_empty());
    }
}
