//! Signet-related types and utilities used throughout the SDK and node.

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
pub use fills::{MarketContext, MarketError};

mod magic_sig;
pub use magic_sig::{MagicSig, MagicSigInfo};

mod height;
pub use height::PairedHeights;

mod slot;
pub use slot::SlotCalculator;
