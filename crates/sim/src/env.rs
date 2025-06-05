use crate::{outcome::SimulatedItem, InnerDb, SimCache, SimDb, SimItem, SimOutcomeWithCache};
use alloy::{consensus::TxEnvelope, hex};
use core::fmt;
use signet_bundle::{SignetEthBundle, SignetEthBundleDriver, SignetEthBundleError};
use signet_evm::SignetLayered;
use signet_types::constants::SignetSystemConstants;
use std::{convert::Infallible, marker::PhantomData, ops::Deref, sync::Arc, time::Instant};
use tokio::{
    select,
    sync::{mpsc, watch},
};
use tracing::{debug, error, instrument, trace, trace_span};
use trevm::{
    db::{cow::CacheOnWrite, TryCachingDb},
    helpers::Ctx,
    inspectors::{Layered, TimeLimit},
    revm::{
        context::{
            result::{EVMError, ExecutionResult},
            BlockEnv, CfgEnv,
        },
        database::{Cache, CacheDB},
        inspector::NoOpInspector,
        DatabaseRef, Inspector,
    },
    Block, BundleDriver, Cfg, DbConnect, EvmFactory,
};

/// A simulation environment.
///
/// Contains enough information to run a simulation.
pub struct SharedSimEnv<Db, Insp = NoOpInspector> {
    inner: Arc<SimEnv<Db, Insp>>,
}

impl<Db, Insp> fmt::Debug for SharedSimEnv<Db, Insp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimEnv")
            .field("finish_by", &self.inner.finish_by)
            .field("concurrency_limit", &self.inner.concurrency_limit)
            .finish_non_exhaustive()
    }
}

impl<Db, Insp> Deref for SharedSimEnv<Db, Insp> {
    type Target = SimEnv<Db, Insp>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<Db, Insp> From<SimEnv<Db, Insp>> for SharedSimEnv<Db, Insp>
where
    Db: DatabaseRef + Send + Sync + 'static,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync + 'static,
{
    fn from(inner: SimEnv<Db, Insp>) -> Self {
        Self { inner: Arc::new(inner) }
    }
}

impl<Db, Insp> SharedSimEnv<Db, Insp>
where
    Db: DatabaseRef + Send + Sync + 'static,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync + 'static,
{
    /// Creates a new `SimEnv` instance.
    pub fn new<C, B>(
        db: Db,
        constants: SignetSystemConstants,
        cfg: C,
        block: B,
        finish_by: std::time::Instant,
        concurrency_limit: usize,
        sim_items: SimCache,
    ) -> Self
    where
        C: Cfg,
        B: Block,
    {
        SimEnv::new(db, constants, cfg, block, finish_by, concurrency_limit, sim_items).into()
    }

    /// Run a simulation round, returning the best item.
    #[instrument(skip(self))]
    pub async fn sim_round(&mut self, max_gas: u64) -> Option<SimulatedItem> {
        let (best_tx, mut best_watcher) = watch::channel(None);

        let this = self.inner.clone();

        // Spawn a blocking task to run the simulations.
        let sim_task = tokio::task::spawn_blocking(move || this.sim_round(max_gas, best_tx));

        // Either simulation is done, or we time out
        select! {
            _ = tokio::time::sleep_until(self.finish_by.into()) => {
                trace!("Sim round timed out");
            },
            _ = sim_task => {
                trace!("Sim round done");
            },
        }

        // Check what the current best outcome is.
        let best = best_watcher.borrow_and_update();
        trace!(score = %best.as_ref().map(|candidate| candidate.score).unwrap_or_default(), "Read outcome from channel");
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
pub struct SimEnv<Db, Insp = NoOpInspector> {
    /// The database to use for the simulation. This database will be wrapped
    /// in [`CacheOnWrite`] databases for each simulation.
    db: InnerDb<Db>,

    /// The cache of items to simulate.
    sim_items: SimCache,

    /// The system constants for the Signet network.
    constants: SignetSystemConstants,

    /// Chain cfg to use for the simulation.
    cfg: CfgEnv,

    /// Block to use for the simulation.
    block: BlockEnv,

    /// The instant by which the simulation should finish.
    finish_by: std::time::Instant,

    /// The maximum number of concurrent simulations to run.
    concurrency_limit: usize,

    /// Spooky ghost inspector.
    _pd: PhantomData<fn() -> Insp>,
}

impl<Db, Insp> fmt::Debug for SimEnv<Db, Insp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimEnvInner")
            .field("finish_by", &self.finish_by)
            .field("concurrency_limit", &self.concurrency_limit)
            .finish_non_exhaustive()
    }
}

impl<Db, Insp> SimEnv<Db, Insp> {
    /// Creates a new `SimFactory` instance.
    pub fn new<C, B>(
        db: Db,
        constants: SignetSystemConstants,
        cfg_ref: C,
        block_ref: B,
        finish_by: std::time::Instant,
        concurrency_limit: usize,
        sim_items: SimCache,
    ) -> Self
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
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }

    /// Get a reference to the cache of items to simulate.
    pub const fn sim_items(&self) -> &SimCache {
        &self.sim_items
    }

    /// Get a reference to the chain cfg.
    pub const fn cfg(&self) -> &CfgEnv {
        &self.cfg
    }

    /// Get a reference to the block.
    pub const fn block(&self) -> &BlockEnv {
        &self.block
    }

