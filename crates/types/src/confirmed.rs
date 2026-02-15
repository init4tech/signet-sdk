//! Wrapper type pairing a value with block confirmation metadata.

use crate::ConfirmationMeta;

/// A value paired with its block confirmation metadata.
///
/// This wrapper associates a transaction or receipt with the block
/// context in which it was confirmed (block number, block hash,
/// transaction index).
///
/// # Example
///
/// ```
/// # use signet_types::{ConfirmationMeta, Confirmed};
/// # use alloy::primitives::B256;
/// let meta = ConfirmationMeta::new(42, B256::ZERO, 0);
/// let confirmed = Confirmed::new("hello", meta);
/// assert_eq!(*confirmed.inner(), "hello");
/// assert_eq!(confirmed.meta().block_number(), 42);
///
/// // Transform the inner value while preserving metadata.
/// let mapped = confirmed.map(|s| s.len());
/// assert_eq!(*mapped.inner(), 5);
/// assert_eq!(mapped.meta().block_number(), 42);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirmed<T> {
    inner: T,
    meta: ConfirmationMeta,
}

impl<T> Confirmed<T> {
    /// Create a new confirmed value with the given metadata.
    pub const fn new(inner: T, meta: ConfirmationMeta) -> Self {
        Self { inner, meta }
    }

    /// Returns a reference to the inner value.
    pub const fn inner(&self) -> &T {
        &self.inner
    }

    /// Consumes self and returns the inner value.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Returns a reference to the confirmation metadata.
    pub const fn meta(&self) -> &ConfirmationMeta {
        &self.meta
    }

    /// Consumes self and returns the inner value and metadata.
    pub fn into_parts(self) -> (T, ConfirmationMeta) {
        (self.inner, self.meta)
    }

    /// Transforms the inner value while preserving metadata.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Confirmed<U> {
        Confirmed { inner: f(self.inner), meta: self.meta }
    }
}
