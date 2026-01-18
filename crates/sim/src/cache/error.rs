/// Possible errors that can occur when using the cache.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// The bundle does not have a replacement UUID, which is required for caching.
    #[error("bundle has no replacement UUID")]
    BundleWithoutReplacementUuid,

    /// Error recovering a transaction.
    #[error(transparent)]
    TxRecover(#[from] alloy::consensus::crypto::RecoveryError),

    /// Error recovering a bundle.
    #[error(transparent)]
    BundleRecover(#[from] signet_bundle::BundleRecoverError),
}
