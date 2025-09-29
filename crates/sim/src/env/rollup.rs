use crate::{InnerDb, SimDb, TimeLimited};
use signet_evm::EvmNeedsCfg;
use signet_types::constants::SignetSystemConstants;
use std::{marker::PhantomData, sync::Arc, time::Instant};
use trevm::{
    db::{cow::CacheOnWrite, TryCachingDb},
    helpers::Ctx,
    inspectors::{Layered, TimeLimit},
    revm::{
        database::{Cache, CacheDB},
        inspector::NoOpInspector,
        DatabaseRef, Inspector,
    },
};

/// A rollup simulation environment.
#[derive(Debug)]
pub struct RollupEnv<Db, Insp = NoOpInspector> {
    db: InnerDb<Db>,
    constants: SignetSystemConstants,
    _pd: PhantomData<fn() -> Insp>,
}

impl<Db, Insp> Clone for RollupEnv<Db, Insp> {
    fn clone(&self) -> Self {
        Self { db: self.db.clone(), constants: self.constants.clone(), _pd: PhantomData }
    }
}

impl<Db, Insp> RollupEnv<Db, Insp> {
    /// Create a new rollup environment.
    pub fn new(db: Db, constants: SignetSystemConstants) -> Self {
        Self { db: Arc::new(CacheDB::new(db)), constants, _pd: PhantomData }
    }

    /// Get a mutable reference to the inner database.
    pub const fn db_mut(&mut self) -> &mut InnerDb<Db> {
        &mut self.db
    }

    /// Get the constants for this environment.
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }
}

impl<Db, Insp> RollupEnv<Db, Insp>
where
    Db: DatabaseRef + Send + Sync,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync,
{
    /// Connect a fresh database for the simulation.
    pub fn sim_db(&self) -> SimDb<Db> {
        CacheOnWrite::new(self.db.clone())
    }

    /// Create a new EVM for the rollup environment that will finish by the
    /// given instant.
    pub fn create_evm(&self, finish_by: Instant) -> EvmNeedsCfg<SimDb<Db>, TimeLimited<Insp>> {
        let db = self.sim_db();

        let inspector = Layered::new(TimeLimit::new(finish_by - Instant::now()), Insp::default());

        signet_evm::signet_evm_with_inspector(db, inspector, self.constants.clone())
    }
}

impl<Db, Insp> RollupEnv<Db, Insp>
where
    Db: DatabaseRef,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync,
{
    /// Accepts a cache from the simulation and extends the database with it.
    pub fn accept_cache(
        &mut self,
        cache: Cache,
    ) -> Result<(), <InnerDb<Db> as TryCachingDb>::Error> {
        self.db_mut().try_extend(cache)
    }

    /// Accepts a cache from the simulation and extends the database with it.
    pub fn accept_cache_ref(
        &mut self,
        cache: &Cache,
    ) -> Result<(), <InnerDb<Db> as TryCachingDb>::Error> {
        self.db_mut().try_extend_ref(cache)
    }
}
