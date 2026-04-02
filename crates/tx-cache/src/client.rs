use crate::error::Result;
use crate::types::{
    BundleResponse, CacheObject, CacheResponse, OrderKey, OrderList, OrderResponse,
    TransactionList, TransactionResponse, TxKey,
};
use alloy::consensus::TxEnvelope;
use futures_util::future::Either;
use futures_util::stream::{self, Stream, StreamExt};
use serde::{de::DeserializeOwned, Serialize};
use signet_bundle::SignetEthBundle;
use signet_constants::parmigiana;
#[allow(deprecated)]
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
    pub fn new_from_string(url: &str) -> Result<Self> {
        let url = reqwest::Url::parse(url)?;
        Ok(Self::new(url))
    }

    /// Connect to the transaction cache with the Parmigiana URL.
    pub fn parmigiana() -> Self {
        Self::new_from_string(parmigiana::TX_CACHE_URL).expect("parmigiana tx cache URL is invalid")
    }

    /// Create a new cache with the Parmigiana URL and a specific [`reqwest::Client`].
    pub fn parmigiana_with_client(client: reqwest::Client) -> Self {
        Self::new_with_client(
            parmigiana::TX_CACHE_URL.parse().expect("parmigiana tx cache URL is invalid"),
            client,
        )
    }

    /// Connect to the transaction cache with the Pecorino URL.
    #[deprecated(note = "Pecorino is being deprecated in favor of Parmigiana")]
    #[allow(deprecated)]
    pub fn pecorino() -> Self {
        Self::new_from_string(pecorino::TX_CACHE_URL).expect("pecorino tx cache URL is invalid")
    }

    /// Connect to the transaction cache with the Pecorino URL and a specific [`reqwest::Client`].
    #[deprecated(note = "Pecorino is being deprecated in favor of Parmigiana")]
    #[allow(deprecated)]
    pub fn pecorino_with_client(client: reqwest::Client) -> Self {
        Self::new_with_client(
            pecorino::TX_CACHE_URL.parse().expect("pecorino tx cache URL is invalid"),
            client,
        )
    }

    /// Get the client used to send requests
    pub const fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Get the URL of the transaction cache.
    pub const fn url(&self) -> &reqwest::Url {
        &self.url
    }

    async fn forward_inner<T: Serialize + Send, R: DeserializeOwned>(
        &self,
        join: &'static str,
        obj: T,
    ) -> Result<R> {
        self.forward_inner_raw(join, obj)
            .await?
            .error_for_status()?
            .json::<R>()
            .await
            .inspect_err(|e| warn!(%e, "Failed to parse response from transaction cache"))
            .map_err(Into::into)
    }

    async fn forward_inner_raw<T: Serialize + Send>(
        &self,
        join: &'static str,
        obj: T,
    ) -> Result<reqwest::Response> {
        // Append the path to the URL.
        let url = self
            .url
            .join(join)
            .inspect_err(|e| warn!(%e, "Failed to join URL. Not forwarding transaction."))?;

        // Send the object and check for success.
        self.client.post(url).json(&obj).send().await?.error_for_status().map_err(Into::into)
    }

    async fn get_inner<T>(&self, join: &'static str, query: Option<T::Key>) -> Result<T>
    where
        T: DeserializeOwned + CacheObject,
    {
        let url = self
            .url
            .join(join)
            .inspect_err(|e| warn!(%e, "Failed to join URL. Not querying transaction cache."))?;

        self.client
            .get(url)
            .query(&query)
            .send()
            .await
            .inspect_err(|e| warn!(%e, "Failed to get object from transaction cache."))?
            .json::<T>()
            .await
            .map_err(Into::into)
    }

    async fn put_inner<T: Serialize + Send, R: DeserializeOwned>(
        &self,
        path: &str,
        obj: T,
    ) -> Result<R> {
        let url = self
            .url
            .join(path)
            .inspect_err(|e| warn!(%e, "Failed to join URL. Not updating resource."))?;

        self.client
            .put(url)
            .json(&obj)
            .send()
            .await?
            .error_for_status()?
            .json::<R>()
            .await
            .inspect_err(|e| warn!(%e, "Failed to parse response from transaction cache"))
            .map_err(Into::into)
    }

    /// Forward a raw transaction to the transaction cache.
    ///
    /// This method submits a signed transaction envelope to the cache for
    /// inclusion in a future block. The transaction will be validated and
    /// stored, returning a response containing its cache identifier.
    ///
    /// # Arguments
    ///
    /// * `tx` - A signed [`TxEnvelope`] containing the transaction to forward.
    ///
    /// # Returns
    ///
    /// A [`TransactionResponse`] containing the transaction's cache
    /// identifier on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the transaction cache rejects
    /// the transaction.
    #[instrument(skip_all)]
    pub async fn forward_raw_transaction(&self, tx: TxEnvelope) -> Result<TransactionResponse> {
        self.forward_inner(TRANSACTIONS, tx).await
    }

    /// Forward a bundle to the transaction cache.
    ///
    /// This method submits a signed bundle to the cache for inclusion in a
    /// future block. Bundles allow multiple transactions to be submitted
    /// atomically with ordering guarantees.
    ///
    /// # Arguments
    ///
    /// * `bundle` - A [`SignetEthBundle`] containing the bundle to forward.
    ///
    /// # Returns
    ///
    /// A [`BundleResponse`] containing the bundle's cache identifier
    /// (UUID) on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the transaction cache rejects
    /// the bundle.
    #[instrument(skip_all)]
    pub async fn forward_bundle(&self, bundle: SignetEthBundle) -> Result<BundleResponse> {
        self.forward_inner(BUNDLES, bundle).await
    }

    /// Forward a signed order to the transaction cache.
    ///
    /// This method submits a signed order to the cache. Orders represent
    /// user intents that can be filled by solvers or market makers.
    ///
    /// # Arguments
    ///
    /// * `order` - A [`SignedOrder`] containing the order to forward.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the transaction cache rejects
    /// the order.
    #[instrument(skip_all)]
    pub async fn forward_order(&self, order: SignedOrder) -> Result<()> {
        self.forward_inner_raw(ORDERS, order).await.map(drop)
    }

    /// Get transactions from the transaction cache.
    ///
    /// Retrieves transactions from the cache, optionally filtered by a query
    /// key for pagination. When no query is provided, returns the first page
    /// of transactions.
    ///
    /// # Arguments
    ///
    /// * `query` - An optional [`TxKey`] for pagination. Use `None` to get the
    ///   first page, or pass the key from a previous response to get subsequent
    ///   pages.
    ///
    /// # Returns
    ///
    /// A [`CacheResponse`] containing a [`TransactionList`] with
    /// the transactions and pagination information. If more transactions are
    /// available, the response will contain a key to fetch the next page.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    #[instrument(skip_all)]
    pub async fn get_transactions(
        &self,
        query: Option<TxKey>,
    ) -> Result<CacheResponse<TransactionList>> {
        self.get_inner(TRANSACTIONS, query).await
    }

    /// Get signed orders from the transaction cache.
    ///
    /// Retrieves signed orders from the cache, optionally filtered by a query
    /// key for pagination. When no query is provided, returns the first page
    /// of orders.
    ///
    /// # Arguments
    ///
    /// * `query` - An optional [`OrderKey`] for pagination. Use `None` to get
    ///   the first page, or pass the key from a previous response to get
    ///   subsequent pages.
    ///
    /// # Returns
    ///
    /// A [`CacheResponse`] containing an [`OrderList`] with the
    /// orders and pagination information. If more orders are available, the
    /// response will contain a key to fetch the next page.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response cannot be parsed.
    #[instrument(skip_all)]
    pub async fn get_orders(&self, query: Option<OrderKey>) -> Result<CacheResponse<OrderList>> {
        self.get_inner(ORDERS, query).await
    }

    /// Update an existing bundle in the transaction cache.
    ///
    /// This method sends a PUT request to update a bundle that already exists
    /// in the cache. The bundle is identified by its UUID and the entire bundle
    /// content is replaced with the provided data.
    ///
    /// # Arguments
    ///
    /// * `bundle_id` - The UUID of the bundle to update.
    /// * `bundle` - The updated [`SignetEthBundle`] to store.
    ///
    /// # Returns
    ///
    /// A [`BundleResponse`] containing the bundle's UUID on success.
    ///
    /// # Errors
    ///
    /// Returns [`TxCacheError::NotFound`] if the bundle does not exist.
    /// Returns an error if the request fails or the response cannot be parsed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use signet_tx_cache::TxCache;
    /// use signet_bundle::SignetEthBundle;
    ///
    /// async fn example() -> Result<(), signet_tx_cache::TxCacheError> {
    ///     let cache = TxCache::parmigiana();
    ///     let bundle_id = "550e8400-e29b-41d4-a716-446655440000";
    ///     // Create bundle from your transaction data
    ///     let bundle: SignetEthBundle = todo!();
    ///
    ///     let response = cache.update_bundle(bundle_id, bundle).await?;
    ///     println!("Updated bundle: {}", response.id);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`TxCacheError::NotFound`]: crate::error::TxCacheError::NotFound
    #[instrument(skip_all)]
    pub async fn update_bundle(
        &self,
        bundle_id: &str,
        bundle: SignetEthBundle,
    ) -> Result<BundleResponse> {
        let path = format!("{BUNDLES}/{bundle_id}");
        self.put_inner(&path, bundle).await
    }

    /// Stream all transactions from the transaction cache, automatically
    /// paginating through all available pages.
    ///
    /// Returns a [`Stream`] that yields each [`TxEnvelope`] individually,
    /// fetching subsequent pages as needed. The stream ends when no more
    /// pages are available or on the first error (which is yielded before
    /// terminating).
    pub fn stream_transactions(&self) -> impl Stream<Item = Result<TxEnvelope>> + Send + '_ {
        stream::unfold(Some(None), move |cursor| async move {
            let cursor = cursor?;

            match self.get_transactions(cursor).await {
                Ok(response) => {
                    let (inner, next_cursor) = response.into_parts();
                    let txns = stream::iter(inner.transactions).map(Ok);
                    Some((Either::Left(txns), next_cursor.map(Some)))
                }
                Err(error) => Some((Either::Right(stream::once(async { Err(error) })), None)),
            }
        })
        .flatten()
    }

    /// Stream all signed orders from the transaction cache, automatically
    /// paginating through all available pages.
    ///
    /// Returns a [`Stream`] that yields each [`SignedOrder`] individually,
    /// fetching subsequent pages as needed. The stream ends when no more
    /// pages are available or on the first error (which is yielded before
    /// terminating).
    pub fn stream_orders(&self) -> impl Stream<Item = Result<SignedOrder>> + Send + '_ {
        stream::unfold(Some(None), move |cursor| async move {
            let cursor = cursor?;

            match self.get_orders(cursor).await {
                Ok(response) => {
                    let (inner, next_cursor) = response.into_parts();
                    let orders = stream::iter(inner.orders).map(Ok);
                    Some((Either::Left(orders), next_cursor.map(Some)))
                }
                Err(error) => Some((Either::Right(stream::once(async { Err(error) })), None)),
            }
        })
        .flatten()
    }

    /// Update an existing order in the transaction cache.
    ///
    /// This method sends a PUT request to update an order that already exists
    /// in the cache. The order is identified by its ID and the entire order
    /// content is replaced with the provided data.
    ///
    /// # Arguments
    ///
    /// * `order_id` - The ID of the order to update (hex-encoded B256).
    /// * `order` - The updated [`SignedOrder`] to store.
    ///
    /// # Returns
    ///
    /// An [`OrderResponse`] containing the order's ID on success.
    ///
    /// # Errors
    ///
    /// Returns [`TxCacheError::NotFound`] if the order does not exist.
    /// Returns an error if the request fails or the response cannot be parsed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn example() -> Result<(), signet_tx_cache::TxCacheError> {
    /// use signet_tx_cache::TxCache;
    /// use signet_types::SignedOrder;
    ///
    /// let cache = TxCache::parmigiana();
    /// let order_id = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    /// # let order: SignedOrder = todo!();
    ///
    /// let response = cache.update_order(order_id, order).await?;
    /// println!("Updated order: {:?}", response.id);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`TxCacheError::NotFound`]: crate::error::TxCacheError::NotFound
    #[instrument(skip_all)]
    pub async fn update_order(&self, order_id: &str, order: SignedOrder) -> Result<OrderResponse> {
        let path = format!("{ORDERS}/{order_id}");
        self.put_inner(&path, order).await
    }
}

