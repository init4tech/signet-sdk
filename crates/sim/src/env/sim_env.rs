use crate::{env::RollupEnv, HostEnv, SimCache, SimDb, SimItem, SimOutcomeWithCache};
use alloy::{consensus::TxEnvelope, hex};
use core::fmt;
use signet_bundle::{SignetEthBundle, SignetEthBundleDriver, SignetEthBundleError};
use signet_evm::SignetInspector;
use signet_types::constants::SignetSystemConstants;
use std::{borrow::Cow, sync::Arc};
use tokio::sync::{mpsc, watch};
use tracing::{instrument, trace, trace_span};
use trevm::{
    helpers::Ctx,
    revm::{
        context::result::{EVMError, ExecutionResult},
        inspector::NoOpInspector,
        DatabaseRef, Inspector,
    },
    BundleDriver,
};

/// A simulation environment.
pub struct SimEnv<RuDb, HostDb, RuInsp = NoOpInspector, HostInsp = NoOpInspector> {
    /// The rollup environment.
    rollup: RollupEnv<RuDb, RuInsp>,

    /// The host environment.
    host: HostEnv<HostDb, HostInsp>,

    /// The cache of items to simulate.
    sim_items: SimCache,

    /// The instant by which the simulation should finish.
    finish_by: std::time::Instant,

    /// The maximum number of concurrent simulations to run.
    concurrency_limit: usize,
}

impl<RuDb, HostDb, RuInsp, HostInsp> fmt::Debug for SimEnv<RuDb, HostDb, RuInsp, HostInsp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimEnv")
            .field("finish_by", &self.finish_by)
            .field("concurrency_limit", &self.concurrency_limit)
            .finish_non_exhaustive()
    }
}

impl<RuDb, HostDb, RuInsp, HostInsp> SimEnv<RuDb, HostDb, RuInsp, HostInsp> {
    /// Create a new `SimEnv` instance.
    pub const fn new(
        rollup: RollupEnv<RuDb, RuInsp>,
        host: HostEnv<HostDb, HostInsp>,
        finish_by: std::time::Instant,
        concurrency_limit: usize,
        sim_items: SimCache,
    ) -> Self {
        Self { rollup, host, finish_by, concurrency_limit, sim_items }
    }

    /// Get a reference to the rollup environment.
    pub const fn rollup_env(&self) -> &RollupEnv<RuDb, RuInsp> {
        &self.rollup
    }

    /// Get a mutable reference to the rollup environment.
    pub const fn rollup_mut(&mut self) -> &mut RollupEnv<RuDb, RuInsp> {
        &mut self.rollup
    }

    /// Get a reference to the host environment.
    pub const fn host_env(&self) -> &HostEnv<HostDb, HostInsp> {
        &self.host
    }

    /// Get a mutable reference to the host environment.
    pub const fn host_mut(&mut self) -> &mut HostEnv<HostDb, HostInsp> {
        &mut self.host
    }

    /// Get a reference to the system constants.
    pub const fn constants(&self) -> &SignetSystemConstants {
        self.rollup.constants()
    }

    /// Get a reference to the cache of items to simulate.
    pub const fn sim_items(&self) -> &SimCache {
        &self.sim_items
    }

    /// Get the exectuion timeout.
    pub const fn finish_by(&self) -> std::time::Instant {
        self.finish_by
    }

    /// Set the execution timeout.
    pub const fn set_finish_by(&mut self, timeout: std::time::Instant) {
        self.finish_by = timeout;
    }

    /// Get the concurrency limit.
    pub const fn concurrency_limit(&self) -> usize {
        self.concurrency_limit
    }
}