    /// Get the exectuion timeout.
    pub const fn finish_by(&self) -> std::time::Instant {
        self.finish_by
    }

    /// Set the execution timeout.
    pub fn set_finish_by(&mut self, timeout: std::time::Instant) {
        self.finish_by = timeout;
    }
}

impl<Db, Insp> DbConnect for SimEnv<Db, Insp>
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

impl<Db, Insp> EvmFactory for SimEnv<Db, Insp>
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

impl<Db, Insp> SimEnv<Db, Insp>
where
    Db: DatabaseRef + Send + Sync,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync,
{
    /// Simulates a transaction in the context of a block.
    ///
    /// This function runs the simulation in a separate thread and waits for
    /// the result or the deadline to expire.
    #[instrument(skip_all, fields(identifier, tx_hash = %transaction.hash()))]
    fn simulate_tx(
        &self,
        identifier: u128,
        transaction: &TxEnvelope,
    ) -> Result<SimOutcomeWithCache, SignetEthBundleError<SimDb<Db>>> {
        let trevm = self.create_with_block(&self.cfg, &self.block).unwrap();
        debug!(tx_hash = ?transaction.hash(), "initialized trevm env for transaction");

        // Get the initial beneficiary balance
        let beneficiary = trevm.beneficiary();
        let initial_beneficiary_balance =
            trevm.try_read_balance_ref(beneficiary).map_err(EVMError::Database)?;

        // If succesful, take the cache. If failed, return the error.
        match trevm.run_tx(transaction) {
            Ok(trevm) => {
                // Get the simulation results
                let gas_used = trevm.result().gas_used();
                let success = trevm.result().is_success();
                let reason = trevm.result().output().cloned().map(hex::encode);
                let halted = trevm.result().is_halt();
                let halt_reason = if let ExecutionResult::Halt { reason, .. } = trevm.result() {
                    Some(reason)
                } else {
                    None
                }
                .cloned();

                let cache = trevm.accept_state().into_db().into_cache();

                let beneficiary_balance = cache
                    .accounts
                    .get(&beneficiary)
                    .map(|acct| acct.info.balance)
                    .unwrap_or_default();
                let score = beneficiary_balance.saturating_sub(initial_beneficiary_balance);

                trace!(
                    gas_used = gas_used,
                    score = %score,
                    reverted = !success,
                    halted,
                    halt_reason = ?if halted { halt_reason } else { None },
                    revert_reason = if !success { reason } else { None },
                    "Simulation complete"
                );

                // Create the outcome
                Ok(SimOutcomeWithCache { identifier, score, cache, gas_used })
            }
            Err(e) => {
                let err = e.into_error();
                error!(?err, "Simulation failed");
                Err(SignetEthBundleError::from(err))
            }
        }
    }

    /// Simulates a bundle on the current environment.
    #[instrument(skip_all, fields(identifier, uuid = bundle.replacement_uuid()))]
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

        trace!(
            gas_used = gas_used,
            score = %score,
            "Bundle simulation successful"
        );

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

    #[instrument(skip_all)]
    fn sim_round(
        self: Arc<Self>,
        max_gas: u64,
        best_tx: watch::Sender<Option<SimOutcomeWithCache>>,
    ) {
        // Pull the `n` best items from the cache.
        let active_sim = self.sim_items.read_best(self.concurrency_limit);

        // Create a channel to send the results back.
        let (candidates, mut candidates_rx) = mpsc::channel(self.concurrency_limit);

        let outer = trace_span!("sim_thread", candidates = active_sim.len());
        let outer_ref = &outer;
        let _og = outer.enter();

        // to be used in the scope
        let this_ref = &self;

        std::thread::scope(move |scope| {
            // Spawn a thread per bundle to simulate.
            for (identifier, item) in active_sim.into_iter() {
                let c = candidates.clone();

                scope.spawn(move || {
                    let _ig = trace_span!(parent: outer_ref, "sim_task", identifier = %identifier)
                        .entered();

                    // If simulation is succesful, send the outcome via the
                    // channel.
                    match this_ref.simulate(identifier, &item) {
                        Ok(candidate) => {
                            if candidate.gas_used <= max_gas {
                                // shortcut return on success
                                let _ = c.blocking_send(candidate);
                                return;
                            }
                            trace!(gas_used = candidate.gas_used, max_gas, "Gas limit exceeded");
                        }
                        Err(e) => {
                            trace!(?identifier, ?e, "Simulation failed");
                        }
                    };
                    // fall through applies to all errors, occurs if
                    // the simulation fails or the gas limit is exceeded.
                    this_ref.sim_items.remove(identifier);
                });
            }
            // Drop the TX so that the channel is closed when all threads
            // are done.
            drop(candidates);

            // Wait for each thread to finish. Find the best outcome.
            while let Some(candidate) = candidates_rx.blocking_recv() {
                // Update the best score and send it to the channel.
                let _ = best_tx.send_if_modified(|current| {
                    let best_score = current.as_ref().map(|c| c.score).unwrap_or_default();
                    let current_id = current.as_ref().map(|c| c.identifier);

                    let changed = candidate.score > best_score;
                    if changed {
                        trace!(
                            old_best = ?best_score,
                            old_identifier = current_id,
                            new_best = %candidate.score,
                            identifier = candidate.identifier,
                            "Found better candidate"
                        );
                        *current = Some(candidate);
                    }
                    changed
                });
            }
        });
    }
}

impl<Db, Insp> SimEnv<Db, Insp>
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
