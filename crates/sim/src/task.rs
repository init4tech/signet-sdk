use crate::{BuiltBlock, SimDb, SimEnv};
use tokio::select;
use trevm::{
    helpers::Ctx,
    revm::{inspector::NoOpInspector, DatabaseRef, Inspector},
};

/// Builds a single block by repeatedly invoking [`SimEnv`].
pub struct BlockBuild<Db, Insp = NoOpInspector> {
    env: SimEnv<Db, Insp>,
    block: BuiltBlock,

    finish_by: std::time::Instant,
    max_gas: u64,
}

impl<Db, Insp> BlockBuild<Db, Insp>
where
    Db: DatabaseRef + Send + Sync + 'static,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync + 'static,
{
    /// Create a new simulation task.
    pub const fn new(env: SimEnv<Db, Insp>, finish_by: std::time::Instant, max_gas: u64) -> Self {
        Self { env, block: BuiltBlock::new(), finish_by, max_gas }
    }

    /// Run a simulation round, and accumulate the results into the block.
    async fn round(&mut self, finish_by: std::time::Instant) {
        let gas_allowed = self.max_gas - self.block.gas_used();

        if let Some(simulated) = self.env.sim_round(finish_by, gas_allowed).await {
            tracing::debug!(score = %simulated.score, gas_used = simulated.gas_used, "Adding item to block");
            self.block.ingest(simulated);
        }
    }

    /// Run several rounds, building
    pub async fn build(mut self) -> BuiltBlock {
        // Run until the deadline is reached.
        loop {
            select! {
                _ = tokio::time::sleep_until(self.finish_by.into()) => break,
                _ = self.round(self.finish_by) => {}
            }
        }

        self.block
    }
}
