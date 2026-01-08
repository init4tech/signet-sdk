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
#![cfg_attr(docsrs, feature(doc_cfg))]

mod built;

pub use built::BuiltBlock;

mod cache;
pub use cache::{CacheError, SimCache, SimIdentifier, SimItem, SimItemValidity, StateSource};

mod env;
pub use env::{HostEnv, RollupEnv, SharedSimEnv, SimEnv};

mod outcome;
pub use outcome::SimOutcomeWithCache;

mod task;
pub use task::BlockBuild;

use std::sync::Arc;
use trevm::{
    db::cow::CacheOnWrite,
    inspectors::{Layered, TimeLimit},
    revm::database::CacheDB,
};

/// A type alias for the database underlying the simulation.
pub type InnerDb<Db> = Arc<CacheDB<Db>>;

/// A type alias for the database used in the simulation.
pub type SimDb<Db> = CacheOnWrite<InnerDb<Db>>;

/// A time-limited layered inspector.
pub type TimeLimited<Insp> = Layered<TimeLimit, Insp>;
