use alloy::primitives::U256;
use trevm::revm::database::Cache;

use crate::SimItem;

/// A simulation outcome that includes the score, gas used, and a cache of
/// state changes.
#[derive(Debug, Clone)]
pub struct SimOutcomeWithCache {
    /// The transaction or bundle that was simulated, as in the cache.
    pub identifier: u128,

    /// The score of the simulation, a [`U256`] value that represents the
    /// increase in the beneficiary's balance.
    pub score: U256,

    /// The result of the simulation, a [`Cache`] containing state changes that
    /// can be applied.
    pub cache: Cache,

    /// The total amount of gas used by the simulation.
    pub gas_used: u64,
}

/// An item after simulation, containing the score and gas used.
#[derive(Debug, Clone)]
pub struct SimulatedItem {
    pub score: U256,
    pub gas_used: u64,
    pub item: SimItem,
}
