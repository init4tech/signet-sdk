//! Signet journal utilities.
//!
//! In general, it is recommended to use the [`Journal`] enum, for forwards
//! compatibility.

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

mod host;
pub use host::HostJournal;

mod meta;
pub use meta::JournalMeta;

mod set;
pub use set::JournalSet;

mod versions;
pub use versions::Journal;

use futures_util::Stream;

/// Any [`Stream`] that produces [`Journal`]s.
pub trait JournalStream<'a>: Stream<Item = Journal<'a>> {}

impl<'a, S> JournalStream<'a> for S where S: Stream<Item = Journal<'a>> {}
