//! Constants for the Gouda testnet (runs on the Parmigiana host chain).

use crate::{
    HostConstants, HostTokens, HostUsdRecord, RollupConstants, RollupTokens, SignetConstants,
    SignetEnvironmentConstants, SignetSystemConstants, UsdRecords,
};
use alloy::primitives::{address, Address};
use std::borrow::Cow;

/// Name for the host chain (gouda reuses the parmigiana host chain).
pub const HOST_NAME: &str = "Parmigiana Host";
/// Chain ID for the Parmigiana host chain (gouda's host).
pub const HOST_CHAIN_ID: u64 = 3151908;
/// Deployment height of the gouda host contracts.
pub const DEPLOY_HEIGHT: u64 = 1143386;
/// `Zenith` contract address for the gouda deployment on parmigiana host.
pub const HOST_ZENITH: Address = address!("0x9872Fa449306838614872d47Dee01FC0f0827cf7");
/// `Orders` contract address for the gouda deployment on parmigiana host.
pub const HOST_ORDERS: Address = address!("0x5C5cd1F1c35227b14F6A94c6e05347403F4C963E");
/// `Passage` contract address for the gouda deployment on parmigiana host.
pub const HOST_PASSAGE: Address = address!("0x57348c54e3F89097579dFcD4F5d2700ca2EB1906");
/// `Transactor` contract address for the gouda deployment on parmigiana host.
pub const HOST_TRANSACTOR: Address = address!("0x31797B512a1481FF2DCDD26f1facb50fD344BF7F");

/// USDC token for the gouda host deployment.
pub const HOST_USDC: Address = address!("0x6a27cc6968b1d08cd04a656075cc25905156827e");
/// USDT token for the gouda host deployment.
pub const HOST_USDT: Address = address!("0x3aad2b8d721bb2f7f79356515d15aad3f8d6a32d");
/// WBTC token for the gouda host deployment.
pub const HOST_WBTC: Address = address!("0x260fdcb6e6e2c1c5f96647ee0fae34e7e92e6f28");
/// WETH token for the gouda host (canonical WETH9 on the parmigiana host chain).
pub const HOST_WETH: Address = address!("0xD1278f17e86071f1E658B656084c65b7FD3c90eF");

/// USDC token record for the gouda host deployment.
pub const HOST_USDC_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDC, Cow::Borrowed("USDC"), 6);
/// USDT token record for the gouda host deployment.
pub const HOST_USDT_RECORD: HostUsdRecord = HostUsdRecord::new(HOST_USDT, Cow::Borrowed("USDT"), 6);
/// Host USD records for the gouda deployment.
pub const HOST_USD_RECORDS: UsdRecords = {
    let mut records = UsdRecords::new();
    records.push(HOST_USDC_RECORD);
    records.push(HOST_USDT_RECORD);
    records
};
/// Host system tokens for gouda.
pub const HOST_TOKENS: HostTokens = HostTokens::new(HOST_USD_RECORDS, HOST_WBTC, HOST_WETH);

/// Start timestamp for the gouda host slot calculator (matches parmigiana host start).
pub const HOST_START_TIMESTAMP: u64 = 1779051536;
/// Slot offset for the gouda host slot calculator.
pub const HOST_SLOT_OFFSET: u64 = 0;
/// Slot duration for the gouda host slot calculator.
pub const HOST_SLOT_DURATION: u64 = 12;

/// Host system constants for gouda.
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

/// Name for the gouda rollup.
pub const RU_NAME: &str = "Gouda";
/// Chain ID for the gouda rollup.
pub const RU_CHAIN_ID: u64 = 792669;

/// WETH token for the gouda rollup (system magic address).
pub const RU_WETH: Address = address!("0x0000000000000000007369676e65742d77657468");
/// WBTC token for the gouda rollup (system magic address).
pub const RU_WBTC: Address = address!("0x0000000000000000007369676e65742D77627463");
/// `Orders` contract address for the gouda rollup (system magic address).
pub const RU_ORDERS: Address = address!("0x000000000000007369676E65742D6f7264657273");
/// `Passage` contract address for the gouda rollup (system magic address).
pub const RU_PASSAGE: Address = address!("0x0000000000007369676E65742D70617373616765");
/// The WETH9-based wrapped native USD token contract (signet system).
pub const WRAPPED: Address = address!("0x0000000000000000007369676e65742D77757364");
/// RU pre-approved system tokens for gouda.
pub const RU_TOKENS: RollupTokens = RollupTokens::new(RU_WBTC, RU_WETH);

/// Base fee recipient address for the gouda rollup.
pub const BASE_FEE_RECIPIENT: Address = address!("0xe0eDA3701D44511ce419344A4CeD30B52c9Ba231");

/// RU system constants for gouda.
pub const ROLLUP: RollupConstants =
    crate::RollupConstants::new(RU_CHAIN_ID, RU_ORDERS, RU_PASSAGE, BASE_FEE_RECIPIENT, RU_TOKENS);

/// Signet system constants for gouda.
pub const GOUDA_SYS: SignetSystemConstants = crate::SignetSystemConstants::new(HOST, ROLLUP);

/// The URL of the Transaction Cache endpoint for gouda.
pub const TX_CACHE_URL: &str = "https://transactions.gouda.signet.sh";

/// Signet environment constants for gouda.
pub const GOUDA_ENV: SignetEnvironmentConstants = SignetEnvironmentConstants::new(
    Cow::Borrowed(HOST_NAME),
    Cow::Borrowed(RU_NAME),
    Cow::Borrowed(TX_CACHE_URL),
);

/// Signet constants for gouda.
pub const GOUDA: SignetConstants = SignetConstants::new(GOUDA_SYS, GOUDA_ENV);
