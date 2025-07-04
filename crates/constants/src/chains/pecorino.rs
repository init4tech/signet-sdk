//! Constants for the Pecorino testnet.

use crate::{
    HostConstants, PredeployTokens, RollupConstants, SignetConstants, SignetEnvironmentConstants,
    SignetSystemConstants,
};
use alloy::primitives::{address, Address};
use std::borrow::Cow;

/// Name for the host chain.
pub const HOST_NAME: &str = "Pecorino Host";
/// Chain ID for the Pecorino testnet host chain.
pub const HOST_CHAIN_ID: u64 = 3151908;
/// Deployment height for the Pecorino testnet host chain.
pub const DEPLOY_HEIGHT: u64 = 149984;
/// `Zenith` contract address for the Pecorino testnet host chain.
pub const HOST_ZENITH: Address = address!("0xbe45611502116387211D28cE493D6Fb3d192bc4E");
/// `Orders` contract address for the Pecorino testnet host chain.
pub const HOST_ORDERS: Address = address!("0x4E8cC181805aFC307C83298242271142b8e2f249");
/// `Passage` contract address for the Pecorino testnet host chain.
pub const HOST_PASSAGE: Address = address!("0xd553C4CA4792Af71F4B61231409eaB321c1Dd2Ce");
/// `Transactor` contract address for the Pecorino testnet host chain.
pub const HOST_TRANSACTOR: Address = address!("0x1af3A16857C28917Ab2C4c78Be099fF251669200");

/// USDC token for the Pecorino testnet host chain.
pub const HOST_USDC: Address = address!("0x885F8DB528dC8a38aA3DDad9D3F619746B4a6A81");
/// USDT token for the Pecorino testnet host chain.
pub const HOST_USDT: Address = address!("0x7970D259D4a96764Fa9B23FF0715A35f06f52D1A");
/// WBTC token for the Pecorino testnet host chain.
pub const HOST_WBTC: Address = address!("0x9aeDED4224f3dD31aD8A0B1FcD05E2d7829283a7");

/// USDC token for the Pecorino testnet RU chain.
pub const RU_USDC: Address = address!("0x0B8BC5e60EE10957E0d1A0d95598fA63E65605e2");
/// USDT token for the Pecorino testnet RU chain.
pub const RU_USDT: Address = address!("0xF34326d3521F1b07d1aa63729cB14A372f8A737C");
/// WBTC token for the Pecorino testnet RU chain.
pub const RU_WBTC: Address = address!("0xE3d7066115f7d6b65F88Dff86288dB4756a7D733");

/// Name for the network.
pub const RU_NAME: &str = "Pecorino";
/// Chain ID for the Pecorino testnet RU chain.
pub const RU_CHAIN_ID: u64 = 14174;
/// `Orders` contract address for the Pecorino testnet RU chain.
pub const RU_ORDERS: Address = address!("0xC2D3Dac6B115564B10329697195656459BFb2c74");
/// `Passage` contract address for the Pecorino testnet RU chain.
/// This is currently a dummy value and will be replaced with the actual Passage contract address in the future.
pub const RU_PASSAGE: Address = Address::repeat_byte(0xff);
/// Base fee recipient address for the Pecorino testnet RU chain.
pub const BASE_FEE_RECIPIENT: Address = address!("0xe0eDA3701D44511ce419344A4CeD30B52c9Ba231");

/// Host system tokens for Pecorino.
pub const HOST_TOKENS: PredeployTokens =
    crate::PredeployTokens::new(HOST_USDC, HOST_USDT, HOST_WBTC);

/// RU system tokens for Pecorino.
pub const RU_TOKENS: PredeployTokens = crate::PredeployTokens::new(RU_USDC, RU_USDT, RU_WBTC);

/// The URL of the Transaction Cache endpoint.
pub const TX_CACHE_URL: &str = "https://transactions.pecorino.signet.sh";

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
