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
#![cfg_attr(docsrs, feature(doc_cfg))]

/// Structs that hold Signet system configuration.
pub use signet_constants as constants;
pub use signet_constants::PairedHeights;

mod agg;
pub use agg::{AggregateFills, AggregateOrders, MarketError};
mod confirmation;
pub use confirmation::ConfirmationMeta;
mod confirmed;
pub use confirmed::Confirmed;
mod receipt;
pub use receipt::Receipt;
mod tx_location;
pub use tx_location::TxLocation;

mod magic_sig;
pub use magic_sig::{MagicSig, MagicSigInfo};

/// Primitive block types used in Signet.
pub mod primitives;

mod seq;
pub use seq::{RequestSigner, SignRequest, SignResponse};

mod signing;
pub use signing::{
    SignedFill, SignedOrder, SignedPermitError, SigningError, UnsignedFill, UnsignedOrder,
};

mod utils;
pub use utils::{alias_address, unalias_address};

// Suppress unused_crate_dependencies warning for proptest (used only in tests)
#[cfg(feature = "proptest")]
use proptest as _;
