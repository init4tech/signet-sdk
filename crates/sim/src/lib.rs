mod built;
pub use built::BuiltBlock;

mod cache;
pub use cache::SimCache;

mod env;
pub use env::SimEnv;

mod item;
pub use item::SimItem;

mod outcome;
pub(crate) use outcome::SimOutcomeWithCache;

mod task;
pub use task::BlockBuild;

/// A type alias for the database underlying the simulation.
pub type InnerDb<Db> = std::sync::Arc<trevm::revm::database::CacheDB<Db>>;

/// A type alias for the database used in the simulation.
pub type SimDb<Db> = trevm::db::cow::CacheOnWrite<InnerDb<Db>>;
