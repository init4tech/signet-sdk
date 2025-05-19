//! Constants for local testnet chains.

use crate::{
    HostConstants, PredeployTokens, RollupConstants, SignetConstants, SignetEnvironmentConstants,
    SignetSystemConstants,
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
pub const DEPLOY_HEIGHT: u64 = 0;
/// Test address for the host zenith.
pub const HOST_ZENITH: Address = Address::repeat_byte(0x11);
/// Test address for the host orders.
pub const HOST_ORDERS: Address = Address::repeat_byte(0x22);
/// Test address for the host passage.
pub const HOST_PASSAGE: Address = Address::repeat_byte(0x33);
/// Test address for the host transactor
pub const HOST_TRANSACTOR: Address = Address::repeat_byte(0x44);

/// Test address for predeployed USDC
pub const HOST_USDC: Address = Address::repeat_byte(0x55);
/// Test address for predeployed USDT
pub const HOST_USDT: Address = Address::repeat_byte(0x66);
/// Test address for predeployed WBTC
pub const HOST_WBTC: Address = Address::repeat_byte(0x77);

/// Test address for predeployed USDC
pub const RU_USDC: Address = address!("0x0B8BC5e60EE10957E0d1A0d95598fA63E65605e2");
/// Test address for predeployed USDT
pub const RU_USDT: Address = address!("0xF34326d3521F1b07d1aa63729cB14A372f8A737C");
/// Test address for predeployed WBTC
pub const RU_WBTC: Address = address!("0xE3d7066115f7d6b65F88Dff86288dB4756a7D733");

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
pub const HOST_TOKENS: PredeployTokens = PredeployTokens::new(HOST_USDC, HOST_USDT, HOST_WBTC);

/// RU system tokens.
pub const RU_TOKENS: PredeployTokens = PredeployTokens::new(RU_USDC, RU_USDT, RU_WBTC);

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
