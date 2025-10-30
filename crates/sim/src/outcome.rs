use crate::SimItem;
use alloy::primitives::U256;
use signet_types::{AggregateFills, AggregateOrders};
use trevm::revm::database::Cache;

/// A simulation outcome that includes the score, gas used, and a cache of
/// state changes.
#[derive(Debug, Clone)]
pub struct SimOutcomeWithCache {
    /// The key for the item in the [`SimCache`].
    pub cache_rank: u128,

    /// The score of the simulation, a [`U256`] value that represents the
    /// increase in the beneficiary's balance.
    pub score: U256,

    /// The total amount of gas used by the simulation.
    pub gas_used: u64,

    /// The result of the simulation, a [`Cache`] containing state changes that
    /// can be applied.
    pub rollup_cache: Cache,

    /// The result of the bundle host simulation a [`Cache`] containing state
    /// changes that can be applied.
    pub host_cache: Cache,

    /// The aggregate fills after simulation.
    pub bundle_fills: AggregateFills,

    /// The aggregate orders after simulation.
    pub bundle_orders: AggregateOrders,
}

/// An item after simulation, containing the score and gas used.
#[derive(Debug, Clone)]
pub struct SimulatedItem {
    /// The score of the simulation, a [`U256`] value that represents the
    /// increase in the beneficiary's balance.
    pub score: U256,

    /// The total amount of gas used by the simulation.
    pub gas_used: u64,

    /// The transaction or bundle that was simulated.
    pub item: SimItem,
}