#[cfg(feature = "sse")]
use eventsource_stream::{Event, EventStreamError, Eventsource};
#[cfg(feature = "sse")]
use tracing::debug;

#[cfg(feature = "sse")]
impl TxCache {
    const TRANSACTIONS_FEED: &str = "transactions/feed";
    const ORDERS_FEED: &str = "orders/feed";

    fn decode_sse_events<T, S>(events: S) -> impl Stream<Item = Result<T>> + Send
    where
        T: DeserializeOwned + Send + 'static,
        S: Stream<Item = std::result::Result<Event, EventStreamError<reqwest::Error>>> + Send,
    {
        events
            .map(|result| match result {
                Ok(event) => serde_json::from_str::<T>(&event.data).map_err(Into::into),
                Err(e) => Err(e.into()),
            })
            .scan(false, |errored, result| {
                if *errored {
                    return std::future::ready(None);
                }
                *errored = result.is_err();
                std::future::ready(Some(result))
            })
    }

    /// Connect to an SSE feed endpoint, returning a stream that
    /// deserializes each event's JSON data into `T`. The stream
    /// terminates on the first error, which is yielded as the final
    /// item.
    async fn subscribe_inner<T: DeserializeOwned + Send + 'static>(
        &self,
        feed: &'static str,
    ) -> Result<impl Stream<Item = Result<T>> + Send> {
        let url = self
            .url
            .join(feed)
            .inspect_err(|e| warn!(%e, "Failed to join URL for SSE subscription"))?;

        let es =
            self.client.get(url).send().await?.error_for_status()?.bytes_stream().eventsource();

        debug!(feed, "SSE subscription established");

        Ok(Self::decode_sse_events(es))
    }

    /// Subscribe to real-time transaction events via SSE.
    ///
    /// Connects to the `/transactions/feed` endpoint and returns a
    /// [`Stream`] that yields each [`TxEnvelope`] as it arrives from
    /// the server. Unlike [`stream_transactions`], which paginates
    /// over existing data, this receives new transactions in
    /// real-time.
    ///
    /// The stream terminates on the first error, which is yielded as
    /// the final item.
    ///
    /// [`stream_transactions`]: TxCache::stream_transactions
    #[cfg_attr(docsrs, doc(cfg(feature = "sse")))]
    #[instrument(skip_all)]
    pub async fn subscribe_transactions(
        &self,
    ) -> Result<impl Stream<Item = Result<TxEnvelope>> + Send> {
        self.subscribe_inner(Self::TRANSACTIONS_FEED).await
    }

    /// Subscribe to real-time order events via SSE.
    ///
    /// Connects to the `/orders/feed` endpoint and returns a
    /// [`Stream`] that yields each [`SignedOrder`] as it arrives from
    /// the server. Unlike [`stream_orders`], which paginates over
    /// existing data, this receives new orders in real-time.
    ///
    /// The stream terminates on the first error, which is yielded as
    /// the final item.
    ///
    /// [`stream_orders`]: TxCache::stream_orders
    #[cfg_attr(docsrs, doc(cfg(feature = "sse")))]
    #[instrument(skip_all)]
    pub async fn subscribe_orders(&self) -> Result<impl Stream<Item = Result<SignedOrder>> + Send> {
        self.subscribe_inner(Self::ORDERS_FEED).await
    }
}

