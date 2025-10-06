//! Contains logic for extracting events and other data from host chain blocks.
//!
//! ## Usage
//!
//! Create a [`Extractor`] from a set of [`SignetSystemConstants`], then invoke
//! [`Extractor::extract_signet`] to extract all relevant Signet events from a
//! chain.
//!
//! These events will be returned as a series of [`Extracts`] objects, each of
//! which containing the relevant [`ExtractedEvent`]s and a [`AggregateFills`]
//! for a specific host block.
//!
//! [`SignetSystemConstants`]: signet_types::config::SignetSystemConstants
//! [`AggregateFills`]: signet_types::AggregateFills

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

mod block;
pub use block::Extracts;

mod events;
pub use events::Events;

mod extracted;
pub use extracted::ExtractedEvent;

mod extractor;
pub use extractor::Extractor;

mod r#trait;
pub use r#trait::{Extractable, HasTxns};

mod step;
pub use step::ExtractStep;
