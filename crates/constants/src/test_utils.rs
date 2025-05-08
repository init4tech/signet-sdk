use crate::{HostConstants, PredeployTokens, RollupConstants, SignetSystemConstants};
use alloy::primitives::Address;

/// Default reward address used in tests when no other is specified.
pub const DEFAULT_REWARD_ADDRESS: Address = Address::repeat_byte(0x81);

/// Test chain id for the host chain.
pub const TEST_HOST_CHAIN_ID: u64 = 1;

/// Test chain id for the RU chain.
pub const TEST_RU_CHAIN_ID: u64 = 15;

/// Test address for the host zenith.
pub const HOST_ZENITH_ADDRESS: Address = Address::repeat_byte(0x82);

/// Test address for the host orders.
pub const HOST_ORDERS_ADDRESS: Address = Address::repeat_byte(0x83);

/// Test address for the host passage.
pub const HOST_PASSAGE_ADDRESS: Address = Address::repeat_byte(0x84);

/// Test address for the host transactor
pub const HOST_TRANSACTOR_ADDRESS: Address = Address::repeat_byte(0x85);

/// Test address for the RU zenith.
pub const RU_ORDERS_ADDRESS: Address = Address::repeat_byte(0x86);

/// Test address for the RU passage.
pub const RU_PASSAGE_ADDRESS: Address = Address::repeat_byte(0x87);

/// Test address for the base fee recipient.
pub const BASE_FEE_RECIPIENT: Address = Address::repeat_byte(0x88);

/// Test address for predeployed USDC
pub const USDC: Address = Address::repeat_byte(0x89);

/// Test address for predeployed USDT
pub const USDT: Address = Address::repeat_byte(0x8a);

/// Test address for predeployed WBTC
pub const WBTC: Address = Address::repeat_byte(0x8b);

/// Predeployed tokens for testing
pub const TEST_PREDEPLOYS: PredeployTokens = PredeployTokens::new(USDC, USDT, WBTC);

/// Host config
pub const TEST_HOST_CONFIG: HostConstants = HostConstants::new(
    TEST_HOST_CHAIN_ID,
    0,
    HOST_ZENITH_ADDRESS,
    HOST_ORDERS_ADDRESS,
    HOST_PASSAGE_ADDRESS,
    HOST_TRANSACTOR_ADDRESS,
    TEST_PREDEPLOYS,
);

/// Rollup config
pub const TEST_ROLLUP_CONFIG: RollupConstants = RollupConstants::new(
    TEST_RU_CHAIN_ID,
    RU_ORDERS_ADDRESS,
    RU_PASSAGE_ADDRESS,
    BASE_FEE_RECIPIENT,
    TEST_PREDEPLOYS,
);

/// Test constants for unit tests.
pub const TEST_CONSTANTS: SignetSystemConstants =
    SignetSystemConstants::new(TEST_HOST_CONFIG, TEST_ROLLUP_CONFIG);
