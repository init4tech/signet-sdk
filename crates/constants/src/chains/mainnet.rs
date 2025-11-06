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
pub const DEPLOY_HEIGHT: u64 = 23734244;
/// `Zenith` contract address for the Mainnet host chain.
pub const HOST_ZENITH: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// `Orders` contract address for the Mainnet host chain.
pub const HOST_ORDERS: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// `Passage` contract address for the Mainnet host chain.
pub const HOST_PASSAGE: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// `Transactor` contract address for the Mainnet host chain.
pub const HOST_TRANSACTOR: Address = address!("0x0000000000000000000000000000000000000000"); // TODO

/// USDC token for the Mainnet host chain (empty placeholder).
pub const HOST_USDC: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// USDT token for the Mainnet host chain (empty placeholder).
pub const HOST_USDT: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// WBTC token for the Mainnet host chain (empty placeholder).
pub const HOST_WBTC: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// WETH token for the Mainnet host chain (empty placeholder).
pub const HOST_WETH: Address = address!("0x0000000000000000000000000000000000000000"); // TODO

/// USDC token record for the Mainnet host chain (placeholder name/decimals).
pub const HOST_USDC_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDC, Cow::Borrowed("USDC"), 6);
/// USDT token record for the Mainnet host chain (placeholder name/decimals).
pub const HOST_USDT_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDT, Cow::Borrowed("USDT"), 6);
/// Host USD records for the Mainnet host chain (empty list by default).
pub const HOST_USD_RECORDS: UsdRecords = {
    let records = UsdRecords::new();
    records
};
/// Host system tokens for Mainnet (placeholders).
pub const HOST_TOKENS: HostTokens = HostTokens::new(HOST_USD_RECORDS, HOST_WBTC, HOST_WETH);

/// Host system constants for Mainnet.
pub const HOST: HostConstants = crate::HostConstants::new(
    HOST_CHAIN_ID,
    DEPLOY_HEIGHT,
    HOST_ZENITH,
    HOST_ORDERS,
    HOST_PASSAGE,
    HOST_TRANSACTOR,
    HOST_TOKENS,
);

/// Name for the network.
pub const RU_NAME: &str = "Signet";
/// Chain ID for the Mainnet RU chain.
pub const RU_CHAIN_ID: u64 = 519;

/// WETH token for the Mainnet RU chain (placeholder).
pub const RU_WETH: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// WBTC token for the Mainnet RU chain (placeholder).
pub const RU_WBTC: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// `Orders` contract address for the Mainnet RU chain (placeholder).
pub const RU_ORDERS: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// `Passage` contract address for the Mainnet RU chain (placeholder).
pub const RU_PASSAGE: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// The WETH9-based wrapped native USD token contract (placeholder).
pub const WRAPPED: Address = address!("0x0000000000000000000000000000000000000000"); // TODO
/// RU pre-approved system tokens for Mainnet (placeholders).
pub const RU_TOKENS: RollupTokens = RollupTokens::new(RU_WBTC, RU_WETH);

/// Base fee recipient address for the Mainnet RU chain (placeholder).
pub const BASE_FEE_RECIPIENT: Address = address!("0x0000000000000000000000000000000000000000"); // TODO

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
pub const TX_CACHE_URL: &str = "TODO";
