/// Result type for [`TxCache`] operations.
///
/// [`TxCache`]: crate::client::TxCache
pub type Result<T> = std::result::Result<T, TxCacheError>;

/// Errors returned by the [`TxCache`] client.
///
/// [`TxCache`]: crate::client::TxCache
#[derive(thiserror::Error, Debug)]
pub enum TxCacheError {
    /// The requested transaction or bundle was not found in the cache.
    #[error("Transaction not found in cache")]
    NotFound,
    /// The request was made during a slot that is not assigned to this builder.
    #[error("Request occurred during a slot that is not assigned to this builder")]
    NotOurSlot,
    /// An error occurred while parsing the URL.
    #[error(transparent)]
    Url(#[from] url::ParseError),

    /// An error occurred while contacting the TxCache API.
    #[error("Error contacting TxCache API: {0}")]
    Reqwest(reqwest::Error),

    /// An error occurred while parsing SSE events.
    #[cfg(feature = "sse")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sse")))]
    #[error("SSE stream error: {0}")]
    Sse(eventsource_stream::EventStreamError<reqwest::Error>),

    /// Failed to deserialize an SSE event payload.
    #[cfg(feature = "sse")]
    #[cfg_attr(docsrs, doc(cfg(feature = "sse")))]
    #[error("Failed to deserialize SSE event: {0}")]
    Deserialization(serde_json::Error),
}

impl From<reqwest::Error> for TxCacheError {
    fn from(err: reqwest::Error) -> Self {
        match err.status() {
            Some(reqwest::StatusCode::NOT_FOUND) => TxCacheError::NotFound,
            Some(reqwest::StatusCode::FORBIDDEN) => TxCacheError::NotOurSlot,
            _ => TxCacheError::Reqwest(err),
        }
    }
}

#[cfg(feature = "sse")]
impl From<eventsource_stream::EventStreamError<reqwest::Error>> for TxCacheError {
    fn from(err: eventsource_stream::EventStreamError<reqwest::Error>) -> Self {
        Self::Sse(err)
    }
}

#[cfg(feature = "sse")]
impl From<serde_json::Error> for TxCacheError {
    fn from(err: serde_json::Error) -> Self {
        Self::Deserialization(err)
    }
}
