use crate::{env::RollupEnv, outcome::SimulatedItem, HostEnv, SimCache, SimDb, SimEnv};
use core::fmt;
use std::{ops::Deref, sync::Arc};
use tokio::{select, sync::watch};
use tracing::{instrument, trace};
use trevm::{
    helpers::Ctx,
    revm::{inspector::NoOpInspector, DatabaseRef, Inspector},
};

/// A simulation environment.
///
/// Contains enough information to run a simulation.
pub struct SharedSimEnv<RuDb, HostDb, RuInsp = NoOpInspector, HostInsp = NoOpInspector> {
    inner: Arc<SimEnv<RuDb, HostDb, RuInsp, HostInsp>>,
}

impl<RuDb, HostDb, RuInsp, HostInsp> fmt::Debug for SharedSimEnv<RuDb, HostDb, RuInsp, HostInsp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedSimEnv")
            .field("finish_by", &self.inner.finish_by())
            .field("concurrency_limit", &self.inner.concurrency_limit())
            .finish_non_exhaustive()
    }
}

impl<RuDb, HostDb, RuInsp, HostInsp> Deref for SharedSimEnv<RuDb, HostDb, RuInsp, HostInsp> {
    type Target = SimEnv<RuDb, HostDb, RuInsp, HostInsp>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<RuDb, HostDb, RuInsp, HostInsp> From<SimEnv<RuDb, HostDb, RuInsp, HostInsp>>
    for SharedSimEnv<RuDb, HostDb, RuInsp, HostInsp>
where
    RuDb: DatabaseRef + Send + Sync + 'static,
    RuInsp: Inspector<Ctx<SimDb<RuDb>>> + Default + Sync + 'static,
    HostDb: DatabaseRef + Send + Sync + 'static,
    HostInsp: Inspector<Ctx<SimDb<HostDb>>> + Default + Sync + 'static,
{
    fn from(inner: SimEnv<RuDb, HostDb, RuInsp, HostInsp>) -> Self {
        Self { inner: Arc::new(inner) }
    }
}

impl<RuDb, HostDb, RuInsp, HostInsp> SharedSimEnv<RuDb, HostDb, RuInsp, HostInsp>
where
    RuDb: DatabaseRef + Send + Sync + 'static,
    RuInsp: Inspector<Ctx<SimDb<RuDb>>> + Default + Sync + 'static,
    HostDb: DatabaseRef + Send + Sync + 'static,
    HostInsp: Inspector<Ctx<SimDb<HostDb>>> + Default + Sync + 'static,
{
    /// Creates a new `SimEnv` instance.
    pub fn new(
        rollup: RollupEnv<RuDb, RuInsp>,
        host: HostEnv<HostDb, HostInsp>,
        finish_by: std::time::Instant,
        concurrency_limit: usize,
        sim_items: SimCache,
    ) -> Self {
        SimEnv::new(rollup, host, finish_by, concurrency_limit, sim_items).into()
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
            .accept_cache_ref(&outcome.rollup_cache)
            .ok()?;
        // Accept the aggregate fills and orders.
        Arc::get_mut(&mut self.inner)
            .expect("sims dropped already")
            .rollup_mut()
            .accept_aggregates(&outcome.bundle_fills, &outcome.bundle_orders)
            .expect("checked during simulation");

        Some(SimulatedItem { gas_used: outcome.gas_used, score: outcome.score, item })
    }
}
