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

//! Contains logic for extracting events and other data from host chain blocks.

mod events;
pub use events::Events;

mod extracted;
pub use extracted::ExtractedEvent;

mod extractor;
pub use extractor::Extractor;

mod block;
pub use block::Extracts;
