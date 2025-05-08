//! Constants for local testnet chains.

use crate::{HostConstants, PredeployTokens, RollupConstants, SignetSystemConstants};
use alloy::primitives::Address;

/// Default reward address used in tests when no other is specified.
pub const DEFAULT_REWARD_ADDRESS: Address = Address::repeat_byte(0x81);

/// Test chain id for the host chain.
pub const HOST_CHAIN_ID: u64 = 1;
/// Test deployment height.
pub const DEPLOY_HEIGHT: u64 = 0;
/// Test address for the host zenith.
pub const HOST_ZENITH: Address = Address::repeat_byte(0x82);
/// Test address for the host orders.
pub const HOST_ORDERS: Address = Address::repeat_byte(0x83);
/// Test address for the host passage.
pub const HOST_PASSAGE: Address = Address::repeat_byte(0x84);
/// Test address for the host transactor
pub const HOST_TRANSACTOR: Address = Address::repeat_byte(0x85);

/// Test address for predeployed USDC
pub const HOST_USDC: Address = Address::repeat_byte(0x89);
/// Test address for predeployed USDT
pub const HOST_USDT: Address = Address::repeat_byte(0x8a);
/// Test address for predeployed WBTC
pub const HOST_WBTC: Address = Address::repeat_byte(0x8b);

/// Test address for predeployed USDC
pub const RU_USDC: Address = HOST_USDC;
/// Test address for predeployed USDT
pub const RU_USDT: Address = HOST_USDT;
/// Test address for predeployed WBTC
pub const RU_WBTC: Address = HOST_WBTC;

/// Test chain id for the RU chain.
pub const RU_CHAIN_ID: u64 = 15;
/// Test address for the RU zenith.
pub const RU_ORDERS: Address = Address::repeat_byte(0x86);
/// Test address for the RU passage.
pub const RU_PASSAGE: Address = Address::repeat_byte(0x87);
/// Test address for the base fee recipient.
pub const BASE_FEE_RECIPIENT: Address = Address::repeat_byte(0x88);

/// Host system tokens.
pub const HOST_TOKENS: PredeployTokens = PredeployTokens::new(HOST_USDC, HOST_USDT, HOST_WBTC);

/// RU system tokens.
pub const RU_TOKENS: PredeployTokens = PredeployTokens::new(RU_USDC, RU_USDT, RU_WBTC);

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

/// Test constants for unit tests.
pub const TEST_CONSTANTS: SignetSystemConstants = SignetSystemConstants::new(HOST, ROLLUP);
