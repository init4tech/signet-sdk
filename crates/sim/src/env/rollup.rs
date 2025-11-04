use crate::{InnerDb, SimDb, TimeLimited};
use signet_evm::EvmNeedsTx;
use signet_types::{
    constants::SignetSystemConstants, AggregateFills, AggregateOrders, MarketError,
};
use std::{marker::PhantomData, sync::Arc, time::Instant};
use trevm::{
    db::{cow::CacheOnWrite, TryCachingDb},
    helpers::Ctx,
    inspectors::{Layered, TimeLimit},
    revm::{
        context::{BlockEnv, CfgEnv},
        database::{Cache, CacheDB},
        inspector::NoOpInspector,
        DatabaseRef, Inspector,
    },
    Block, Cfg,
};

/// A rollup simulation environment.
#[derive(Debug)]
pub struct RollupEnv<Db, Insp = NoOpInspector> {
    db: InnerDb<Db>,
    constants: SignetSystemConstants,
    fill_state: AggregateFills,

    cfg: CfgEnv,
    block: BlockEnv,

    _pd: PhantomData<fn() -> Insp>,
}

impl<Db, Insp> Clone for RollupEnv<Db, Insp> {
    fn clone(&self) -> Self {
        Self {
            db: self.db.clone(),
            constants: self.constants.clone(),
            fill_state: self.fill_state.clone(),
            cfg: self.cfg.clone(),
            block: self.block.clone(),
            _pd: PhantomData,
        }
    }
}

impl<Db, Insp> RollupEnv<Db, Insp> {
    /// Create a new rollup environment.
    pub fn new<C, B>(db: Db, constants: SignetSystemConstants, cfg_ref: &C, block_ref: &B) -> Self
    where
        C: Cfg,
        B: Block,
    {
        let mut cfg = CfgEnv::default();
        cfg_ref.fill_cfg_env(&mut cfg);
        let mut block = BlockEnv::default();
        block_ref.fill_block_env(&mut block);

        Self {
            db: Arc::new(CacheDB::new(db)),
            constants,
            fill_state: AggregateFills::default(),
            cfg,
            block,
            _pd: PhantomData,
        }
    }

    /// Get a reference to the inner database.
    pub const fn db(&self) -> &InnerDb<Db> {
        &self.db
    }

    /// Get a mutable reference to the inner database.
    pub const fn db_mut(&mut self) -> &mut InnerDb<Db> {
        &mut self.db
    }

    /// Get the constants for this environment.
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }

    /// Get a reference to the fill state.
    pub const fn fill_state(&self) -> &AggregateFills {
        &self.fill_state
    }

    /// Get a mutable reference to the fill state.
    pub const fn fill_state_mut(&mut self) -> &mut AggregateFills {
        &mut self.fill_state
    }

    /// Accepts aggregate fills and orders, updating the fill state.
    pub fn accept_aggregates(
        &mut self,
        fills: &AggregateFills,
        orders: &AggregateOrders,
    ) -> Result<(), MarketError> {
        self.fill_state.checked_remove_ru_tx_events(fills, orders)
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
    pub fn create_evm(&self, finish_by: Instant) -> EvmNeedsTx<SimDb<Db>, TimeLimited<Insp>> {
        let db = self.sim_db();

        let inspector = Layered::new(TimeLimit::new(finish_by - Instant::now()), Insp::default());

        signet_evm::signet_evm_with_inspector(db, inspector, self.constants.clone())
            .fill_cfg(&self.cfg)
            .fill_block(&self.block)
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
