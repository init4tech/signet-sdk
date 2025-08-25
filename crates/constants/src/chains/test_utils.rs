//! Constants for local testnet chains.

use crate::{
    types::{
        HostConstants, HostTokens, HostUsdRecord, RollupConstants, RollupTokens, SignetConstants,
        SignetEnvironmentConstants, SignetSystemConstants,
    },
    UsdRecords,
};
use alloy::primitives::{address, Address};
use std::borrow::Cow;

/// Default reward address used in tests when no other is specified.
pub const DEFAULT_REWARD_ADDRESS: Address = Address::repeat_byte(0x81);

/// Name for the host chain.
pub const HOST_NAME: &str = "Test Host";
/// Test chain id for the host chain.
pub const HOST_CHAIN_ID: u64 = 1;
/// Test deployment height.
pub const DEPLOY_HEIGHT: u64 = 100;
/// Test address for the host zenith.
pub const HOST_ZENITH: Address = Address::repeat_byte(0x11);
/// Test address for the host orders.
pub const HOST_ORDERS: Address = Address::repeat_byte(0x22);
/// Test address for the host passage.
pub const HOST_PASSAGE: Address = Address::repeat_byte(0x33);
/// Test address for the host transactor
pub const HOST_TRANSACTOR: Address = Address::repeat_byte(0x44);

/// Test address for host USDC
pub const HOST_USDC: Address = Address::repeat_byte(0x55);
/// Test address for host USDT
pub const HOST_USDT: Address = Address::repeat_byte(0x66);
/// Test address for host WBTC
pub const HOST_WBTC: Address = Address::repeat_byte(0x77);
/// Test address for host WETH
pub const HOST_WETH: Address = Address::repeat_byte(0x88);

/// Test record for host USDC.
pub const USDC_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDC, Cow::Borrowed("USDC"), 6);

/// Test record for host USDT.
pub const USDT_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDT, Cow::Borrowed("USDT"), 12);

/// Test records for host USD tokens.
pub const HOST_USDS: UsdRecords = {
    let mut records = UsdRecords::new();
    records.push(USDC_RECORD);
    records.push(USDT_RECORD);
    records
};

/// Test address for predeployed WBTC
pub const RU_WBTC: Address = Address::repeat_byte(0x99);
/// Test address for predeployed WETH
pub const RU_WETH: Address = Address::repeat_byte(0xaa);

/// Name for the network.
pub const RU_NAME: &str = "Test Rollup";
/// Test chain id for the RU chain.
pub const RU_CHAIN_ID: u64 = 15;
/// Test address for the RU zenith.
pub const RU_ORDERS: Address = address!("0xC2D3Dac6B115564B10329697195656459BFb2c74");
/// Test address for the RU passage.
pub const RU_PASSAGE: Address = address!("0xB043BdD3d91376A76078c361bb82496Fdb809aE2");
/// Test address for the base fee recipient.
pub const BASE_FEE_RECIPIENT: Address = Address::repeat_byte(0xab);

/// Host system tokens.
pub const HOST_TOKENS: HostTokens = HostTokens::new(HOST_USDS, HOST_WBTC, HOST_WETH);

/// RU system tokens.
pub const RU_TOKENS: RollupTokens = RollupTokens::new(RU_WBTC, RU_WETH);

/// The URL of the Transaction Cache endpoint.
pub const TX_CACHE_URL: &str = "localhost:8080/txcache";

/// Host config
pub const HOST: HostConstants = HostConstants::new(
    HOST_CHAIN_ID,
    0,
    HOST_ZENITH,
    HOST_ORDERS,
    HOST_PASSAGE,
    HOST_TRANSACTOR,
    HOST_TOKENS,
);

/// Rollup config
pub const ROLLUP: RollupConstants =
    RollupConstants::new(RU_CHAIN_ID, RU_ORDERS, RU_PASSAGE, BASE_FEE_RECIPIENT, RU_TOKENS);

/// System constants for unit tests.
pub const TEST_SYS: SignetSystemConstants = SignetSystemConstants::new(HOST, ROLLUP);

/// Environment constants for unit tests.
pub const TEST_ENV: SignetEnvironmentConstants = SignetEnvironmentConstants::new(
    Cow::Borrowed(HOST_NAME),
    Cow::Borrowed(RU_NAME),
    Cow::Borrowed(TX_CACHE_URL),
);

/// Signet constants for Pecorino.
pub const TEST: SignetConstants = SignetConstants::new(TEST_SYS, TEST_ENV);
