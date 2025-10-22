use crate::{env::RollupEnv, outcome::SimulatedItem, SimCache, SimDb, SimEnv};
use core::fmt;
use std::{ops::Deref, sync::Arc};
use tokio::{select, sync::watch};
use tracing::{instrument, trace};
use trevm::{
    helpers::Ctx,
    revm::{inspector::NoOpInspector, DatabaseRef, Inspector},
    Block, Cfg,
};

/// A simulation environment.
///
/// Contains enough information to run a simulation.
pub struct SharedSimEnv<Db, Insp = NoOpInspector> {
    inner: Arc<SimEnv<Db, Insp>>,
}

impl<Db, Insp> fmt::Debug for SharedSimEnv<Db, Insp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedSimEnv")
            .field("finish_by", &self.inner.finish_by())
            .field("concurrency_limit", &self.inner.concurrency_limit())
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
        rollup: RollupEnv<Db, Insp>,
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
        SimEnv::new(rollup, cfg, block, finish_by, concurrency_limit, sim_items).into()
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
            _ = tokio::time::sleep_until(self.finish_by().into()) => {
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
        let item = self.sim_items().remove(outcome.cache_rank)?;
        // Accept the cache from the simulation.
        Arc::get_mut(&mut self.inner)
            .expect("sims dropped already")
            .rollup_mut()
            .accept_cache_ref(&outcome.cache)
            .ok()?;
        // Accept the aggregate fills and orders.
        Arc::get_mut(&mut self.inner)
            .expect("sims dropped already")
            .rollup_mut()
            .accept_aggregates(&outcome.fills, &outcome.orders)
            .expect("checked during simulation");

        Some(SimulatedItem { gas_used: outcome.gas_used, score: outcome.score, item })
    }
}
