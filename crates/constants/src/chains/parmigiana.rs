//! Constants for the Parmigiana testnet.

use crate::{
    HostConstants, HostTokens, HostUsdRecord, RollupConstants, RollupTokens, SignetConstants,
    SignetEnvironmentConstants, SignetSystemConstants, UsdRecords,
};
use alloy::primitives::{address, Address};
use std::borrow::Cow;

/// Name for the host chain.
pub const HOST_NAME: &str = "Parmigiana Host";
/// Chain ID for the Parmigiana testnet host chain.
pub const HOST_CHAIN_ID: u64 = 3151908;
/// Deployment height for the Parmigiana testnet host chain.
pub const DEPLOY_HEIGHT: u64 = 0;
/// `Zenith` contract address for the Parmigiana testnet host chain.
pub const HOST_ZENITH: Address = address!("0x143A5BE4E559cA49Dbf0966d4B9C398425C5Fc19");
/// `Orders` contract address for the Parmigiana testnet host chain.
pub const HOST_ORDERS: Address = address!("0x96f44ddc3Bc8892371305531F1a6d8ca2331fE6C");
/// `Passage` contract address for the Parmigiana testnet host chain.
pub const HOST_PASSAGE: Address = address!("0x28524D2a753925Ef000C3f0F811cDf452C6256aF");
/// `Transactor` contract address for the Parmigiana testnet host chain.
pub const HOST_TRANSACTOR: Address = address!("0x0B4fc18e78c585687E01c172a1087Ea687943db9");

/// USDC token for the Parmigiana testnet host chain.
pub const HOST_USDC: Address = address!("0x65fb255585458de1f9a246b476aa8d5c5516f6fd");
/// USDT token for the Parmigiana testnet host chain.
pub const HOST_USDT: Address = address!("0xb9df1b911b6cf6935b2a918ba03df2372e94e267");
/// WBTC token for the Parmigiana testnet host chain.
pub const HOST_WBTC: Address = address!("0xfb29f7d7a4ce607d6038d44150315e5f69bea08a");
/// WETH token for the Parmigiana testnet host chain.
pub const HOST_WETH: Address = address!("0xD1278f17e86071f1E658B656084c65b7FD3c90eF");

/// USDC token record for the Parmigiana testnet host chain.
pub const HOST_USDC_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDC, Cow::Borrowed("USDC"), 6);
/// USDT token record for the Parmigiana testnet host chain.
pub const HOST_USDT_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDT, Cow::Borrowed("USDT"), 6);
/// Host USD records for the Parmigiana testnet host chain.
pub const HOST_USD_RECORDS: UsdRecords = {
    let mut records = UsdRecords::new();
    records.push(HOST_USDC_RECORD);
    records.push(HOST_USDT_RECORD);
    records
};
/// Host system tokens for Parmigiana.
pub const HOST_TOKENS: HostTokens = HostTokens::new(HOST_USD_RECORDS, HOST_WBTC, HOST_WETH);

/// Start timestamp for the Parmigiana host chain slot calculator.
pub const HOST_START_TIMESTAMP: u64 = 1765226348;
/// Slot offset for the Parmigiana host chain slot calculator.
pub const HOST_SLOT_OFFSET: u64 = 0;
/// Slot duration for the Parmigiana host chain slot calculator.
pub const HOST_SLOT_DURATION: u64 = 12;

/// Host system constants for Parmigiana.
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
pub const RU_NAME: &str = "Parmigiana";
/// Chain ID for the Parmigiana testnet RU chain.
pub const RU_CHAIN_ID: u64 = 88888;

/// WETH token for the Parmigiana testnet RU chain.
pub const RU_WETH: Address = address!("0x0000000000000000007369676e65742d77657468");
/// WBTC token for the Parmigiana testnet RU chain.
pub const RU_WBTC: Address = address!("0x0000000000000000007369676e65742D77627463");
/// `Orders` contract address for the Parmigiana testnet RU chain.
pub const RU_ORDERS: Address = address!("0x000000000000007369676E65742D6f7264657273");
/// `Passage` contract address for the Parmigiana testnet RU chain.
/// This is currently a dummy value and will be replaced with the actual Passage contract address in the future.
pub const RU_PASSAGE: Address = address!("0x0000000000007369676E65742D70617373616765");
/// The WETH9-based wrapped native USD token contract.
/// This is signet's native token in wrapped form.
pub const WRAPPED: Address = address!("0x0000000000000000007369676e65742D77757364");
/// RU pre-approved system tokens for Parmigiana.
pub const RU_TOKENS: RollupTokens = RollupTokens::new(RU_WBTC, RU_WETH);

/// Base fee recipient address for the Parmigiana testnet RU chain.
pub const BASE_FEE_RECIPIENT: Address = address!("0xe0eDA3701D44511ce419344A4CeD30B52c9Ba231");

/// RU system constants for Parmigiana.
pub const ROLLUP: RollupConstants =
    crate::RollupConstants::new(RU_CHAIN_ID, RU_ORDERS, RU_PASSAGE, BASE_FEE_RECIPIENT, RU_TOKENS);

/// Signet system constants for Parmigiana.
pub const PARMIGIANA_SYS: SignetSystemConstants = crate::SignetSystemConstants::new(HOST, ROLLUP);

/// Signet environment constants for Parmigiana.
pub const PARMIGIANA_ENV: SignetEnvironmentConstants = SignetEnvironmentConstants::new(
    Cow::Borrowed(HOST_NAME),
    Cow::Borrowed(RU_NAME),
    Cow::Borrowed(TX_CACHE_URL),
);

/// Signet constants for Parmigiana.
pub const PARMIGIANA: SignetConstants = SignetConstants::new(PARMIGIANA_SYS, PARMIGIANA_ENV);

/// The URL of the Transaction Cache endpoint.
pub const TX_CACHE_URL: &str = "https://transactions.parmigiana.signet.sh";
