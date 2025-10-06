//! Signet Transaction Cache types and client.

#![warn(
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    unreachable_pub,
    clippy::missing_const_for_fn,
    rustdoc::all
)]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![deny(unused_must_use, rust_2018_idioms)]
#![cfg_attr(docsrs, feature(doc_cfg))]

/// The [`TxCache`] client.
pub mod client;

/// Response types for the [`TxCache`].
///
/// [`TxCache`]: crate::client::TxCache
pub mod types;
