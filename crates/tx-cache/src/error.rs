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
    /// A conflict occurred when updating a resource (e.g., version mismatch).
    #[error("Conflict occurred when updating resource")]
    Conflict,

    /// An error occurred while parsing the URL.
    #[error(transparent)]
    Url(#[from] url::ParseError),

    /// An error occurred while contacting the TxCache API.
    #[error("Error contacting TxCache API: {0}")]
    Reqwest(reqwest::Error),
}

impl From<reqwest::Error> for TxCacheError {
    fn from(err: reqwest::Error) -> Self {
        match err.status() {
            Some(reqwest::StatusCode::NOT_FOUND) => TxCacheError::NotFound,
            Some(reqwest::StatusCode::FORBIDDEN) => TxCacheError::NotOurSlot,
            Some(reqwest::StatusCode::CONFLICT) => TxCacheError::Conflict,
            _ => TxCacheError::Reqwest(err),
        }
    }
}
