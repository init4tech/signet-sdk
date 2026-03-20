use crate::{
    cache::StateSource, env::RollupEnv, outcome::SimulatedItem, AcctInfo, HostEnv, SimCache, SimDb,
    SimEnv,
};
use alloy::primitives::Address;
use core::fmt;
use std::{ops::Deref, sync::Arc};
use tokio::{select, sync::watch};
use tracing::{debug, debug_span, instrument, trace, warn, Span};
use trevm::{
    db::TryCachingDb,
    helpers::Ctx,
    revm::{database::Cache, inspector::NoOpInspector, DatabaseRef, Inspector},
};

/// Composite async source that overlays the sim env's committed cache
/// on top of a fallback [`StateSource`].
///
/// Accounts whose state was modified by prior sim rounds (present in
/// the cache) are returned directly, avoiding async I/O. Accounts not
/// in the cache fall through to the asynchronous source.
struct CachedAsyncSource<'a, S> {
    cache: &'a Cache,
    fallback: &'a S,
}

impl<S: StateSource> StateSource for CachedAsyncSource<'_, S> {
    type Error = S::Error;

    #[instrument(level = "trace", skip_all, fields(%address, source = tracing::field::Empty))]
    async fn account_details(&self, address: &Address) -> Result<AcctInfo, Self::Error> {
        if let Some(acct) = self.cache.accounts.get(address) {
            Span::current().record("source", "cache_hit");
            return Ok(AcctInfo {
                nonce: acct.info.nonce,
                balance: acct.info.balance,
                has_code: acct.info.code_hash() != trevm::revm::primitives::KECCAK_EMPTY,
            });
        }
        Span::current().record("source", "rpc_fallback");
        self.fallback.account_details(address).await
    }
}

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
        finish_by: tokio::time::Instant,
        concurrency_limit: usize,
        sim_items: SimCache,
    ) -> Self {
        SimEnv::new(rollup, host, finish_by, concurrency_limit, sim_items).into()
    }

    /// Get a reference the simulation cache used by this builder.
    pub fn sim_items(&self) -> &SimCache {
        self.inner.sim_items()
    }

    /// Get a reference to the rollup environment.
    pub fn rollup_env(&self) -> &RollupEnv<RuDb, RuInsp> {
        self.inner.rollup_env()
    }

    /// Get a mutable reference to the rollup environment.
    pub fn rollup_env_mut(&mut self) -> &mut RollupEnv<RuDb, RuInsp> {
        Arc::get_mut(&mut self.inner).expect("sims dropped already").rollup_mut()
    }

    /// Get a reference to the host environment.
    pub fn host_env(&self) -> &HostEnv<HostDb, HostInsp> {
        self.inner.host_env()
    }

    /// Get a mutable reference to the host environment.
    pub fn host_env_mut(&mut self) -> &mut HostEnv<HostDb, HostInsp> {
        Arc::get_mut(&mut self.inner).expect("sims dropped already").host_mut()
    }

    /// Run a simulation round, returning the best item.
    ///
    /// Preflight validity checks (nonce/balance) are performed asynchronously
    /// using the provided [`StateSource`]s. This avoids the tokio I/O
    /// driver starvation deadlock that occurs when sync `DatabaseRef` calls
    /// go through `block_in_place` + `Handle::block_on`.
    pub async fn sim_round<AS, AH>(
        &mut self,
        max_gas: u64,
        max_host_gas: u64,
        async_ru_source: &AS,
        async_host_source: &AH,
    ) -> Option<SimulatedItem>
    where
        AS: StateSource,
        AH: StateSource,
    {
        let span = debug_span!(
            "sim_round",
            max_gas,
            max_host_gas,
            items_to_simulate = tracing::field::Empty,
            items_simulated_ok = tracing::field::Empty,
            items_simulated_err = tracing::field::Empty,
        )
        .or_current();

        // Overlay the sim env's committed cache so that accounts touched
        // by prior rounds (e.g. nonce bumps) are visible to the preflight
        // validity check without requiring async I/O.
        let ru_source = CachedAsyncSource {
            cache: self.inner.rollup_env().db().cache(),
            fallback: async_ru_source,
        };
        let host_source = CachedAsyncSource {
            cache: self.inner.host_env().db().cache(),
            fallback: async_host_source,
        };

        let active_sim = match self
            .inner
            .sim_items()
            .read_best_valid(self.inner.concurrency_limit(), &ru_source, &host_source)
            .await
        {
            Ok(items) => items,
            Err(error) => {
                warn!(%error, "preflight validity check failed");
                return None;
            }
        };

        span.record("items_to_simulate", active_sim.len());

        if active_sim.is_empty() {
            return None;
        }

        // These will be moved into the blocking task.
        let scope_span = span.clone();
        let this = self.inner.clone();
        let (best_tx, mut best_watcher) = watch::channel(None);

        // Spawn a blocking task to run the simulations.
        let sim_task = tokio::task::spawn_blocking(move || {
            scope_span.in_scope(|| this.sim_round(max_gas, max_host_gas, best_tx, active_sim))
        });

        // Either simulation is done, or we time out
        let sim_counts = select! {
            _ = tokio::time::sleep_until(self.finish_by()) => {
                span.in_scope(|| trace!("Sim round timed out"));
                None
            },
            result = sim_task => {
                span.in_scope(|| trace!("Sim round done"));
                result.ok()
            },
        };

        if let Some(counts) = sim_counts {
            span.record("items_simulated_ok", counts.ok);
            span.record("items_simulated_err", counts.err);
        }

        let _guard = span.entered();

        // Check what the current best outcome is.
        let best = best_watcher.borrow_and_update();
        trace!(score = %best.as_ref().map(|candidate| candidate.score).unwrap_or_default(), "Read outcome from channel");
        let outcome = best.as_ref()?;

        // Remove the item from the cache.
        let item = self.sim_items().remove(outcome.cache_rank)?;

        // We can expect here as all of our simulations are done and cleaned up.
        let inner = Arc::get_mut(&mut self.inner).expect("sims dropped already");

        // Accept the cache from the simulation.
        inner.rollup_mut().accept_cache_ref(&outcome.rollup_cache).ok()?;
        // Accept the host cache from the simulation.
        inner.host_mut().accept_cache_ref(&outcome.host_cache).ok()?;
        // Accept the aggregate fills and orders.
        inner
            .rollup_mut()
            .accept_aggregates(&outcome.bundle_fills, &outcome.bundle_orders)
            .expect("checked during simulation");

        debug!(
            score = %outcome.score,
            gas_used = outcome.gas_used,
            host_gas_used = outcome.host_gas_used,
            identifier = %item.identifier(),
            "Selected simulated item",
        );

        Some(SimulatedItem {
            gas_used: outcome.gas_used,
            host_gas_used: outcome.host_gas_used,
            score: outcome.score,
            item,
        })
    }
}
