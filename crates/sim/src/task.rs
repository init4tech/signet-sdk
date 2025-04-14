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
}

impl<Db, Insp> BlockBuild<Db, Insp>
where
    Db: DatabaseRef + Send + Sync + 'static,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync + 'static,
{
    /// Create a new simulation task.
    pub const fn new(env: SimEnv<Db, Insp>) -> Self {
        Self { env, block: BuiltBlock::new() }
    }

    /// Run a simulation round, and accumulate the results into the block.
    pub async fn round(&mut self, finish_by: std::time::Instant) {
        if let Some((score, item)) = self.env.sim_round(finish_by).await {
            tracing::debug!(%score, "Adding item to block");
            self.block.ingest(item);
        }
    }

    pub async fn build(&mut self) {}
}
