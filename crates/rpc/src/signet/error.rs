//! Signet RPC errors.

use reth::rpc::server_types::eth::EthApiError;

/// Errors that can occur when interacting with the `signet` namespace.
#[derive(Debug, thiserror::Error)]
pub enum SignetError {
    /// The transaction cache URL was not provided.
    #[error("transaction cache URL not provided")]
    TxCacheUrlNotProvided,
    /// An error coming from interacting with components
    /// that could emit `EthApiError`s, such as the tx-cache.
    #[error(transparent)]
    EthApiError(#[from] EthApiError),
}

impl SignetError {
    /// Turn into a string by value, allows for `.map_err(SignetError::to_string)`
    /// to be used.
    pub fn into_string(self) -> String {
        ToString::to_string(&self)
    }
}
