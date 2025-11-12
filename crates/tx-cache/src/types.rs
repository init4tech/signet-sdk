//! The endpoints for the transaction cache.
use alloy::{consensus::TxEnvelope, primitives::B256};
use serde::{
    de::{DeserializeOwned, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use signet_bundle::SignetEthBundle;
use signet_types::SignedOrder;
use uuid::Uuid;

/// A trait for types that can be used as a cache object.
pub trait CacheObject {
    /// The cursor key type for the cache object.
    type Key: Serialize + DeserializeOwned;
}

/// A response from the transaction cache, containing an item.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CacheResponse<T: CacheObject> {
    /// A paginated response, containing the inner item and a next cursor.
    Paginated {
        /// The actual item.
        #[serde(flatten)]
        inner: T,
        /// The next cursor for pagination, if any.
        next_cursor: Option<T::Key>,
    },
    /// An unpaginated response, containing the actual item.
    Unpaginated {
        /// The actual item.
        #[serde(flatten)]
        inner: T,
    },
}

impl<T: CacheObject> CacheObject for CacheResponse<T> {
    type Key = T::Key;
}

impl<T: CacheObject> CacheResponse<T> {
    /// Create a new paginated response from a list of items and a pagination info.
    pub const fn paginated(inner: T, pagination: T::Key) -> Self {
        Self::Paginated { inner, next_cursor: Some(pagination) }
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

    /// Return the next cursor for pagination, if any.
    pub const fn next_cursor(&self) -> Option<&T::Key> {
        match self {
            Self::Paginated { next_cursor, .. } => next_cursor.as_ref(),
            Self::Unpaginated { .. } => None,
        }
    }

    /// Check if the response has more items to fetch.
    pub const fn has_more(&self) -> bool {
        self.next_cursor().is_some()
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
    pub fn into_parts(self) -> (T, Option<T::Key>) {
        match self {
            Self::Paginated { inner, next_cursor } => (inner, next_cursor),
            Self::Unpaginated { inner } => (inner, None),
        }
    }

    /// Consume the response and return the next cursor for pagination, if any.
    pub fn into_next_cursor(self) -> Option<T::Key> {
        self.into_parts().1
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

/// A query for pagination.
#[derive(Clone, Debug, Serialize)]
pub struct PaginationParams<C: Serialize + for<'a> Deserialize<'a>> {
    /// The cursor to start from.
    #[serde(flatten)]
    cursor: Option<C>,
}

impl<C: Serialize + for<'a> Deserialize<'a>> PaginationParams<C> {
    /// Creates a new instance of [`PaginationParams`].
    pub const fn new(cursor: Option<C>) -> Self {
        Self { cursor }
    }

    /// Get the cursor to start from.
    pub const fn cursor(&self) -> Option<&C> {
        self.cursor.as_ref()
    }

    /// Consumes the [`PaginationParams`] and returns the cursor.
    pub fn into_cursor(self) -> Option<C> {
        self.cursor
    }
}

impl<'de> Deserialize<'de> for PaginationParams<TxKey> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            TxnHash,
            Score,
            GlobalTransactionScoreKey,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct TxKeyVisitor;

                impl<'de> serde::de::Visitor<'de> for TxKeyVisitor {
                    type Value = Field;

                    fn expecting(
                        &self,
                        formatter: &mut std::fmt::Formatter<'_>,
                    ) -> std::fmt::Result {
                        formatter.write_str("a TxKeyField")
                    }

                    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        match v {
                            "txnHash" => Ok(Field::TxnHash),
                            "score" => Ok(Field::Score),
                            "globalTransactionScoreKey" => Ok(Field::GlobalTransactionScoreKey),
                            _ => Err(serde::de::Error::unknown_field(v, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_str(TxKeyVisitor)
            }
        }

        struct TxKeyVisitor;

        impl<'de> Visitor<'de> for TxKeyVisitor {
            type Value = PaginationParams<TxKey>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a PaginationParams<TxKey>")
            }

            fn visit_seq<S>(self, mut seq: S) -> Result<PaginationParams<TxKey>, S::Error>
            where
                S: SeqAccess<'de>,
            {
                // We consider this a complete request if we have no elements in the sequence.
                let Some(txn_hash) = seq.next_element()? else {
                    // We consider this a complete request if we have no txn hash.
                    return Ok(PaginationParams::new(None));
                };

                // For all other items, we require a score and a global transaction score key.
                let score = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                let global_transaction_score_key = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                Ok(PaginationParams::new(Some(TxKey {
                    txn_hash,
                    score,
                    global_transaction_score_key,
                })))
            }

            fn visit_map<M>(self, mut map: M) -> Result<PaginationParams<TxKey>, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut txn_hash = None;
                let mut score = None;
                let mut global_transaction_score_key = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::TxnHash => {
                            if txn_hash.is_some() {
                                return Err(serde::de::Error::duplicate_field("txnHash"));
                            }
                            txn_hash = Some(map.next_value()?);
                        }
                        Field::Score => {
                            if score.is_some() {
                                return Err(serde::de::Error::duplicate_field("score"));
                            }
                            score = Some(map.next_value()?);
                        }
                        Field::GlobalTransactionScoreKey => {
                            if global_transaction_score_key.is_some() {
                                return Err(serde::de::Error::duplicate_field(
                                    "globalTransactionScoreKey",
                                ));
                            }
                            global_transaction_score_key = Some(map.next_value()?);
                        }
                    }
                }

                // We consider this a complete request if we have no txn hash and no other fields are present.
                let txn_hash = match txn_hash {
                    Some(hash) => hash,
                    None => {
                        if score.is_some() || global_transaction_score_key.is_some() {
                            return Err(serde::de::Error::invalid_length(
                                score.is_some() as usize
                                    + global_transaction_score_key.is_some() as usize,
                                &self,
                            ));
                        }
                        return Ok(PaginationParams::new(None));
                    }
                };

                // For all other items, we require a score and a global transaction score key.
                let score = score.ok_or_else(|| serde::de::Error::missing_field("score"))?;
                let global_transaction_score_key = global_transaction_score_key
                    .ok_or_else(|| serde::de::Error::missing_field("globalTransactionScoreKey"))?;

                Ok(PaginationParams::new(Some(TxKey {
                    txn_hash,
                    score,
                    global_transaction_score_key,
                })))
            }
        }

        const FIELDS: &[&str] = &["txnHash", "score", "globalTransactionScoreKey"];
        deserializer.deserialize_struct("TxKey", FIELDS, TxKeyVisitor)
    }
}

impl<'de> Deserialize<'de> for PaginationParams<BundleKey> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        {
            enum Field {
                Id,
                Score,
                GlobalBundleScoreKey,
            }

            impl<'de> Deserialize<'de> for Field {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    struct BundleKeyVisitor;

                    impl<'de> serde::de::Visitor<'de> for BundleKeyVisitor {
                        type Value = Field;

                        fn expecting(
                            &self,
                            formatter: &mut std::fmt::Formatter<'_>,
                        ) -> std::fmt::Result {
                            formatter.write_str("a BundleKeyField")
                        }

                        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                        where
                            E: serde::de::Error,
                        {
                            match v {
                                "id" => Ok(Field::Id),
                                "score" => Ok(Field::Score),
                                "globalBundleScoreKey" => Ok(Field::GlobalBundleScoreKey),
                                _ => Err(serde::de::Error::unknown_field(v, FIELDS)),
                            }
                        }
                    }

                    deserializer.deserialize_str(BundleKeyVisitor)
                }
            }

            struct BundleKeyVisitor;

            impl<'de> Visitor<'de> for BundleKeyVisitor {
                type Value = PaginationParams<BundleKey>;

                fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str("a PaginationParams<BundleKey>")
                }

                fn visit_seq<S>(self, mut seq: S) -> Result<PaginationParams<BundleKey>, S::Error>
                where
                    S: SeqAccess<'de>,
                {
                    // We consider this a complete request if we have no elements in the sequence.
                    let Some(id) = seq.next_element()? else {
                        return Ok(PaginationParams::new(None));
                    };

                    // For all other items, we require a score and a global transaction score key.
                    let score = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                    let global_bundle_score_key = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                    Ok(PaginationParams::new(Some(BundleKey {
                        id,
                        score,
                        global_bundle_score_key,
                    })))
                }

                fn visit_map<M>(self, mut map: M) -> Result<PaginationParams<BundleKey>, M::Error>
                where
                    M: MapAccess<'de>,
                {
                    let mut id = None;
                    let mut score = None;
                    let mut global_bundle_score_key = None;

                    while let Some(key) = map.next_key()? {
                        match key {
                            Field::Id => {
                                if id.is_some() {
                                    return Err(serde::de::Error::duplicate_field("id"));
                                }
                                id = Some(map.next_value()?);
                            }
                            Field::Score => {
                                if score.is_some() {
                                    return Err(serde::de::Error::duplicate_field("score"));
                                }
                                score = Some(map.next_value()?);
                            }
                            Field::GlobalBundleScoreKey => {
                                if global_bundle_score_key.is_some() {
                                    return Err(serde::de::Error::duplicate_field(
                                        "globalBundleScoreKey",
                                    ));
                                }
                                global_bundle_score_key = Some(map.next_value()?);
                            }
                        }
                    }

                    // We consider this a complete request if we have no id and no other fields are present.
                    let Some(id) = id else {
                        if score.is_some() || global_bundle_score_key.is_some() {
                            return Err(serde::de::Error::invalid_length(
                                score.is_some() as usize
                                    + global_bundle_score_key.is_some() as usize,
                                &self,
                            ));
                        }
                        return Ok(PaginationParams::new(None));
                    };

                    // For all other items, we require a score and a global bundle score key.
                    let score = score.ok_or_else(|| serde::de::Error::missing_field("score"))?;
                    let global_bundle_score_key = global_bundle_score_key
                        .ok_or_else(|| serde::de::Error::missing_field("globalBundleScoreKey"))?;
                    Ok(PaginationParams::new(Some(BundleKey {
                        id,
                        score,
                        global_bundle_score_key,
                    })))
                }
            }

            const FIELDS: &[&str] = &["id", "score", "globalBundleScoreKey"];
            deserializer.deserialize_struct("BundleKey", FIELDS, BundleKeyVisitor)
        }
    }
}

