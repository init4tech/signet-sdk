//! Signet Sim
//!
//! A simple parallelized transaction simulation library.

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

mod built;
pub use built::BuiltBlock;

mod cache;
pub use cache::{CacheError, SimCache};

mod env;
pub use env::{SharedSimEnv, SimEnv};

mod item;
pub use item::{SimIdentifier, SimItem};

mod outcome;
pub use outcome::SimOutcomeWithCache;

mod task;
pub use task::BlockBuild;

/// A type alias for the database underlying the simulation.
pub type InnerDb<Db> = std::sync::Arc<trevm::revm::database::CacheDB<Db>>;

/// A type alias for the database used in the simulation.
pub type SimDb<Db> = trevm::db::cow::CacheOnWrite<InnerDb<Db>>;
