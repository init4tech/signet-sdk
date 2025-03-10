mod error;
pub use error::ConfigError;

mod host;
pub use host::HostConfig;

mod rollup;
pub use rollup::{RollupConfig, MINTER_ADDRESS};

mod tokens;
pub use tokens::{PermissionedToken, PredeployTokens};

use crate::PairedHeights;
use alloy::{
    genesis::Genesis,
    primitives::{Address, U256},
};

/// Signet constants.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct SignetSystemConstants {
    /// Host constants.
    host: HostConfig,
    /// Rollup constants.
    rollup: RollupConfig,
}

impl SignetSystemConstants {
    /// Create a new set of constants.
    pub const fn new(host: HostConfig, rollup: RollupConfig) -> Self {
        Self { host, rollup }
    }

    /// Load the constants from a [`Genesis`].
    pub fn try_from_genesis(genesis: &Genesis) -> Result<Self, ConfigError> {
        let k = "signetConstants";
        let constants =
            genesis.config.extra_fields.get(k).ok_or_else(|| ConfigError::missing(k))?;
        serde_json::from_value(constants.clone()).map_err(Into::into)
    }

    /// Get the host addresses.
    pub const fn host(&self) -> HostConfig {
        self.host
    }

    /// Get the rollup addresses.
    pub const fn rollup(&self) -> RollupConfig {
        self.rollup
    }

    /// True if the contract is a system contract deployed on the rollup.
    pub const fn const_is_ru_system_contract(&self, address: Address) -> bool {
        self.rollup.const_is_system_contract(address)
    }

    /// True if the contract is a system contract deployed on the host.
    pub const fn const_is_host_system_contract(&self, address: Address) -> bool {
        self.host.const_is_system_contract(address)
    }

    /// True if the contract is a system contract deployed on the rollup.
    pub fn is_ru_system_contract(&self, address: Address) -> bool {
        self.rollup.is_system_contract(address)
    }

    /// True if the contract is a system contract deployed on the host.
    pub fn is_host_system_contract(&self, address: Address) -> bool {
        self.host.is_system_contract(address)
    }

    /// Get the host chain ID.
    pub const fn host_chain_id(&self) -> u64 {
        self.host.chain_id()
    }

    /// Get the height at which the Zenith contract was deployed on the host
    /// chain.
    pub const fn host_deploy_height(&self) -> u64 {
        self.host.deploy_height()
    }

    /// Convert a rollup block number to a host block number.
    pub const fn rollup_block_to_host_block_num(&self, rollup_block_num: u64) -> u64 {
        rollup_block_num + self.host_deploy_height()
    }

    /// Convert a host block number to a rollup block number. Returns `None` if
    /// the host block number is less than the zenith deploy height.
    pub const fn host_block_to_rollup_block_num(&self, host_block_num: u64) -> Option<u64> {
        host_block_num.checked_sub(self.host_deploy_height())
    }

    /// Pair the RU height with the host height.
    pub const fn pair_ru(&self, ru_height: u64) -> PairedHeights {
        PairedHeights { host: self.rollup_block_to_host_block_num(ru_height), rollup: ru_height }
    }

    /// Pair the host height with the RU height.
    pub fn pair_host(&self, host_height: u64) -> Option<PairedHeights> {
        let rollup_height = self.host_block_to_rollup_block_num(host_height)?;
        Some(PairedHeights { host: host_height, rollup: rollup_height })
    }

    /// Get the host zenith address.
    pub const fn host_zenith(&self) -> Address {
        self.host.zenith()
    }

    /// Get the host orders address.
    pub const fn host_orders(&self) -> Address {
        self.host.orders()
    }

    /// Get the host passage address.
    pub const fn host_passage(&self) -> Address {
        self.host.passage()
    }

    /// Get the host transactor address
    pub const fn host_transactor(&self) -> Address {
        self.host.transactor()
    }

    /// Get the ru passage address.
    pub const fn ru_orders(&self) -> Address {
        self.rollup.orders()
    }

    /// Get the ru passage address.
    pub const fn ru_passage(&self) -> Address {
        self.rollup.passage()
    }

    /// Get the base fee recipient address.
    pub const fn base_fee_recipient(&self) -> Address {
        self.rollup.base_fee_recipient()
    }

    /// Get the rollup chain ID.
    pub const fn ru_chain_id(&self) -> u64 {
        self.rollup.chain_id()
    }

    /// Get the rollup chain ID as a [`U256`].
    pub fn ru_chain_id_u256(&self) -> U256 {
        U256::from(self.ru_chain_id())
    }

    /// `True` if the address is a host token corresponding to a pre-deployed
    /// token on the rollup.
    pub const fn const_is_host_token(&self, address: Address) -> bool {
        self.host.tokens().const_is_token(address)
    }

    /// `True` if the address is a host token corresponding to a pre-deployed
    /// token on the rollup.
    pub fn is_host_token(&self, address: Address) -> bool {
        self.host.tokens().is_token(address)
    }

    /// `True` if the address is a pre-deployed token on the rollup.
    pub const fn const_is_rollup_token(&self, address: Address) -> bool {
        self.rollup.tokens().const_is_token(address)
    }
    /// `True` if the address is a pre-deployed token on the rollup.
    pub fn is_rollup_token(&self, address: Address) -> bool {
        self.rollup.tokens().is_token(address)
    }

    /// Get the rollup token address corresponding to a host address.
    ///
    /// Returns `None` if the address is not a pre-deployed token.
    pub fn rollup_token_from_host_address(&self, host_address: Address) -> Option<Address> {
        self.host.tokens().token_for(host_address).map(|t| self.rollup.tokens().address_for(t))
    }

    /// Get the minter address.
    pub const fn minter(&self) -> Address {
        self.rollup.minter()
    }
}
