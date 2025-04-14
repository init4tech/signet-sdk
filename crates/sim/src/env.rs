use crate::{outcome::SimulatedItem, SimCache, SimItem, SimOutcomeWithCache};
use alloy::{consensus::TxEnvelope, primitives::U256};
use signet_bundle::{SignetEthBundle, SignetEthBundleDriver, SignetEthBundleError};
use signet_evm::SignetLayered;
use signet_types::config::SignetSystemConstants;
use std::{convert::Infallible, marker::PhantomData, ops::Deref, sync::Arc, time::Instant};
use tokio::{
    select,
    sync::{mpsc, oneshot, watch},
};
use trevm::{
    db::{cow::CacheOnWrite, TryCachingDb},
    helpers::Ctx,
    inspectors::{Layered, TimeLimit},
    revm::{
        context::result::EVMError,
        database::{Cache, CacheDB},
        inspector::NoOpInspector,
        DatabaseRef, Inspector,
    },
    Block, BundleDriver, Cfg, DbConnect, EvmFactory,
};

/// A type alias for the database underlying the simulation.
pub type InnerDb<Db> = Arc<CacheDB<Db>>;

/// A type alias for the database used in the simulation.
pub type SimDb<Db> = CacheOnWrite<InnerDb<Db>>;

pub struct SimEnv<Db, Insp = NoOpInspector> {
    inner: Arc<SimEnvInner<Db, Insp>>,
}

impl<Db, Insp> Deref for SimEnv<Db, Insp> {
    type Target = SimEnvInner<Db, Insp>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<Db, Insp> From<SimEnvInner<Db, Insp>> for SimEnv<Db, Insp>
where
    Db: DatabaseRef + Send + Sync + 'static,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync + 'static,
{
    fn from(inner: SimEnvInner<Db, Insp>) -> Self {
        Self { inner: Arc::new(inner) }
    }
}

impl<Db, Insp> SimEnv<Db, Insp>
where
    Db: DatabaseRef + Send + Sync + 'static,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync + 'static,
{
    /// Creates a new `SimEnv` instance.
    pub fn new(
        db: Db,
        constants: SignetSystemConstants,
        cfg: Box<dyn Cfg>,
        block: Box<dyn Block>,
        finish_by: std::time::Instant,
        concurrency_limit: usize,
        sim_items: SimCache,
    ) -> Self {
        SimEnvInner::new(db, constants, cfg, block, finish_by, concurrency_limit, sim_items).into()
    }

    /// Run a simulation round, returning the best item.
    pub async fn sim_round(
        &mut self,
        finish_by: std::time::Instant,
        max_gas: u64,
    ) -> Option<SimulatedItem> {
        let (best_tx, mut best_watcher) = watch::channel(None);

        let (done_tx, done_rx) = oneshot::channel();

        let this = self.inner.clone();

        // Spawn a blocking task to run the simulations.
        tokio::task::spawn_blocking(move || async move {
            // Pull the `n` best items from the cache.
            let active_sim = this.sim_items.read_best(this.concurrency_limit);

            // If there are no items to simulate, return None.
            let best_score = U256::ZERO;

            std::thread::scope(|scope| {
                // Create a channel to send the results back.
                let (candidates, mut candidates_rx) = mpsc::channel(this.concurrency_limit);

                // Spawn a thread per bundle to simulate.
                for (identifier, item) in active_sim.iter() {
                    let this_ref = &this;
                    let c = candidates.clone();

                    scope.spawn(move || {
                        // If simulation is succesful, send the outcome via the
                        // channel.
                        if let Ok(candidate) = this_ref.simulate(*identifier, item) {
                            if candidate.gas_used <= max_gas {
                                let _ = c.blocking_send(candidate);
                                return;
                            }
                        };
                        // fall through applies to all errors, occurs if
                        // the simulation fails or the gas limit is exceeded.
                        this_ref.sim_items.remove(*identifier);
                    });
                }
                // Drop the TX so that the channel is closed when all threads
                // are done.
                drop(candidates);

                // Wait for each thread to finish. Find the best outcome.
                while let Some(candidate) = candidates_rx.blocking_recv() {
                    if candidate.score > best_score {
                        let _ = best_tx.send(Some(candidate));
                    }
                }
                let _ = done_tx.send(());
            });
        });

        // Either simulation is done, or we time out
        select! {
            _ = tokio::time::sleep_until(finish_by.into()) => {},
            _ = done_rx => {},
        }

        // Check what the current best outcome is.
        let best = best_watcher.borrow_and_update();
        let outcome = best.as_ref()?;

        // Remove the item from the cache.
        let item = self.sim_items.remove(outcome.identifier)?;
        // Accept the cache from the simulation.
        Arc::get_mut(&mut self.inner)
            .expect("sims dropped already")
            .accept_cache_ref(&outcome.cache)
            .ok()?;

        Some(SimulatedItem { gas_used: outcome.gas_used, score: outcome.score, item })
    }
}

/// A simulation environment.
pub struct SimEnvInner<Db, Insp = NoOpInspector> {
    /// The database to use for the simulation. This database will be wrapped
    /// in [`CacheOnWrite`] databases for each simulation.
    db: InnerDb<Db>,

    /// The cache of items to simulate.
    sim_items: SimCache,

    /// The system constants for the Signet network.
    constants: SignetSystemConstants,

    /// Chain cfg to use for the simulation.
    cfg: Box<dyn Cfg>,

    /// Block to use for the simulation.
    block: Box<dyn Block>,

