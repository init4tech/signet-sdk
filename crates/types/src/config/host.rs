use crate::config::{ConfigError, PredeployTokens};
use alloy::{genesis::Genesis, primitives::Address};
use serde_json::Value;

/// System addresses and other configuration details on the host chain.
///
/// These are system constants which may vary between chains, and are used to
/// determine the behavior of the chain, such as which contracts the Signet
/// node should listen to, and the addresses of system-priveleged tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostConfig {
    /// Host chain ID.
    chain_id: u64,
    /// Height at which the host chain deployed the rollup contracts.
    deploy_height: u64,
    /// Host address for the zenith contract
    zenith: Address,
    /// Host address for the orders contract
    orders: Address,
    /// Host address for the passage contract
    passage: Address,
    /// Host address for the transactor contract
    transactor: Address,
    /// Host chain tokens that are predeployed on the rollup.
    tokens: PredeployTokens,
}

impl std::fmt::Display for HostConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{ zenith: {}, orders: {}, passage: {}, transactor: {} }}",
            self.zenith, self.orders, self.passage, self.transactor
        )
    }
}

impl HostConfig {
    /// Create a new host configuration.
    pub const fn new(
        chain_id: u64,
        deploy_height: u64,
        zenith: Address,
        orders: Address,
        passage: Address,
        transactor: Address,
        tokens: PredeployTokens,
    ) -> Self {
        Self { chain_id, deploy_height, zenith, orders, passage, transactor, tokens }
    }

    /// Load the constants from a [`Genesis`].
    pub fn try_from_genesis(genesis: &Genesis) -> Result<Self, ConfigError> {
        let constants = genesis
            .config
            .extra_fields
            .get("signetConstants")
            .and_then(Value::as_object)
            .and_then(|v| v.get("host"))
            .ok_or_else(|| ConfigError::missing("signetConstants.host"))?;
        serde_json::from_value(constants.clone()).map_err(Into::into)
    }

    /// True if the contract is a system contract deployed on the host.
    pub const fn const_is_system_contract(&self, address: Address) -> bool {
        address.const_eq(&self.zenith)
            || address.const_eq(&self.orders)
            || address.const_eq(&self.passage)
            || address.const_eq(&self.transactor)
    }

    /// True if the contract is a system contract deployed on the host.
    pub fn is_system_contract(&self, address: Address) -> bool {
        address == self.zenith
            || address == self.orders
            || address == self.passage
            || address == self.transactor
    }

    /// Get the host chain ID.
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Get the height at which the host chain deployed the rollup contracts.
    pub const fn deploy_height(&self) -> u64 {
        self.deploy_height
    }

    /// Get the address of the zenith contract.
    pub const fn zenith(&self) -> Address {
        self.zenith
    }

    /// Get the address of the orders contract.
    pub const fn orders(&self) -> Address {
        self.orders
    }

    /// Get the address of the passage contract.
    pub const fn passage(&self) -> Address {
        self.passage
    }

    /// Get the address of the transactor contract.
    pub const fn transactor(&self) -> Address {
        self.transactor
    }

    /// Get the host tokens.
    pub const fn tokens(&self) -> PredeployTokens {
        self.tokens
    }
}
