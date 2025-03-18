//! Signet Bundle Library
//!
//! Contains the [`SignetBundle`] type, and utilities related to creating and
//! simulating Signet bundles.

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
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod call;
pub use call::{SignetBundleDriver, SignetCallBundle, SignetCallBundleResponse};

mod send;
pub use send::{SignetEthBundle, SignetEthBundleResponse};
