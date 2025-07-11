//! Signet system constants.
//!
//! This crate contains the system constants for Signet chains, including the
//! host and rollup system contracts, pre-deployed tokens, etc.
//!

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

mod chains;
#[cfg(any(test, feature = "test-utils"))]
pub use chains::test_utils;

mod types;
pub use types::{
    ConfigError, HostConstants, HostPermitted, HostTokens, HostUsdRecord, KnownChains,
    PairedHeights, ParseChainError, RollupConstants, RollupPermitted, RollupTokens,
    SignetConstants, SignetEnvironmentConstants, SignetSystemConstants, UsdRecords, MINTER_ADDRESS,
};

/// Placeholder address for ETH.
pub const ETH_ADDRESS: alloy::primitives::Address = alloy::primitives::Address::repeat_byte(0xee);
