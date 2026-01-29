use crate::{env::SimEnv, BuiltBlock, HostEnv, RollupEnv, SharedSimEnv, SimCache, SimDb};
use std::time::Duration;
use tokio::select;
use tracing::{debug, trace};
use trevm::{
    helpers::Ctx,
    revm::{inspector::NoOpInspector, DatabaseRef, Inspector},
};

/// The amount of time to sleep between simulation rounds when there are no items to simulate.
pub(crate) const SIM_SLEEP_MS: u64 = 50;

/// Builds a single block by repeatedly invoking [`SimEnv`].
#[derive(Debug)]
pub struct BlockBuild<RuDb, HostDb, RuInsp = NoOpInspector, HostInsp = NoOpInspector> {
    /// The simulation environment.
    env: SharedSimEnv<RuDb, HostDb, RuInsp, HostInsp>,

    /// The block being built.
    block: BuiltBlock,

    /// The deadline to produce a block by.
    finish_by: tokio::time::Instant,

    /// The maximum amount of gas to use in the built block
    max_gas: u64,

    /// The maximum amount of host gas to use in the user portion of the built
    /// block, not including overhead for the signet RU block submission.
    max_host_gas: u64,
}

impl<RuDb, HostDb, RuInsp, HostInsp> BlockBuild<RuDb, HostDb, RuInsp, HostInsp>
where
    RuDb: DatabaseRef + Send + Sync + 'static,
    RuInsp: Inspector<Ctx<SimDb<RuDb>>> + Default + Sync + 'static,
    HostDb: DatabaseRef + Send + Sync + 'static,
    HostInsp: Inspector<Ctx<SimDb<HostDb>>> + Default + Sync + 'static,
{
    /// Create a new block building process.
    pub fn new(
        rollup: RollupEnv<RuDb, RuInsp>,
        host: HostEnv<HostDb, HostInsp>,
        finish_by: tokio::time::Instant,
        concurrency_limit: usize,
        sim_items: SimCache,
        max_gas: u64,
        max_host_gas: u64,
    ) -> Self {
        let number = rollup.block().number;

        let env = SimEnv::<RuDb, HostDb, RuInsp, HostInsp>::new(
            rollup,
            host,
            finish_by,
            concurrency_limit,
            sim_items,
        );
        let finish_by = env.finish_by();
        Self {
            env: env.into(),
            block: BuiltBlock::new(number.to()),
            finish_by,
            max_gas,
            max_host_gas,
        }
    }

    /// Get the maximum gas limit for the block being built.
    pub const fn max_gas(&self) -> u64 {
        self.max_gas
    }

    /// Set the maximum gas limit for the block being built.
    pub const fn set_max_gas(&mut self, max_gas: u64) {
        self.max_gas = max_gas;
    }

    /// Get the maximum host gas limit for the block being built.
    pub const fn max_host_gas(&self) -> u64 {
        self.max_host_gas
    }

    /// Set the maximum host gas limit for the block being built.
    pub const fn set_max_host_gas(&mut self, max_host_gas: u64) {
        self.max_host_gas = max_host_gas;
    }

    /// Get a reference the simulation cache used by this builder.
    pub fn sim_items(&self) -> &SimCache {
        self.env.sim_items()
    }

    /// Get a reference to the rollup environment.
    pub fn rollup_env(&self) -> &RollupEnv<RuDb, RuInsp> {
        self.env.rollup_env()
    }

    /// Get a reference to the host environment.
    pub fn host_env(&self) -> &HostEnv<HostDb, HostInsp> {
        self.env.host_env()
    }

    /// Consume the builder and return the built block.
    ///
    /// This should generally not be called directly; use [`BlockBuild::build`]
    /// instead.
    pub fn into_block(self) -> BuiltBlock {
        self.block
    }

    /// Run a simulation round, and accumulate the results into the block.
    async fn round(&mut self) {
        let gas_allowed = self.max_gas - self.block.gas_used();
        let host_gas_allowed = self.max_host_gas - self.block.host_gas_used();

        if let Some(simulated) = self.env.sim_round(gas_allowed, host_gas_allowed).await {
            debug!(
                score = %simulated.score,
                gas_used = simulated.gas_used,
                host_gas_used = simulated.host_gas_used,
                identifier = %simulated.item.identifier(),
                "Adding item to block"
            );
            self.block.ingest(simulated);
        }
    }

    /// Run several rounds, building a block by iteratively adding simulated
    /// items.
    ///
    /// This version returns self to allow inspection of the building process.
    /// It does nothing if the block already has transactions (i.e. this
    /// function should be idempotent).
    pub async fn run_build(mut self) -> Self {
        if !self.block.transactions.is_empty() {
            debug!(
                transactions = self.block.transactions.len(),
                "Starting block build with pre-existing transactions",
            );
            return self;
        }
        let mut i = 1;
        // Run until the deadline is reached.
        loop {
            let finish_by = self.finish_by;

            let next_round_time = tokio::time::Instant::now() + Duration::from_millis(SIM_SLEEP_MS);

            // If the next round time is past the deadline, we stop the simulation loop.
            // This will stop the simulation even if there are items, but that is an acceptable tradeoff
            // as we must ensure there's enough time to submit the blob to the host chain.
            if next_round_time >= finish_by {
                debug!("Next round time is past the deadline, stopping sim loop");
                break;
            }

            // Only simulate if there are items to simulate.
            // If there are not items, we sleep for [`SIM_SLEEP_MS`] and restart the loop.
            if self.env.sim_items().is_empty() {
                tokio::time::sleep_until(next_round_time).await;
                continue;
            }

            // If there are items to simulate, we run a simulation round.
            let fut = self.round();

            select! {
                biased;
                _ = tokio::time::sleep_until(finish_by) => {
                    // This event is not a round event. It's a control flow
                    // event for the outer loop. As such it's not in span
                    debug!("Deadline reached, stopping sim loop");
                    break;
                },
                _ = fut => {
                    i += 1;
                    let remaining_items = self.env.sim_items().len();
                    trace!(remaining_items, "Round completed");
                }
            }
        }

        debug!(rounds = i, transactions = self.block.transactions.len(), "Building completed",);
        self
    }

    /// Run several rounds, building a block by iteratively adding simulated
    /// items.
    pub async fn build(self) -> BuiltBlock {
        self.run_build().await.block
    }
}

#[cfg(test)]
mod test {
    use std::future::Future;

    use super::*;

    /// Compile-time check to ensure that the block building process is
    /// `Send`.
    fn _build_fut_is_send<RuDb, HostDb, RuInsp, HostInsp>(
        b: BlockBuild<RuDb, HostDb, RuInsp, HostInsp>,
    ) where
        RuDb: DatabaseRef + Send + Sync + 'static,
        RuInsp: Inspector<Ctx<SimDb<RuDb>>> + Default + Sync + 'static,
        HostDb: DatabaseRef + Send + Sync + 'static,
        HostInsp: Inspector<Ctx<SimDb<HostDb>>> + Default + Sync + 'static,
    {
        let _: Box<dyn Future<Output = BuiltBlock> + Send> = Box::new(b.build());
    }
}
