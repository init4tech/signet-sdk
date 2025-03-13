#![doc = include_str!("../README.md")]
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

mod bindings;
pub use bindings::{
    mintCall, BundleHelper, HostOrders, Passage, RollupOrders, RollupPassage, Transactor, Zenith,
};

mod block;
pub use block::{decode_txns, encode_txns, Alloy2718Coder, Coder, ZenithBlock, ZenithTransaction};

mod orders;
pub use orders::{AggregateOrders, SignedOrder};

mod trevm;

use alloy::primitives::{address, Address};

/// System address with permission to mint tokens on pre-deploys.
/// "tokenadmin"
pub const MINTER_ADDRESS: Address = address!("00000000000000000000746f6b656e61646d696e");
