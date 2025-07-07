use std::time::Duration;
use crate::{env::SimEnv, BuiltBlock, SharedSimEnv, SimCache, SimDb};
use signet_types::constants::SignetSystemConstants;
use tokio::{select, time::Instant};
use tracing::{debug, info_span, trace, Instrument};
use trevm::{
    helpers::Ctx,
    revm::{inspector::NoOpInspector, DatabaseRef, Inspector},
    Block, Cfg,
};

/// The amount of time to sleep between simulation rounds when there are no items to simulate.
pub(crate) const SIM_SLEEP_MS: u64 = 50;

/// Builds a single block by repeatedly invoking [`SimEnv`].
#[derive(Debug)]
pub struct BlockBuild<Db, Insp = NoOpInspector> {
    /// The simulation environment.
    env: SharedSimEnv<Db, Insp>,

    /// The block being built.
    block: BuiltBlock,

    /// The deadline to produce a block by.
    finish_by: std::time::Instant,

    /// The maximum amount of gas to use in the built block
    max_gas: u64,
}

impl<Db, Insp> BlockBuild<Db, Insp>
where
    Db: DatabaseRef + Send + Sync + 'static,
    Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync + 'static,
{
    /// Create a new block building process.
    #[allow(clippy::too_many_arguments)] // sadge but.
    pub fn new<C, B>(
        db: Db,
        constants: SignetSystemConstants,
        cfg: C,
        block: B,
        finish_by: std::time::Instant,
        concurrency_limit: usize,
        sim_items: SimCache,
        max_gas: u64,
    ) -> Self
    where
        C: Cfg,
        B: Block,
    {
        let env = SimEnv::<Db, Insp>::new(
            db,
            constants,
            cfg,
            block,
            finish_by,
            concurrency_limit,
            sim_items,
        );
        let finish_by = env.finish_by();
        let number = env.block().number;
        Self { env: env.into(), block: BuiltBlock::new(number), finish_by, max_gas }
    }

    /// Run a simulation round, and accumulate the results into the block.
    async fn round(&mut self) {
        let gas_allowed = self.max_gas - self.block.gas_used();

        if let Some(simulated) = self.env.sim_round(gas_allowed).await {
            tracing::debug!(score = %simulated.score, gas_used = simulated.gas_used, identifier = %simulated.item.identifier(), "Adding item to block");
            self.block.ingest(simulated);
        }
    }

    /// Run several rounds, building
    pub async fn build(mut self) -> BuiltBlock {
        let mut i = 1;
        // Run until the deadline is reached.
        loop {
            let span = info_span!("build", round = i);
            let finish_by = self.finish_by.into();

            // Only simulate if there are items to simulate.
            // If there are not items, we sleep for the minimum of 50ms or until the deadline is reached,
            // and restart the loop.
            if self.env.sim_items().is_empty() {
                debug!("No items to simulate. Skipping simulation round");
                let sleep_until =
                    (Instant::now() + Duration::from_millis(SIM_SLEEP_MS)).min(finish_by);
                tokio::time::sleep_until(sleep_until).instrument(span).await;

                // If we sleep until the deadline, we just break the loop.
                if sleep_until == finish_by {
                    debug!("Deadline reached, stopping sim loop");
                    break;
                }

                continue;
            }

            // If there are items to simulate, we run a simulation round.
            let fut = self.round().instrument(span);

            select! {
                _ = tokio::time::sleep_until(finish_by) => {
                    debug!("Deadline reached, stopping sim loop");
                    break;
                },
                _ = fut => {
                    i+= 1;
                    let remaining = self.env.sim_items().len();
                    trace!(%remaining, round = i, "Round completed");
                }
            }
        }

        debug!(rounds = i, transactions = self.block.transactions.len(), "Building completed",);

        self.block
    }
}

#[cfg(test)]
mod test {
    use std::future::Future;

    use super::*;

    /// Compile-time check to ensure that the block building process is
    /// `Send`.
    fn _build_fut_is_send<Db, Insp>(b: BlockBuild<Db, Insp>)
    where
        Db: DatabaseRef + Send + Sync + 'static,
        Insp: Inspector<Ctx<SimDb<Db>>> + Default + Sync + 'static,
    {
        let _: Box<dyn Future<Output = BuiltBlock> + Send> = Box::new(b.build());
    }
}
