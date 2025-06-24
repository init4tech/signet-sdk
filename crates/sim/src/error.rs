/// Possible errors that can occur when using the cache.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum CacheError {
    /// The bundle does not have a replacement UUID, which is required for caching.
    #[error("bundle has no replacement UUID")]
    BundleWithoutReplacementUuid,
}