impl<RuDb, HostDb, RuInsp, HostInsp> SimEnv<RuDb, HostDb, RuInsp, HostInsp>
where
    RuDb: DatabaseRef + Send + Sync,
    RuInsp: Inspector<Ctx<SimDb<RuDb>>> + Default + Sync,
    HostDb: DatabaseRef + Send + Sync,
    HostInsp: Inspector<Ctx<SimDb<HostDb>>> + Default + Sync,
{
    /// Simulates a transaction in the context of a block.
    ///
    /// This function runs the simulation in a separate thread and waits for
    /// the result or the deadline to expire.
    #[instrument(skip_all, fields(cache_rank, tx_hash = %transaction.hash()))]
    fn simulate_tx(
        &self,
        cache_rank: u128,
        transaction: &TxEnvelope,
    ) -> Result<SimOutcomeWithCache, SignetEthBundleError<SimDb<RuDb>>> {
        let trevm = self.rollup.create_evm(self.finish_by);

        // Get the initial beneficiary balance
        let beneficiary = trevm.beneficiary();
        let initial_beneficiary_balance =
            trevm.try_read_balance_ref(beneficiary).map_err(EVMError::Database)?;

        // If succesful, take the cache. If failed, return the error.
        match trevm.run_tx(transaction) {
            Ok(mut trevm) => {
                // Get the simulation results
                let gas_used = trevm.result().gas_used();
                let success = trevm.result().is_success();
                let reason = trevm.result().output().cloned().map(hex::encode);
                let halted = trevm.result().is_halt();
                let halt_reason = if let ExecutionResult::Halt { reason, .. } = trevm.result() {
                    Some(reason.clone())
                } else {
                    None
                };

                // We collect the orders and fills from the inspector, and check
                // them against the provided fill state. If the fills are
                // insufficient, we error out. Otherwise we'll return them as
                // part of the `SimOutcomeWithCache`, to allow _later_ stages to
                // process them (e.g., to update the fill state).
                let (bundle_fills, bundle_orders) =
                    trevm.inner_mut_unchecked().inspector.as_mut_detector().take_aggregates();

                self.rollup.fill_state().check_ru_tx_events(&bundle_fills, &bundle_orders)?;

                // We will later commit these to the trevm DB when the
                // SimOutcome is accepted.
                let cache = trevm.accept_state().into_db().into_cache();

                let beneficiary_balance = cache
                    .accounts
                    .get(&beneficiary)
                    .map(|acct| acct.info.balance)
                    .unwrap_or_default();
                let score = beneficiary_balance.saturating_sub(initial_beneficiary_balance);

                trace!(
                    gas_used,
                    score = %score,
                    reverted = !success,
                    halted,
                    halt_reason = ?if halted { halt_reason } else { None },
                    revert_reason = if !success { reason } else { None },
                    "Transaction simulation complete"
                );

                // Create the outcome
                Ok(SimOutcomeWithCache {
                    cache_rank,
                    score,
                    rollup_cache: cache,
                    host_cache: Default::default(),
                    host_gas_used: 0,
                    gas_used,
                    bundle_fills,
                    bundle_orders,
                })
            }
            Err(e) => Err(SignetEthBundleError::from(e.into_error())),
        }
    }

    /// Simulates a bundle on the current environment.
    #[instrument(skip_all, fields(cache_rank, uuid = bundle.replacement_uuid()))]
    fn simulate_bundle(
        &self,
        cache_rank: u128,
        bundle: &SignetEthBundle,
    ) -> Result<SimOutcomeWithCache, SignetEthBundleError<SimDb<RuDb>>>
    where
        RuInsp: Inspector<Ctx<SimDb<RuDb>>> + Default + Sync,
    {
        let trevm = self.rollup.create_evm(self.finish_by);

        let mut driver = SignetEthBundleDriver::new_with_fill_state(
            bundle,
            self.host.create_evm(self.finish_by),
            self.finish_by,
            Cow::Borrowed(self.rollup.fill_state()),
        );

        // Run the bundle
        let trevm = match driver.run_bundle(trevm) {
            Ok(result) => result,
            Err(e) => return Err(e.into_error()),
        };

        // Build the SimOutcome
        let score = driver.beneficiary_balance_increase();
        let outputs = driver.into_outputs();

        // This is redundant with the driver, however, we double check here.
        // If perf is hit too much we can remove.
        self.rollup
            .fill_state()
            .check_ru_tx_events(&outputs.bundle_fills, &outputs.bundle_orders)?;

        let host_cache = outputs.host_evm.map(|evm| evm.into_db().into_cache()).unwrap_or_default();
        trace!(
            gas_used = outputs.total_gas_used,
            host_gas_used = outputs.total_host_gas_used,
            %score,
            "Bundle simulation successful"
        );

        Ok(SimOutcomeWithCache {
            cache_rank,
            score: score.to(),
            rollup_cache: trevm.into_db().into_cache(),
            host_cache,
            gas_used: outputs.total_gas_used,
            host_gas_used: outputs.total_host_gas_used,
            bundle_fills: outputs.bundle_fills,
            bundle_orders: outputs.bundle_orders,
        })
    }

    /// Simulates a transaction or bundle in the context of a block.
    fn simulate(
        &self,
        cache_rank: u128,
        item: &SimItem,
    ) -> Result<SimOutcomeWithCache, SignetEthBundleError<SimDb<RuDb>>> {
        match item {
            SimItem::Bundle(bundle) => self.simulate_bundle(cache_rank, bundle),
            SimItem::Tx(tx) => self.simulate_tx(cache_rank, tx),
        }
    }

    pub(crate) fn sim_round(
        self: Arc<Self>,
        max_gas: u64,
        max_host_gas: u64,
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
        let this_ref = self.clone();

        std::thread::scope(|scope| {
            // Spawn a thread per bundle to simulate.
            for (cache_rank, item) in active_sim.into_iter() {
                let c = candidates.clone();
                let this_ref = this_ref.clone();
                scope.spawn(move || {
                    let identifier = item.identifier();
                    let _ig = trace_span!(parent: outer_ref, "sim_task", %identifier).entered();

                    // If simulation is succesful, send the outcome via the
                    // channel.

                    match this_ref.simulate(cache_rank, &item) {
                        Ok(candidate) if candidate.score.is_zero() => {
                            trace!("zero score candidate, skipping");
                        }
                        Ok(candidate) if candidate.host_gas_used > max_host_gas => {
                            trace!(
                                host_gas_used = candidate.host_gas_used,
                                max_host_gas,
                                "Host gas limit exceeded"
                            );
                        }
                        Ok(candidate) if candidate.gas_used > max_gas => {
                            trace!(gas_used = candidate.gas_used, max_gas, "Gas limit exceeded");
                        }
                        Ok(candidate) => {
                            // shortcut return on success
                            let _ = c.blocking_send(candidate);
                            return;
                        }
                        Err(e) => {
                            trace!(?identifier, %e, "Simulation failed");
                        }
                    };
                    // fall through applies to all errors, occurs if
                    // the simulation fails or the gas limit is exceeded.
                    this_ref.sim_items.remove(cache_rank);
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
                    let current_cache_rank = current.as_ref().map(|c| c.cache_rank);

                    let changed = candidate.score > best_score;
                    if changed {
                        trace!(
                            old_best = ?best_score,
                            old_cache_rank = current_cache_rank,
                            new_best = %candidate.score,
                            new_cache_rank = candidate.cache_rank,
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
