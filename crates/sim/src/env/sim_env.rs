use crate::{env::RollupEnv, SimCache, SimDb, SimItem, SimOutcomeWithCache, TimeLimited};
use alloy::{consensus::TxEnvelope, hex};
use core::fmt;
use signet_bundle::{SignetEthBundle, SignetEthBundleDriver, SignetEthBundleError};
use signet_types::constants::SignetSystemConstants;
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tracing::{instrument, trace, trace_span};
use trevm::{
    helpers::Ctx,
    revm::{
        context::{
            result::{EVMError, ExecutionResult},
            BlockEnv, CfgEnv,
        },
        inspector::NoOpInspector,
        DatabaseRef, Inspector,
    },
    Block, BundleDriver, Cfg,
};

/// A simulation environment.
pub struct SimEnv<RuDb, RuInsp = NoOpInspector> {
    rollup: RollupEnv<RuDb, RuInsp>,

    /// The cache of items to simulate.
    sim_items: SimCache,

    /// Chain cfg to use for the simulation.
    cfg: CfgEnv,

    /// Block to use for the simulation.
    block: BlockEnv,

    /// The instant by which the simulation should finish.
    finish_by: std::time::Instant,

    /// The maximum number of concurrent simulations to run.
    concurrency_limit: usize,
}

impl<RuDb, RuInsp> fmt::Debug for SimEnv<RuDb, RuInsp> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimEnv")
            .field("finish_by", &self.finish_by)
            .field("concurrency_limit", &self.concurrency_limit)
            .finish_non_exhaustive()
    }
}

impl<RuDb, RuInsp> SimEnv<RuDb, RuInsp> {
    /// Creates a new `SimFactory` instance.
    pub fn new<C, B>(
        rollup: RollupEnv<RuDb, RuInsp>,
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

        Self { rollup, cfg, block, finish_by, concurrency_limit, sim_items }
    }

    /// Get a reference to the database.
    pub const fn rollup_mut(&mut self) -> &mut RollupEnv<RuDb, RuInsp> {
        &mut self.rollup
    }

    /// Get a reference to the system constants.
    pub const fn constants(&self) -> &SignetSystemConstants {
        self.rollup.constants()
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
    pub const fn set_finish_by(&mut self, timeout: std::time::Instant) {
        self.finish_by = timeout;
    }

    /// Get the concurrency limit.
    pub const fn concurrency_limit(&self) -> usize {
        self.concurrency_limit
    }
}

impl<RuDb, RuInsp> SimEnv<RuDb, RuInsp>
where
    RuDb: DatabaseRef + Send + Sync,
    RuInsp: Inspector<Ctx<SimDb<RuDb>>> + Default + Sync,
{
    fn rollup_evm(&self) -> signet_evm::EvmNeedsTx<SimDb<RuDb>, TimeLimited<RuInsp>> {
        self.rollup.create_evm(self.finish_by).fill_cfg(&self.cfg).fill_block(&self.block)
    }

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
        let trevm = self.rollup_evm();

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
                    ?cache_rank,
                    tx_hash = %transaction.hash(),
                    gas_used = gas_used,
                    score = %score,
                    reverted = !success,
                    halted,
                    halt_reason = ?if halted { halt_reason } else { None },
                    revert_reason = if !success { reason } else { None },
                    "Transaction simulation complete"
                );

                // Create the outcome
                Ok(SimOutcomeWithCache { cache_rank, score, cache, gas_used })
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
        let mut driver =
            SignetEthBundleDriver::new(bundle, self.constants().host_chain_id(), self.finish_by);
        let trevm = self.rollup_evm();

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
            ?cache_rank,
            uuid = %bundle.replacement_uuid().expect("Bundle must have a replacement UUID"),
            gas_used = gas_used,
            score = %score,
            "Bundle simulation successful"
        );

        Ok(SimOutcomeWithCache { cache_rank, score, cache, gas_used })
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

    #[instrument(skip_all)]
    pub(crate) fn sim_round(
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
            for (cache_rank, item) in active_sim.into_iter() {
                let c = candidates.clone();

                scope.spawn(move || {
                    let identifier = item.identifier();
                    let _ig = trace_span!(parent: outer_ref, "sim_task", %identifier).entered();

                    // If simulation is succesful, send the outcome via the
                    // channel.
                    match this_ref.simulate(cache_rank, &item) {
                        Ok(candidate) => {
                            if candidate.gas_used <= max_gas {
                                // shortcut return on success
                                let _ = c.blocking_send(candidate);
                                return;
                            }
                            trace!(gas_used = candidate.gas_used, max_gas, %identifier, "Gas limit exceeded");
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
