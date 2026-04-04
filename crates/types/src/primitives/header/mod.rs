//! Validated signet header newtypes.
//!
//! Signet headers have stricter invariants than standard Ethereum headers.
//! These newtypes enforce those invariants at construction time and eagerly
//! cache the block hash.

mod v1;
pub use v1::SignetHeaderV1;

#[cfg(feature = "experimental")]
mod v2;
#[cfg(feature = "experimental")]
#[allow(deprecated)]
pub use v2::SignetHeaderV2;

use alloy::consensus::{constants::EMPTY_ROOT_HASH, BlockHeader, Header};
use std::sync::LazyLock;

/// Default header used for comparison in validation checks, allocated once.
static DEFAULT_HEADER: LazyLock<Header> = LazyLock::new(Header::default);

/// Error returned when a [`Header`] violates signet header invariants.
#[derive(Debug, thiserror::Error)]
#[error("invalid signet header: expected default: {must_be_default:?}, expected non-default: {must_not_be_default:?}")]
pub struct SignetHeaderError {
    /// Field names that must be default but were not.
    pub must_be_default: Vec<&'static str>,
    /// Field names that must not be default but were.
    pub must_not_be_default: Vec<&'static str>,
}

/// Check that shared fields equal their defaults.
pub(crate) fn check_shared_defaults(header: &Header) -> Vec<&'static str> {
    let d = &*DEFAULT_HEADER;
    let mut bad = Vec::new();
    if header.ommers_hash() != d.ommers_hash() {
        bad.push("ommers_hash");
    }
    if header.state_root() != d.state_root() {
        bad.push("state_root");
    }
    if header.withdrawals_root() != d.withdrawals_root() {
        bad.push("withdrawals_root");
    }
    if header.blob_gas_used() != d.blob_gas_used() {
        bad.push("blob_gas_used");
    }
    if header.excess_blob_gas() != d.excess_blob_gas() {
        bad.push("excess_blob_gas");
    }
    if header.requests_hash() != d.requests_hash() {
        bad.push("requests_hash");
    }
    if header.extra_data() != d.extra_data() {
        bad.push("extra_data");
    }
    bad
}

/// Check that `transactions_root` and `receipts_root` are `EMPTY_ROOT_HASH`.
pub(crate) fn check_roots_empty(header: &Header) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if header.transactions_root() != EMPTY_ROOT_HASH {
        bad.push("transactions_root");
    }
    if header.receipts_root() != EMPTY_ROOT_HASH {
        bad.push("receipts_root");
    }
    bad
}

/// Check that `transactions_root` and `receipts_root` are NOT `EMPTY_ROOT_HASH`.
#[cfg(feature = "experimental")]
pub(crate) fn check_roots_non_empty(header: &Header) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if header.transactions_root() == EMPTY_ROOT_HASH {
        bad.push("transactions_root");
    }
    if header.receipts_root() == EMPTY_ROOT_HASH {
        bad.push("receipts_root");
    }
    bad
}
