//! Constants for the Mainnet.

use crate::{
    HostConstants, HostTokens, HostUsdRecord, RollupConstants, RollupTokens, SignetConstants,
    SignetEnvironmentConstants, SignetSystemConstants, UsdRecords,
};
use alloy::primitives::{address, Address};
use std::borrow::Cow;

/// Name for the host chain.
pub const HOST_NAME: &str = "Mainnet";
/// Chain ID for the Mainnet host chain.
pub const HOST_CHAIN_ID: u64 = 1;
/// Deployment height of the rollup on Mainnet host chain.
pub const DEPLOY_HEIGHT: u64 = 0;
/// `Zenith` contract address for the Mainnet host chain.
pub const HOST_ZENITH: Address = address!("0xBCe84D45d7be8859bcBd838d4a7b3448B55E6869");
/// `Orders` contract address for the Mainnet host chain.
pub const HOST_ORDERS: Address = address!("0x96f44ddc3Bc8892371305531F1a6d8ca2331fE6C");
/// `Passage` contract address for the Mainnet host chain.
pub const HOST_PASSAGE: Address = address!("0x02a64d6e2c30d2B07ddBD177b24D9D0f6439CcbD");
/// `Transactor` contract address for the Mainnet host chain.
pub const HOST_TRANSACTOR: Address = address!("0xC4388A6f4917B8D392B19b43F9c46FEC1B890f45");

/// USDC token for the Mainnet host chain (empty placeholder).
pub const HOST_USDC: Address = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
/// USDT token for the Mainnet host chain (empty placeholder).
pub const HOST_USDT: Address = address!("0xdAC17F958D2ee523a2206206994597C13D831ec7");
/// WBTC token for the Mainnet host chain (empty placeholder).
pub const HOST_WBTC: Address = address!("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599");
/// WETH token for the Mainnet host chain (empty placeholder).
pub const HOST_WETH: Address = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

/// USDC token record for the Mainnet host chain (placeholder name/decimals).
pub const HOST_USDC_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDC, Cow::Borrowed("USDC"), 6);
/// USDT token record for the Mainnet host chain (placeholder name/decimals).
pub const HOST_USDT_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDT, Cow::Borrowed("USDT"), 6);
/// Host USD records for the Mainnet host chain (empty list by default).
pub const HOST_USD_RECORDS: UsdRecords = UsdRecords::new();
/// Host system tokens for Mainnet (placeholders).
pub const HOST_TOKENS: HostTokens = HostTokens::new(HOST_USD_RECORDS, HOST_WBTC, HOST_WETH);

/// Start timestamp for the Mainnet host chain slot calculator (Ethereum Mainnet merge timestamp).
pub const HOST_START_TIMESTAMP: u64 = 1663224179;
/// Slot offset for the Mainnet host chain slot calculator.
pub const HOST_SLOT_OFFSET: u64 = 4700013;
/// Slot duration for the Mainnet host chain slot calculator.
pub const HOST_SLOT_DURATION: u64 = 12;

/// Host system constants for Mainnet.
pub const HOST: HostConstants = crate::HostConstants::new(
    HOST_CHAIN_ID,
    DEPLOY_HEIGHT,
    HOST_ZENITH,
    HOST_ORDERS,
    HOST_PASSAGE,
    HOST_TRANSACTOR,
    HOST_TOKENS,
    HOST_START_TIMESTAMP,
    HOST_SLOT_OFFSET,
    HOST_SLOT_DURATION,
);

/// Name for the network.
pub const RU_NAME: &str = "Signet";
/// Chain ID for the Mainnet RU chain.
pub const RU_CHAIN_ID: u64 = 519;

/// WETH token for the Mainnet RU chain (placeholder).
pub const RU_WETH: Address = address!("0x0000000000000000007369676e65742d77657468");
/// WBTC token for the Mainnet RU chain (placeholder).
pub const RU_WBTC: Address = address!("0x0000000000000000007369676e65742d77627463");
/// `Orders` contract address for the Mainnet RU chain (placeholder).
pub const RU_ORDERS: Address = address!("0x000000000000007369676e65742d6f7264657273");
/// `Passage` contract address for the Mainnet RU chain (placeholder).
pub const RU_PASSAGE: Address = address!("0x0000000000007369676e65742d70617373616765");
/// The WETH9-based wrapped native USD token contract (placeholder).
pub const WRAPPED: Address = address!("0x0000000000000000007369676e65742D77757364");
/// RU pre-approved system tokens for Mainnet (placeholders).
pub const RU_TOKENS: RollupTokens = RollupTokens::new(RU_WBTC, RU_WETH);

/// Base fee recipient address for the Mainnet RU chain (placeholder).
pub const BASE_FEE_RECIPIENT: Address = address!("0x86Fa9c9fb93C5F6022276db84bf2A05b5a72283E");

/// RU system constants for Mainnet.
pub const ROLLUP: RollupConstants =
    crate::RollupConstants::new(RU_CHAIN_ID, RU_ORDERS, RU_PASSAGE, BASE_FEE_RECIPIENT, RU_TOKENS);

/// Signet system constants for Mainnet.
pub const MAINNET_SYS: SignetSystemConstants = crate::SignetSystemConstants::new(HOST, ROLLUP);

/// Signet environment constants for Mainnet.
pub const MAINNET_ENV: SignetEnvironmentConstants = SignetEnvironmentConstants::new(
    Cow::Borrowed(HOST_NAME),
    Cow::Borrowed(RU_NAME),
    Cow::Borrowed(TX_CACHE_URL),
);

/// Signet constants for Mainnet.
pub const MAINNET: SignetConstants = SignetConstants::new(MAINNET_SYS, MAINNET_ENV);

/// The URL of the Transaction Cache endpoint (empty for mainnet placeholder).
pub const TX_CACHE_URL: &str = "https://transactions.signet.sh";
