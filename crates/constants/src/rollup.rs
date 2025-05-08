use crate::{ConfigError, PredeployTokens};
use alloy::{
    genesis::Genesis,
    primitives::{address, Address},
};
use serde_json::Value;

/// System address with permission to mint tokens. This is the address from
/// which the node will issue transactions to mint ETH or ERC20 tokens.
// NB: the hex is: tokenadmin
pub const MINTER_ADDRESS: Address = address!("00000000000000000000746f6b656e61646d696e");

/// Configuration details for the rollup chain.
///
/// These are system constants which may vary between chains, and are used to
/// determine the behavior of the chain, such as which contracts the Signet
/// node should listen to, and the addresses of system-priveleged tokens.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollupConstants {
    /// Rollup chain ID.
    chain_id: u64,
    /// Address of the orders contract.
    orders: Address,
    /// Address of the passage contract.
    passage: Address,
    /// Address of the base fee recipient.
    base_fee_recipient: Address,
    /// Address of the pre-deployed tokens.
    tokens: PredeployTokens,
}

impl RollupConstants {
    /// Create a new rollup configuration.
    pub const fn new(
        chain_id: u64,
        orders: Address,
        passage: Address,
        base_fee_recipient: Address,
        tokens: PredeployTokens,
    ) -> Self {
        Self { chain_id, orders, passage, base_fee_recipient, tokens }
    }

    /// Load the constants from a [`Genesis`].
    pub fn try_from_genesis(genesis: &Genesis) -> Result<Self, ConfigError> {
        let constants = genesis
            .config
            .extra_fields
            .get("signetConstants")
            .and_then(Value::as_object)
            .and_then(|v| v.get("rollup"))
            .ok_or_else(|| ConfigError::missing("signetConstants.rollup"))?;
        serde_json::from_value(constants.clone()).map_err(Into::into)
    }

    /// Get the address of the orders contract.
    pub const fn orders(&self) -> Address {
        self.orders
    }

    /// Get the address of the passage contract.
    pub const fn passage(&self) -> Address {
        self.passage
    }

    /// Get the address of the base fee recipient.
    pub const fn base_fee_recipient(&self) -> Address {
        self.base_fee_recipient
    }

    /// Get the rollup chain ID.
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// True if the contract is a system contract deployed on the rollup.
    pub const fn const_is_system_contract(&self, address: Address) -> bool {
        address.const_eq(&self.orders) || address.const_eq(&self.passage)
    }

    /// True if the contract is a system contract deployed on the rollup.
    pub fn is_system_contract(&self, address: Address) -> bool {
        address == self.orders || address == self.passage
    }

    /// Get the address of the pre-deployed tokens.
    pub const fn tokens(&self) -> PredeployTokens {
        self.tokens
    }

    /// Get the address of the minter.
    pub const fn minter(&self) -> Address {
        MINTER_ADDRESS
    }
}
