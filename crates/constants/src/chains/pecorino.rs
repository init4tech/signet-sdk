//! Constants for the Pecorino testnet.

use crate::{
    HostConstants, HostTokens, HostUsdRecord, RollupConstants, RollupTokens, SignetConstants,
    SignetEnvironmentConstants, SignetSystemConstants, UsdRecords,
};
use alloy::primitives::{address, Address};
use std::borrow::Cow;

/// Name for the host chain.
pub const HOST_NAME: &str = "Pecorino Host";
/// Chain ID for the Pecorino testnet host chain.
pub const HOST_CHAIN_ID: u64 = 3151908;
/// Deployment height for the Pecorino testnet host chain.
pub const DEPLOY_HEIGHT: u64 = 366;
/// `Zenith` contract address for the Pecorino testnet host chain.
pub const HOST_ZENITH: Address = address!("0xf17E98baF73F7C78a42D73DF4064de5B7A20EcA6");
/// `Orders` contract address for the Pecorino testnet host chain.
pub const HOST_ORDERS: Address = address!("0x0A4f505364De0Aa46c66b15aBae44eBa12ab0380");
/// `Passage` contract address for the Pecorino testnet host chain.
pub const HOST_PASSAGE: Address = address!("0x12585352AA1057443D6163B539EfD4487f023182");
/// `Transactor` contract address for the Pecorino testnet host chain.
pub const HOST_TRANSACTOR: Address = address!("0x3903279B59D3F5194053dA8d1f0C7081C8892Ce4");

/// USDC token for the Pecorino testnet host chain.
pub const HOST_USDC: Address = address!("0x65fb255585458de1f9a246b476aa8d5c5516f6fd");
/// USDT token for the Pecorino testnet host chain.
pub const HOST_USDT: Address = address!("0xb9df1b911b6cf6935b2a918ba03df2372e94e267");
/// WBTC token for the Pecorino testnet host chain.
pub const HOST_WBTC: Address = address!("0xfb29f7d7a4ce607d6038d44150315e5f69bea08a");
/// WETH token for the Pecorino testnet host chain.
pub const HOST_WETH: Address = address!("0xd03d085B78067A18155d3B29D64914df3D19A53C");

/// USDC token record for the Pecorino testnet host chain.
pub const HOST_USDC_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDC, Cow::Borrowed("USDC"), 6);
/// USDT token record for the Pecorino testnet host chain.
pub const HOST_USDT_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDT, Cow::Borrowed("USDT"), 6);
/// Host USD records for the Pecorino testnet host chain.
pub const HOST_USD_RECORDS: UsdRecords = {
    let mut records = UsdRecords::new();
    records.push(HOST_USDC_RECORD);
    records.push(HOST_USDT_RECORD);
    records
};
/// Host system tokens for Pecorino.
pub const HOST_TOKENS: HostTokens = HostTokens::new(HOST_USD_RECORDS, HOST_WBTC, HOST_WETH);

/// Host system constants for Pecorino.
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
pub const RU_NAME: &str = "Pecorino";
/// Chain ID for the Pecorino testnet RU chain.
pub const RU_CHAIN_ID: u64 = 14174;

/// WETH token for the Pecorino testnet RU chain.
pub const RU_WETH: Address = address!("0x0000000000000000007369676e65742d77657468");
/// WBTC token for the Pecorino testnet RU chain.
pub const RU_WBTC: Address = address!("0x0000000000000000007369676e65742D77627463");
/// `Orders` contract address for the Pecorino testnet RU chain.
pub const RU_ORDERS: Address = address!("0x000000000000007369676E65742D6f7264657273");
/// `Passage` contract address for the Pecorino testnet RU chain.
/// This is currently a dummy value and will be replaced with the actual Passage contract address in the future.
pub const RU_PASSAGE: Address = address!("0x0000000000007369676E65742D70617373616765");
/// The WETH9-based wrapped native USD token contract.
/// This is signet's native token in wrapped form.
pub const WRAPPED: Address = address!("0x0000000000000000007369676e65742D77757364");
/// RU pre-approved system tokens for Pecorino.
pub const RU_TOKENS: RollupTokens = RollupTokens::new(RU_WBTC, RU_WETH);

/// Base fee recipient address for the Pecorino testnet RU chain.
pub const BASE_FEE_RECIPIENT: Address = address!("0xe0eDA3701D44511ce419344A4CeD30B52c9Ba231");

/// RU system constants for Pecorino.
pub const ROLLUP: RollupConstants =
    crate::RollupConstants::new(RU_CHAIN_ID, RU_ORDERS, RU_PASSAGE, BASE_FEE_RECIPIENT, RU_TOKENS);

/// Signet system constants for Pecorino.
pub const PECORINO_SYS: SignetSystemConstants = crate::SignetSystemConstants::new(HOST, ROLLUP);

/// Signet environment constants for Pecorino.
pub const PECORINO_ENV: SignetEnvironmentConstants = SignetEnvironmentConstants::new(
    Cow::Borrowed(HOST_NAME),
    Cow::Borrowed(RU_NAME),
    Cow::Borrowed(TX_CACHE_URL),
);

/// Signet constants for Pecorino.
pub const PECORINO: SignetConstants = SignetConstants::new(PECORINO_SYS, PECORINO_ENV);

/// The URL of the Transaction Cache endpoint.
pub const TX_CACHE_URL: &str = "https://transactions.pecorino.signet.sh";
