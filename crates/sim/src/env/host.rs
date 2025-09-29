use crate::{InnerDb, SimDb, TimeLimited};
use std::{marker::PhantomData, sync::Arc, time::Instant};
use trevm::{
    helpers::Ctx,
    inspectors::{Layered, TimeLimit},
    revm::{
        database::CacheDB,
        inspector::{Inspector, NoOpInspector},
        DatabaseRef,
    },
    TrevmBuilder,
};

/// A host simulation environment.
#[derive(Debug)]
pub struct HostEnv<Db, Insp = NoOpInspector> {
    db: InnerDb<Db>,
    _pd: PhantomData<fn() -> Insp>,
}

impl<Db, Insp> Clone for HostEnv<Db, Insp> {
    fn clone(&self) -> Self {
        Self { db: self.db.clone(), _pd: PhantomData }
    }
}

impl<Db> From<Db> for HostEnv<Db, NoOpInspector>
where
    Db: DatabaseRef + Send + Sync,
{
    fn from(db: Db) -> Self {
        Self::new(db)
    }
}

impl<Db, Insp> HostEnv<Db, Insp> {
    /// Create a new host environment.
    pub fn new(db: Db) -> Self {
        Self { db: Arc::new(CacheDB::new(db)), _pd: PhantomData }
    }

    /// Get a mutable reference to the inner database.
    pub const fn db_mut(&mut self) -> &mut InnerDb<Db> {
        &mut self.db
    }
}

impl<Db, Insp> HostEnv<Db, Insp>
where
    Db: DatabaseRef + Send + Sync,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync,
{
    /// Connect a fresh database for the simulation.
    pub fn sim_db(&self) -> crate::SimDb<Db> {
        crate::SimDb::new(self.db.clone())
    }

    /// Create a new EVM for the host environment that will finish by the
    /// given instant.
    pub fn create_evm(
        &self,
        finish_by: Instant,
    ) -> trevm::EvmNeedsCfg<crate::SimDb<Db>, TimeLimited<Insp>> {
        let db = self.sim_db();
        let inspector = Layered::new(TimeLimit::new(finish_by - Instant::now()), Insp::default());

        TrevmBuilder::new().with_insp(inspector).with_db(db).build_trevm().unwrap()
    }
}
