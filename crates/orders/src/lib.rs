//! Signet Orders Library
//!
//! Contains utilities for placing and filling orders on Signet.

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

mod impls;

mod traits;
pub use traits::{BundleSubmitter, OrderSource, OrderSubmitter};