#[cfg(all(test, feature = "sse"))]
mod tests {
    use super::TxCache;
    use crate::error::TxCacheError;
    use futures_util::{stream, StreamExt};

    type SseError = eventsource_stream::EventStreamError<reqwest::Error>;

    fn event(data: &str) -> eventsource_stream::Event {
        eventsource_stream::Event { data: data.to_owned(), ..Default::default() }
    }

    fn utf8_sse_error() -> SseError {
        eventsource_stream::EventStreamError::Utf8(
            String::from_utf8(vec![0xff]).expect_err("invalid UTF-8 should error"),
        )
    }

    #[tokio::test]
    async fn decode_sse_events_deserializes_json_events() {
        let events = stream::iter([Ok::<_, SseError>(event(r#"{"ok":true}"#))]);

        let decoded: Vec<_> =
            TxCache::decode_sse_events::<serde_json::Value, _>(events).collect().await;

        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].as_ref().unwrap()["ok"], true);
    }

    #[tokio::test]
    async fn decode_sse_events_maps_invalid_json_to_deserialization_error() {
        let events = stream::iter([Ok::<_, SseError>(event("not-json"))]);

        let mut decoded = TxCache::decode_sse_events::<serde_json::Value, _>(events);

        match decoded.next().await.expect("stream should yield an error") {
            Err(TxCacheError::Deserialization(_)) => {}
            other => panic!("expected deserialization error, got {other:?}"),
        }
        assert!(decoded.next().await.is_none(), "stream should terminate after the error");
    }

    #[tokio::test]
    async fn decode_sse_events_maps_sse_errors() {
        let events = stream::iter([Err::<eventsource_stream::Event, _>(utf8_sse_error())]);

        let mut decoded = TxCache::decode_sse_events::<serde_json::Value, _>(events);

        match decoded.next().await.expect("stream should yield an error") {
            Err(TxCacheError::Sse(eventsource_stream::EventStreamError::Utf8(_))) => {}
            other => panic!("expected SSE error, got {other:?}"),
        }
        assert!(decoded.next().await.is_none(), "stream should terminate after the error");
    }

    #[tokio::test]
    async fn decode_sse_events_stops_after_first_error() {
        let events = stream::iter([
            Ok::<_, SseError>(event(r#"{"idx":1}"#)),
            Err(utf8_sse_error()),
            Ok(event(r#"{"idx":2}"#)),
        ]);

        let decoded: Vec<_> =
            TxCache::decode_sse_events::<serde_json::Value, _>(events).collect().await;

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].as_ref().unwrap()["idx"], 1);
        match &decoded[1] {
            Err(TxCacheError::Sse(eventsource_stream::EventStreamError::Utf8(_))) => {}
            other => panic!("expected final SSE error, got {other:?}"),
        }
    }
}
