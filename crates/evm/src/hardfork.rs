//! Ethereum hardfork configuration.

use alloy::{
    consensus::{constants::EMPTY_WITHDRAWALS, proofs::state_root_ref_unhashed, Header},
    eips::{eip1559::INITIAL_BASE_FEE, eip7685::EMPTY_REQUESTS_HASH},
    genesis::{ChainConfig, Genesis},
    primitives::B256,
};
use bitflags::bitflags;

bitflags! {
    #[doc="Ethereum HardForks."]
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
    #[repr(transparent)]
    pub struct EthereumHardfork: u64 {
        /// Frontier: <https://blog.ethereum.org/2015/03/03/ethereum-launch-process>.
        const Frontier = 1 << 0;
        /// Homestead: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/homestead.md>.
        const Homestead = 1 << 1;
        /// The DAO fork: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/dao-fork.md>.
        const Dao = 1 << 2;
        /// Tangerine: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/tangerine-whistle.md>.
        const Tangerine = 1 << 3;
        /// Spurious Dragon: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/spurious-dragon.md>.
        const SpuriousDragon = 1 << 4;
        /// Byzantium: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/byzantium.md>.
        const Byzantium = 1 << 5;
        /// Constantinople: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/constantinople.md>.
        const Constantinople = 1 << 6;
        /// Petersburg: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/petersburg.md>.
        const Petersburg = 1 << 7;
        /// Istanbul: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/istanbul.md>.
        const Istanbul = 1 << 8;
        /// Muir Glacier: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/muir-glacier.md>.
        const MuirGlacier = 1 << 9;
        /// Berlin: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/berlin.md>.
        const Berlin = 1 << 10;
        /// London: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/london.md>.
        const London = 1 << 11;
        /// Arrow Glacier: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/arrow-glacier.md>.
        const ArrowGlacier = 1 << 12;
        /// Gray Glacier: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/gray-glacier.md>.
        const GrayGlacier = 1 << 13;
        /// Paris: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/paris.md>.
        const Paris = 1 << 14;
        /// Shanghai: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/shanghai.md>.
        const Shanghai = 1 << 15;
        /// Cancun: <https://github.com/ethereum/execution-specs/blob/master/network-upgrades/mainnet-upgrades/cancun.md>
        const Cancun = 1 << 16;
        /// Prague.
        const Prague = 1 << 17;
        /// Osaka: <https://eips.ethereum.org/EIPS/eip-7607>
        const Osaka = 1 << 18;
        // BPOs: <https://eips.ethereum.org/EIPS/eip-7892>
        /// BPO 1
        const Bpo1 = 1 << 19;
        /// BPO 2
        const Bpo2 = 1 << 20;
        /// BPO 3
        const Bpo3 = 1 << 21;
        /// BPO 4
        const Bpo4 = 1 << 22;
        /// BPO 5
        const Bpo5 = 1 << 23;
        /// Amsterdam: <https://eips.ethereum.org/EIPS/eip-7773>
        const Amsterdam = 1 << 24;
    }
}

impl EthereumHardfork {
    /// Returns the [`SpecId`] corresponding to the highest active hardfork.
    ///
    /// BPO flags are ignored as they have no [`SpecId`] equivalent.
    ///
    /// [`SpecId`]: trevm::revm::primitives::hardfork::SpecId
    pub const fn spec_id(&self) -> trevm::revm::primitives::hardfork::SpecId {
        use trevm::revm::primitives::hardfork::SpecId;
        if self.contains(Self::Amsterdam) {
            SpecId::AMSTERDAM
        } else if self.contains(Self::Osaka) {
            SpecId::OSAKA
        } else if self.contains(Self::Prague) {
            SpecId::PRAGUE
        } else if self.contains(Self::Cancun) {
            SpecId::CANCUN
        } else if self.contains(Self::Shanghai) {
            SpecId::SHANGHAI
        } else if self.contains(Self::Paris) {
            SpecId::MERGE
        } else if self.contains(Self::GrayGlacier) {
            SpecId::GRAY_GLACIER
        } else if self.contains(Self::ArrowGlacier) {
            SpecId::ARROW_GLACIER
        } else if self.contains(Self::London) {
            SpecId::LONDON
        } else if self.contains(Self::Berlin) {
            SpecId::BERLIN
        } else if self.contains(Self::MuirGlacier) {
            SpecId::MUIR_GLACIER
        } else if self.contains(Self::Istanbul) {
            SpecId::ISTANBUL
        } else if self.contains(Self::Petersburg) {
            SpecId::PETERSBURG
        } else if self.contains(Self::Constantinople) {
            SpecId::CONSTANTINOPLE
        } else if self.contains(Self::Byzantium) {
            SpecId::BYZANTIUM
        } else if self.contains(Self::SpuriousDragon) {
            SpecId::SPURIOUS_DRAGON
        } else if self.contains(Self::Tangerine) {
            SpecId::TANGERINE
        } else if self.contains(Self::Dao) {
            SpecId::DAO_FORK
        } else if self.contains(Self::Homestead) {
            SpecId::HOMESTEAD
        } else {
            SpecId::FRONTIER
        }
    }

