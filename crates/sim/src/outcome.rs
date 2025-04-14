use alloy::primitives::U256;
use trevm::revm::database::Cache;

pub struct SimOutcome {
    /// The transaction or bundle that was simulated, as in the cache.
    pub identifier: u128,

    /// The score of the simulation, a [`U256`] value that represents the
    /// increase in the beneficiary's balance.
    pub score: U256,
}

pub struct SimOutcomeWithCache {
    /// The transaction or bundle that was simulated, as in the cache.
    pub identifier: u128,

    /// The score of the simulation, a [`U256`] value that represents the
    /// increase in the beneficiary's balance.
    pub score: U256,

    /// The result of the simulation, a [`Cache`] containing state changes that
    /// can be applied.
    pub cache: Cache,
}
