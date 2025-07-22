use crate::types::{ConfigError, KnownChains, ParseChainError, RollupTokens};
use alloy::{
    genesis::Genesis,
    primitives::{address, Address},
};
use serde_json::Value;
use std::str::FromStr;

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
    tokens: RollupTokens,
}

impl RollupConstants {
    /// Create a new rollup configuration.
    pub const fn new(
        chain_id: u64,
        orders: Address,
        passage: Address,
        base_fee_recipient: Address,
        tokens: RollupTokens,
    ) -> Self {
        Self { chain_id, orders, passage, base_fee_recipient, tokens }
    }

    /// Get the hard-coded Pecorino rollup constants.
    pub const fn pecorino() -> Self {
        crate::chains::pecorino::ROLLUP
    }

    /// Get the hard-coded local test rollup constants.
    #[cfg(any(test, feature = "test-utils"))]
    pub const fn test() -> Self {
        crate::chains::test_utils::ROLLUP
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
    pub const fn tokens(&self) -> RollupTokens {
        self.tokens
    }

    /// Get the address of the minter.
    pub const fn minter(&self) -> Address {
        MINTER_ADDRESS
    }
}

impl TryFrom<KnownChains> for RollupConstants {
    type Error = ParseChainError;

    fn try_from(chain: KnownChains) -> Result<Self, Self::Error> {
        match chain {
            KnownChains::Pecorino => Ok(Self::pecorino()),
            #[cfg(any(test, feature = "test-utils"))]
            KnownChains::Test => Ok(Self::test()),
        }
    }
}

impl FromStr for RollupConstants {
    type Err = ParseChainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<KnownChains>()?.try_into()
    }
}
