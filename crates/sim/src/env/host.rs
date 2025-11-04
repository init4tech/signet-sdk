use crate::{InnerDb, SimDb, TimeLimited};
use signet_evm::{signet_precompiles, EvmNeedsTx, OrderDetector, SignetLayered};
use signet_types::constants::SignetSystemConstants;
use std::{marker::PhantomData, sync::Arc, time::Instant};
use trevm::{
    db::TryCachingDb,
    helpers::Ctx,
    inspectors::{Layered, TimeLimit},
    revm::{
        context::{BlockEnv, CfgEnv},
        database::{Cache, CacheDB},
        inspector::{Inspector, NoOpInspector},
        DatabaseRef,
    },
    Block, Cfg, TrevmBuilder,
};

/// A host simulation environment.
#[derive(Debug)]
pub struct HostEnv<Db, Insp = NoOpInspector> {
    db: InnerDb<Db>,

    constants: SignetSystemConstants,

    cfg: CfgEnv,
    block: BlockEnv,

    _pd: PhantomData<fn() -> Insp>,
}

impl<Db, Insp> Clone for HostEnv<Db, Insp> {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            constants: self.constants.clone(),
            cfg: self.cfg.clone(),
            block: self.block.clone(),
            _pd: PhantomData,
        }
    }
}

impl<Db, Insp> HostEnv<Db, Insp> {
    /// Create a new host environment.
    pub fn new<C, B>(db: Db, constants: SignetSystemConstants, cfg_ref: &C, block_ref: &B) -> Self
    where
        C: Cfg,
        B: Block,
    {
        let mut cfg = CfgEnv::default();
        cfg_ref.fill_cfg_env(&mut cfg);
        let mut block = BlockEnv::default();
        block_ref.fill_block_env(&mut block);

        Self { db: Arc::new(CacheDB::new(db)), constants, cfg, block, _pd: PhantomData }
    }

    /// Get a mutable reference to the inner database.
    pub const fn db_mut(&mut self) -> &mut InnerDb<Db> {
        &mut self.db
    }

    /// Get a reference to the signet system constants.
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }

    /// Get a reference to the [`CfgEnv`].
    pub const fn cfg(&self) -> &CfgEnv {
        &self.cfg
    }

    /// Get a mutable reference to the [`CfgEnv`].
    pub const fn cfg_mut(&mut self) -> &mut CfgEnv {
        &mut self.cfg
    }

    /// Get a reference to the [`BlockEnv`].
    pub const fn block(&self) -> &BlockEnv {
        &self.block
    }

    /// Get a mutable reference to the [`BlockEnv`].
    pub const fn block_mut(&mut self) -> &mut BlockEnv {
        &mut self.block
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
    ) -> EvmNeedsTx<crate::SimDb<Db>, TimeLimited<Insp>> {
        let db = self.sim_db();
        let inspector = Layered::new(TimeLimit::new(finish_by - Instant::now()), Insp::default());

        // We layer on a order detector specific to the host environment.
        let inspector =
            SignetLayered::new(inspector, OrderDetector::for_host(self.constants.clone()));

        // This is the same code as `signet_evm::signet_evm_with_inspector`, but
        // we need to build the EVM manually to insert our layered inspector,
        // as the shortcut will insert a rollup order detector.
        TrevmBuilder::new()
            .with_db(db)
            .with_insp(inspector)
            .with_precompiles(signet_precompiles())
            .build_trevm()
            .expect("db set")
            .fill_cfg(&self.cfg)
            .fill_block(&self.block)
    }
}

impl<Db, Insp> HostEnv<Db, Insp>
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
