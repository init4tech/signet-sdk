//! Ethereum hardfork configuration.

use alloy::{
    consensus::{constants::EMPTY_WITHDRAWALS, proofs::state_root_ref_unhashed, Header},
    eips::{eip1559::INITIAL_BASE_FEE, eip7685::EMPTY_REQUESTS_HASH},
    genesis::Genesis,
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