    /// Returns all active hardforks at the given block number and timestamp,
    /// as determined by the given [`ChainConfig`].
    ///
    /// # Example
    ///
    /// ```
    /// use signet_evm::EthereumHardfork;
    /// use alloy::genesis::ChainConfig;
    ///
    /// let config = ChainConfig {
    ///     homestead_block: Some(0),
    ///     london_block: Some(100),
    ///     ..Default::default()
    /// };
    ///
    /// let forks = EthereumHardfork::active_hardforks(&config, 50, 0);
    /// assert!(forks.contains(EthereumHardfork::Homestead));
    /// assert!(!forks.contains(EthereumHardfork::London));
    /// ```
    pub fn active_hardforks(config: &ChainConfig, block: u64, timestamp: u64) -> Self {
        Self::Frontier
            | fork_active(config.homestead_block, block, Self::Homestead)
            | fork_active(
                config.dao_fork_block.filter(|_| config.dao_fork_support),
                block,
                Self::Dao,
            )
            | fork_active(config.eip150_block, block, Self::Tangerine)
            | fork_active(config.eip158_block, block, Self::SpuriousDragon)
            | fork_active(config.byzantium_block, block, Self::Byzantium)
            | fork_active(config.constantinople_block, block, Self::Constantinople)
            | fork_active(config.petersburg_block, block, Self::Petersburg)
            | fork_active(config.istanbul_block, block, Self::Istanbul)
            | fork_active(config.muir_glacier_block, block, Self::MuirGlacier)
            | fork_active(config.berlin_block, block, Self::Berlin)
            | fork_active(config.london_block, block, Self::London)
            | fork_active(config.arrow_glacier_block, block, Self::ArrowGlacier)
            | fork_active(config.gray_glacier_block, block, Self::GrayGlacier)
            | if config.terminal_total_difficulty_passed { Self::Paris } else { Self::empty() }
            | fork_active(config.shanghai_time, timestamp, Self::Shanghai)
            | fork_active(config.cancun_time, timestamp, Self::Cancun)
            | fork_active(config.prague_time, timestamp, Self::Prague)
            | fork_active(config.osaka_time, timestamp, Self::Osaka)
            | fork_active(config.bpo1_time, timestamp, Self::Bpo1)
            | fork_active(config.bpo2_time, timestamp, Self::Bpo2)
            | fork_active(config.bpo3_time, timestamp, Self::Bpo3)
            | fork_active(config.bpo4_time, timestamp, Self::Bpo4)
            | fork_active(config.bpo5_time, timestamp, Self::Bpo5)
    }

    /// Returns all active hardforks at the given [`Header`]'s block number
    /// and timestamp, as determined by the given [`ChainConfig`].
    pub fn active_hardforks_at_header(config: &ChainConfig, header: &Header) -> Self {
        Self::active_hardforks(config, header.number, header.timestamp)
    }

