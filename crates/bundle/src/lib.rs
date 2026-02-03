//! Signet Bundle Library
//!
//! Contains the [`SignetCallBundle`] and [`SignetEthBundle`] type, and
//! utilities related to creating and simulating Signet bundles.
//!
//! # Bundles
//!
//! The [`SignetCallBundle`] and [`SignetEthBundle`] types are used to simulate
//! transaction bundles in different ways. The [`SignetBundleDriver`] type
//! drives a [`SignetCallBundle`] to completion and generates a
//! [`SignetCallBundleResponse`]. This is used primarily by the RPC server to
//! serve `signet_callBundle` requests. The response includes the standard
//! flashbots-style response information, as well as a description of the fills
//! necessary to make the bundle valid on Signet.
//!
//! The [`SignetEthBundle`] type is used to simulate transaction bundles while
//! building blocks. It is used primarily by builders and relays. The
//! [`SignetEthBundleDriver`] drives a [`SignetEthBundle`] to completion and
//! enforces bundle rules. When used in a block builder, it will ensure that the
//! bundle is valid and that the fills are valid at the time of block
//! construction.
//!
//! # Using [`SignetEthBundle`] safely
//!
//! The [`SignetEthBundle`] type contains actions that must be performed on
//! both chains. As such, its simulation must be performed on both chains. The
//! primary transaction simulation via [`SignetEthBundleDriver`] is performed
//! locally using [`trevm`].

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

mod call;
pub use call::{SignetBundleApi, SignetBundleDriver, SignetCallBundle, SignetCallBundleResponse};

mod send;
pub use send::{
    BundleInspector, BundleRecoverError, RecoverError, RecoveredBundle, SignetEthBundle,
    SignetEthBundleDriver, SignetEthBundleError, SignetEthBundleInsp, TxRequirement,
};