impl<'de> Deserialize<'de> for PaginationParams<OrderKey> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        {
            enum Field {
                Id,
            }

            impl<'de> Deserialize<'de> for Field {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: Deserializer<'de>,
                {
                    struct OrderKeyVisitor;

                    impl<'de> serde::de::Visitor<'de> for OrderKeyVisitor {
                        type Value = Field;

                        fn expecting(
                            &self,
                            formatter: &mut std::fmt::Formatter<'_>,
                        ) -> std::fmt::Result {
                            formatter.write_str("a OrderKeyField")
                        }

                        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                        where
                            E: serde::de::Error,
                        {
                            match v {
                                "id" => Ok(Field::Id),
                                _ => Err(serde::de::Error::unknown_field(v, FIELDS)),
                            }
                        }
                    }

                    deserializer.deserialize_str(OrderKeyVisitor)
                }
            }

            struct OrderKeyVisitor;

            impl<'de> Visitor<'de> for OrderKeyVisitor {
                type Value = PaginationParams<OrderKey>;

                fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    formatter.write_str("a PaginationParams<OrderKey>")
                }

                fn visit_seq<S>(self, mut seq: S) -> Result<PaginationParams<OrderKey>, S::Error>
                where
                    S: SeqAccess<'de>,
                {
                    let Some(id) = seq.next_element()? else {
                        return Ok(PaginationParams::new(None));
                    };

                    Ok(PaginationParams::new(Some(OrderKey { id })))
                }