    /// Returns the single latest active hardfork at the given block number
    /// and timestamp, as determined by the given [`ChainConfig`].
    ///
    /// # Example
    ///
    /// ```
    /// use signet_evm::EthereumHardfork;
    /// use alloy::genesis::ChainConfig;
    ///
    /// let config = ChainConfig {
    ///     homestead_block: Some(0),
    ///     london_block: Some(0),
    ///     ..Default::default()
    /// };
    ///
    /// let latest = EthereumHardfork::latest_hardfork(&config, 100, 0);
    /// assert_eq!(latest, EthereumHardfork::London);
    /// ```
    pub fn latest_hardfork(config: &ChainConfig, block: u64, timestamp: u64) -> Self {
        let active = Self::active_hardforks(config, block, timestamp);
        // Frontier is always active, so bits() is always >= 1.
        Self::from_bits_retain(1 << active.bits().ilog2())
    }
}

/// Returns the given `fork` if the activation point has been reached,
/// or an empty set otherwise.
const fn fork_active(
    activation: Option<u64>,
    current: u64,
    fork: EthereumHardfork,
) -> EthereumHardfork {
    match activation {
        Some(a) if current >= a => fork,
        _ => EthereumHardfork::empty(),
    }
}

/// Helper method building a [`Header`] given [`Genesis`] and [`EthereumHardfork`].
pub fn genesis_header(genesis: &Genesis, hardforks: &EthereumHardfork) -> Header {
    // If London is activated at genesis, we set the initial base fee as per EIP-1559.
    let base_fee_per_gas = hardforks
        .contains(EthereumHardfork::London)
        .then(|| genesis.base_fee_per_gas.map(|fee| fee as u64).unwrap_or(INITIAL_BASE_FEE));

    // If shanghai is activated, initialize the header with an empty withdrawals hash, and
    // empty withdrawals list.
    let withdrawals_root =
        hardforks.contains(EthereumHardfork::Shanghai).then_some(EMPTY_WITHDRAWALS);

    // If Cancun is activated at genesis, we set:
    // * parent beacon block root to 0x0
    // * blob gas used to provided genesis or 0x0
    // * excess blob gas to provided genesis or 0x0
    let (parent_beacon_block_root, blob_gas_used, excess_blob_gas) =
        if hardforks.contains(EthereumHardfork::Cancun) {
            let blob_gas_used = genesis.blob_gas_used.unwrap_or(0);
            let excess_blob_gas = genesis.excess_blob_gas.unwrap_or(0);
            (Some(B256::ZERO), Some(blob_gas_used), Some(excess_blob_gas))
        } else {
            (None, None, None)
        };

    // If Prague is activated at genesis we set requests root to an empty trie root.
    let requests_hash = hardforks.contains(EthereumHardfork::Prague).then_some(EMPTY_REQUESTS_HASH);

    Header {
        number: genesis.number.unwrap_or_default(),
        parent_hash: genesis.parent_hash.unwrap_or_default(),
        gas_limit: genesis.gas_limit,
        difficulty: genesis.difficulty,
        nonce: genesis.nonce.into(),
        extra_data: genesis.extra_data.clone(),
        state_root: state_root_ref_unhashed(&genesis.alloc),
        timestamp: genesis.timestamp,
        mix_hash: genesis.mix_hash,
        beneficiary: genesis.coinbase,
        base_fee_per_gas,
        withdrawals_root,
        parent_beacon_block_root,
        blob_gas_used,
        excess_blob_gas,
        requests_hash,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::consensus::Header;

    #[test]
    fn frontier_always_active() {
        let config = ChainConfig::default();
        let forks = EthereumHardfork::active_hardforks(&config, 0, 0);
        assert_eq!(forks, EthereumHardfork::Frontier);
    }

    #[test]
    fn block_forks_activate_at_threshold() {
        let config = ChainConfig {
            homestead_block: Some(10),
            byzantium_block: Some(20),
            ..Default::default()
        };

        let forks = EthereumHardfork::active_hardforks(&config, 9, 0);
        assert!(!forks.contains(EthereumHardfork::Homestead));

        let forks = EthereumHardfork::active_hardforks(&config, 10, 0);
        assert!(forks.contains(EthereumHardfork::Homestead));
        assert!(!forks.contains(EthereumHardfork::Byzantium));

        let forks = EthereumHardfork::active_hardforks(&config, 20, 0);
        assert!(forks.contains(EthereumHardfork::Homestead));
        assert!(forks.contains(EthereumHardfork::Byzantium));
    }

    #[test]
    fn timestamp_forks_activate_at_threshold() {
        let config = ChainConfig {
            shanghai_time: Some(1000),
            cancun_time: Some(2000),
            ..Default::default()
        };

        let forks = EthereumHardfork::active_hardforks(&config, 0, 999);
        assert!(!forks.contains(EthereumHardfork::Shanghai));

        let forks = EthereumHardfork::active_hardforks(&config, 0, 1000);
        assert!(forks.contains(EthereumHardfork::Shanghai));
        assert!(!forks.contains(EthereumHardfork::Cancun));

        let forks = EthereumHardfork::active_hardforks(&config, 0, 2000);
        assert!(forks.contains(EthereumHardfork::Shanghai));
        assert!(forks.contains(EthereumHardfork::Cancun));
    }

    #[test]
    fn dao_requires_support_flag() {
        let config =
            ChainConfig { dao_fork_block: Some(10), dao_fork_support: false, ..Default::default() };
        let forks = EthereumHardfork::active_hardforks(&config, 10, 0);
        assert!(!forks.contains(EthereumHardfork::Dao));

        let config =
            ChainConfig { dao_fork_block: Some(10), dao_fork_support: true, ..Default::default() };
        let forks = EthereumHardfork::active_hardforks(&config, 10, 0);
        assert!(forks.contains(EthereumHardfork::Dao));
    }

    #[test]
    fn paris_uses_ttd_passed() {
        let config = ChainConfig { terminal_total_difficulty_passed: true, ..Default::default() };
        let forks = EthereumHardfork::active_hardforks(&config, 0, 0);
        assert!(forks.contains(EthereumHardfork::Paris));

        let config = ChainConfig::default();
        let forks = EthereumHardfork::active_hardforks(&config, 0, 0);
        assert!(!forks.contains(EthereumHardfork::Paris));
    }

    #[test]
    fn latest_returns_highest_active() {
        let config = ChainConfig {
            homestead_block: Some(0),
            eip150_block: Some(0),
            eip158_block: Some(0),
            byzantium_block: Some(0),
            constantinople_block: Some(0),
            petersburg_block: Some(0),
            istanbul_block: Some(0),
            berlin_block: Some(0),
            london_block: Some(0),
            terminal_total_difficulty_passed: true,
            shanghai_time: Some(0),
            cancun_time: Some(0),
            ..Default::default()
        };

        let latest = EthereumHardfork::latest_hardfork(&config, 100, 100);
        assert_eq!(latest, EthereumHardfork::Cancun);
    }

    #[test]
    fn latest_frontier_when_no_forks() {
        let config = ChainConfig::default();
        let latest = EthereumHardfork::latest_hardfork(&config, 0, 0);
        assert_eq!(latest, EthereumHardfork::Frontier);
    }

    #[test]
    fn active_hardforks_at_header_delegates() {
        let config = ChainConfig {
            homestead_block: Some(10),
            shanghai_time: Some(1000),
            ..Default::default()
        };
        let header = Header { number: 10, timestamp: 1000, ..Default::default() };
        let forks = EthereumHardfork::active_hardforks_at_header(&config, &header);
        assert!(forks.contains(EthereumHardfork::Homestead));
        assert!(forks.contains(EthereumHardfork::Shanghai));
    }

    #[test]
    fn bpo_forks_activate() {
        let config =
            ChainConfig { bpo1_time: Some(100), bpo3_time: Some(300), ..Default::default() };

        let forks = EthereumHardfork::active_hardforks(&config, 0, 200);
        assert!(forks.contains(EthereumHardfork::Bpo1));
        assert!(!forks.contains(EthereumHardfork::Bpo3));

        let forks = EthereumHardfork::active_hardforks(&config, 0, 300);
        assert!(forks.contains(EthereumHardfork::Bpo1));
        assert!(forks.contains(EthereumHardfork::Bpo3));
    }
}