    /// The instant by which the simulation should finish.
    finish_by: std::time::Instant,

    /// The maximum number of concurrent simulations to run.
    concurrency_limit: usize,

    /// Spooky ghost inspector.
    _pd: PhantomData<fn() -> Insp>,
}

impl<Db, Insp> SimEnvInner<Db, Insp> {
    /// Creates a new `SimFactory` instance.
    pub fn new(
        db: Db,
        constants: SignetSystemConstants,
        cfg: Box<dyn Cfg>,
        block: Box<dyn Block>,
        finish_by: std::time::Instant,
        concurrency_limit: usize,
        sim_items: SimCache,
    ) -> Self {
        Self {
            db: Arc::new(CacheDB::new(db)),
            constants,
            cfg,
            block,
            finish_by,
            concurrency_limit,
            sim_items,
            _pd: PhantomData,
        }
    }

    /// Get a reference to the database.
    pub fn db_mut(&mut self) -> &mut InnerDb<Db> {
        &mut self.db
    }

    /// Get a reference to the system constants.
    pub fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }

    /// Get a reference to the chain cfg.
    pub fn cfg(&self) -> &dyn Cfg {
        &self.cfg
    }

    /// Get a reference to the block.
    pub fn block(&self) -> &dyn Block {
        &self.block
    }

    /// Get the exectuion timeout.
    pub fn finish_by(&self) -> std::time::Instant {
        self.finish_by
    }

    /// Set the execution timeout.
    pub fn set_finish_by(&mut self, timeout: std::time::Instant) {
        self.finish_by = timeout;
    }
}

impl<Db, Insp> DbConnect for SimEnvInner<Db, Insp>
where
    Db: DatabaseRef + Send + Sync,
    Insp: Sync,
{
    type Database = SimDb<Db>;

    type Error = Infallible;

    fn connect(&self) -> Result<Self::Database, Self::Error> {
        Ok(CacheOnWrite::new(self.db.clone()))
    }
}

impl<Db, Insp> EvmFactory for SimEnvInner<Db, Insp>
where
    Db: DatabaseRef + Send + Sync,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync,
{
    type Insp = SignetLayered<Layered<TimeLimit, Insp>>;

    fn create(&self) -> Result<trevm::EvmNeedsCfg<Self::Database, Self::Insp>, Self::Error> {
        let db = self.connect().unwrap();

        let inspector =
            Layered::new(TimeLimit::new(self.finish_by - Instant::now()), Insp::default());

        Ok(signet_evm::signet_evm_with_inspector(db, inspector, self.constants))
    }
}

impl<Db, Insp> SimEnvInner<Db, Insp>
where
    Db: DatabaseRef + Send + Sync,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync,
{
    /// Simulates a transaction in the context of a block.
    ///
    /// This function runs the simulation in a separate thread and waits for
    /// the result or the deadline to expire.
    fn simulate_tx(
        &self,
        identifier: u128,
        transaction: &TxEnvelope,
    ) -> Result<SimOutcomeWithCache, SignetEthBundleError<SimDb<Db>>> {
        let trevm = self.create_with_block(&self.cfg, &self.block).unwrap();

        // Get the initial beneficiary balance
        let beneificiary = trevm.beneficiary();
        let initial_beneficiary_balance =
            trevm.try_read_balance_ref(beneificiary).map_err(EVMError::Database)?;

        // If succesful, take the cache. If failed, return the error.
        match trevm.run_tx(transaction) {
            Ok(trevm) => {
                // Get the beneficiary balance after the transaction and calculate the
                // increase
                let beneficiary_balance =
                    trevm.try_read_balance_ref(beneificiary).map_err(EVMError::Database)?;
                let score = beneficiary_balance.saturating_sub(initial_beneficiary_balance);

                // Get the simulation results
                let gas_used = trevm.result().gas_used();
                let cache = trevm.accept_state().into_db().into_cache();

                // Create the outcome
                Ok(SimOutcomeWithCache { identifier, score, cache, gas_used })
            }
            Err(e) => Err(SignetEthBundleError::from(e.into_error())),
        }
    }

    /// Simulates a bundle on the current environment.
    fn simulate_bundle(
        &self,
        identifier: u128,
        bundle: &SignetEthBundle,
    ) -> Result<SimOutcomeWithCache, SignetEthBundleError<SimDb<Db>>>
    where
        Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync,
    {
        let mut driver = SignetEthBundleDriver::new(bundle, self.finish_by);
        let trevm = self.create_with_block(&self.cfg, &self.block).unwrap();

        // Run the bundle
        let trevm = match driver.run_bundle(trevm) {
            Ok(result) => result,
            Err(e) => return Err(e.into_error()),
        };

        // Build the SimOutcome
        let score = driver.beneficiary_balance_increase();
        let gas_used = driver.total_gas_used();
        let cache = trevm.into_db().into_cache();

        Ok(SimOutcomeWithCache { identifier, score, cache, gas_used })
    }

    /// Simulates a transaction or bundle in the context of a block.
    fn simulate(
        &self,
        identifier: u128,
        item: &SimItem,
    ) -> Result<SimOutcomeWithCache, SignetEthBundleError<SimDb<Db>>> {
        match item {
            SimItem::Bundle(bundle) => self.simulate_bundle(identifier, bundle),
            SimItem::Tx(tx) => self.simulate_tx(identifier, tx),
        }
    }
}

impl<Db, Insp> SimEnvInner<Db, Insp>
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
