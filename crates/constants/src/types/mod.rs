mod error;
pub use error::ConfigError;

mod height;
pub use height::PairedHeights;

mod host;
pub use host::HostConstants;

mod rollup;
pub use rollup::{RollupConstants, MINTER_ADDRESS};

mod chains;
pub use chains::{KnownChains, ParseChainError};

mod tokens;
pub use tokens::{PermissionedToken, PredeployTokens};

mod environment;
pub use environment::SignetEnvironmentConstants;

use alloy::{
    genesis::Genesis,
    primitives::{Address, U256},
};
use std::str::FromStr;

/// Signet constants.
///
/// This struct contains the system constants for a Signet chain, including
/// information about the host and rollup state. These constants are used to
/// determine the behavior of the chain, such as which contracts the Signet
/// node should listen to, and the addresses of system-priveleged tokens.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct SignetSystemConstants {
    /// Host constants.
    host: HostConstants,
    /// Rollup constants.
    rollup: RollupConstants,
}

impl SignetSystemConstants {
    /// Create a new set of constants.
    pub const fn new(host: HostConstants, rollup: RollupConstants) -> Self {
        Self { host, rollup }
    }

    /// Get the hard-coded pecorino system constants.
    pub const fn pecorino() -> Self {
        crate::chains::pecorino::PECORINO_SYS
    }

    /// Get the hard-coded local test constants.
    #[cfg(any(test, feature = "test-utils"))]
    pub const fn test() -> Self {
        crate::chains::test_utils::TEST_SYS
    }

    /// Load the constants from a [`Genesis`].
    pub fn try_from_genesis(genesis: &Genesis) -> Result<Self, ConfigError> {
        let k = "signetConstants";
        let constants =
            genesis.config.extra_fields.get(k).ok_or_else(|| ConfigError::missing(k))?;
        serde_json::from_value(constants.clone()).map_err(Into::into)
    }

    /// Get the host addresses.
    pub const fn host(&self) -> HostConstants {
        self.host
    }

    /// Get the rollup addresses.
    pub const fn rollup(&self) -> RollupConstants {
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

    /// True if the address is a host USD that can be used to mint rollup
    /// native asset.
    pub const fn is_host_usd(&self, address: Address) -> bool {
        self.host.is_usd(address)
    }

    /// Get the Order contract address for the given chain id.
    pub const fn orders_for(&self, chain_id: u64) -> Option<Address> {
        if chain_id == self.host_chain_id() {
            Some(self.host_orders())
        } else if chain_id == self.ru_chain_id() {
            Some(self.ru_orders())
        } else {
            None
        }
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

impl FromStr for SignetSystemConstants {
    type Err = ParseChainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chain: KnownChains = s.parse()?;
        match chain {
            KnownChains::Pecorino => Ok(Self::pecorino()),
            #[cfg(any(test, feature = "test-utils"))]
            KnownChains::Test => Ok(Self::test()),
        }
    }
}

/// All constants pertaining to the Signet system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignetConstants {
    /// System constants for a Signet chain.
    system: SignetSystemConstants,
    /// Environment constants for a Signet chain.
    environment: SignetEnvironmentConstants,
}

impl SignetConstants {
    /// Create a new set of Signet constants.
    pub const fn new(
        system: SignetSystemConstants,
        environment: SignetEnvironmentConstants,
    ) -> Self {
        Self { system, environment }
    }

    /// Get the hard-coded pecorino rollup constants.
    pub const fn pecorino() -> Self {
        crate::chains::pecorino::PECORINO
    }

    /// Get the hard-coded local test rollup constants.
    #[cfg(any(test, feature = "test-utils"))]
    pub const fn test() -> Self {
        crate::chains::test_utils::TEST
    }

    /// Get the system constants.
    pub const fn system(&self) -> SignetSystemConstants {
        self.system
    }

    /// Get the host constants.
    pub const fn host(&self) -> HostConstants {
        self.system.host
    }

    /// Get the rollup constants.
    pub const fn rollup(&self) -> RollupConstants {
        self.system.rollup
    }

    /// Get the environment constants.
    pub const fn environment(&self) -> &SignetEnvironmentConstants {
        &self.environment
    }
}

impl FromStr for SignetConstants {
    type Err = ParseChainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chain: KnownChains = s.parse()?;
        match chain {
            KnownChains::Pecorino => Ok(Self::pecorino()),
            #[cfg(any(test, feature = "test-utils"))]
            KnownChains::Test => Ok(Self::test()),
        }
    }
}
