//! Signet-related types and utilities used throughout the SDK and node.
//!
//! This is a utility and data-type crate. As a result its documentation is
//! boring.

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

/// Structs that hold Signet system configuration.
pub mod config;

mod fills;
pub use fills::{AggregateFills, MarketError};

mod magic_sig;
pub use magic_sig::{MagicSig, MagicSigInfo};

mod height;
pub use height::PairedHeights;

mod slot;
pub use slot::SlotCalculator;

mod seq;
pub use seq::{RequestSigner, SignRequest, SignResponse};

#[cfg(any(test, feature = "test-utils"))]
/// Utils for unit and integration tests.
pub mod test_utils;