                fn visit_map<M>(self, mut map: M) -> Result<PaginationParams<OrderKey>, M::Error>
                where
                    M: MapAccess<'de>,
                {
                    let mut id = None;

                    while let Some(key) = map.next_key()? {
                        match key {
                            Field::Id => {
                                if id.is_some() {
                                    return Err(serde::de::Error::duplicate_field("id"));
                                }
                                id = Some(map.next_value()?);
                            }
                        }
                    }

                    let Some(id) = id else {
                        return Ok(PaginationParams::new(None));
                    };

                    Ok(PaginationParams::new(Some(OrderKey { id })))
                }
            }

            const FIELDS: &[&str] = &["id"];
            deserializer.deserialize_struct("OrderKey", FIELDS, OrderKeyVisitor)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_pagination_params_simple_deser() {
        let tx_key = TxKey {
            txn_hash: B256::repeat_byte(0xaa),
            score: 100,
            global_transaction_score_key: "gtsk".to_string(),
        };
        let params = PaginationParams::<TxKey>::new(Some(tx_key));
        let empty_params = PaginationParams::<TxKey>::new(None);

        let serialized = serde_urlencoded::to_string(&params).unwrap();
        let empty_serialized = serde_urlencoded::to_string(&empty_params).unwrap();
        assert_eq!(serialized, "txnHash=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&score=100&globalTransactionScoreKey=gtsk");
        assert_eq!(empty_serialized, "");
    }

    #[test]
    fn test_pagination_params_partial_deser() {
        let tx_key = TxKey {
            txn_hash: B256::repeat_byte(0xaa),
            score: 100,
            global_transaction_score_key: "gtsk".to_string(),
        };
        let params = PaginationParams::<TxKey>::new(Some(tx_key.clone()));
        let serialized = serde_urlencoded::to_string(&params).unwrap();
        assert_eq!(serialized, "txnHash=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&score=100&globalTransactionScoreKey=gtsk");

        let deserialized =
            serde_urlencoded::from_str::<PaginationParams<TxKey>>(&serialized).unwrap();
        assert_eq!(deserialized.cursor().unwrap(), &tx_key);

        let partial_query_string = "score=100&globalTransactionScoreKey=gtsk";
        let partial_params =
            serde_urlencoded::from_str::<PaginationParams<TxKey>>(partial_query_string);
        assert!(partial_params.is_err());

        let partial_query_string =
            "txnHash=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa&score=100";
        let partial_params =
            serde_urlencoded::from_str::<PaginationParams<TxKey>>(partial_query_string);
        assert!(partial_params.is_err());

        let empty_query_string = "";
        let empty_params =
            serde_urlencoded::from_str::<PaginationParams<TxKey>>(empty_query_string);
        assert!(empty_params.is_ok());
        assert!(empty_params.unwrap().cursor().is_none());
    }

    #[test]
    fn test_pagination_params_bundle_deser() {
        let bundle_key = BundleKey {
            // This is our UUID. Nobody else use it.
            id: Uuid::from_str("5932d4bb-58d9-41a9-851d-8dd7f04ccc33").unwrap(),
            score: 100,
            global_bundle_score_key: "gbsk".to_string(),
        };

        let params = PaginationParams::<BundleKey>::new(Some(bundle_key.clone()));
        let serialized = serde_urlencoded::to_string(&params).unwrap();
        assert_eq!(
            serialized,
            "id=5932d4bb-58d9-41a9-851d-8dd7f04ccc33&score=100&globalBundleScoreKey=gbsk"
        );

        let deserialized =
            serde_urlencoded::from_str::<PaginationParams<BundleKey>>(&serialized).unwrap();
        assert_eq!(deserialized.cursor().unwrap(), &bundle_key);

        let partial_query_string = "score=100&globalBundleScoreKey=gbsk";
        let partial_params =
            serde_urlencoded::from_str::<PaginationParams<BundleKey>>(partial_query_string);
        assert!(partial_params.is_err());

        let partial_query_string = "id=5932d4bb-58d9-41a9-851d-8dd7f04ccc33&score=100";
        let partial_params =
            serde_urlencoded::from_str::<PaginationParams<BundleKey>>(partial_query_string);
        assert!(partial_params.is_err());

        let empty_query_string = "";
        let empty_params =
            serde_urlencoded::from_str::<PaginationParams<BundleKey>>(empty_query_string);
        assert!(empty_params.is_ok());
        assert!(empty_params.unwrap().cursor().is_none());
    }

    #[test]
    fn test_pagination_params_order_deser() {
        let order_key = OrderKey { id: B256::repeat_byte(0xaa) };

        let params = PaginationParams::<OrderKey>::new(Some(order_key));
        let serialized = serde_urlencoded::to_string(&params).unwrap();
        assert_eq!(
            serialized,
            "id=0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );

        let deserialized =
            serde_urlencoded::from_str::<PaginationParams<OrderKey>>(&serialized).unwrap();
        assert_eq!(deserialized.cursor().unwrap(), &order_key);
    }
}
