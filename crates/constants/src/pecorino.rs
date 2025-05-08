//! Constants for the Pecorino testnet.

use crate::{HostConstants, PredeployTokens, RollupConstants, SignetSystemConstants};
use alloy::primitives::Address;

/// Chain ID for the Pecorino testnet host chain.
pub const HOST_ID: u64 = todo!();
/// Deployment height for the Pecorino testnet host chain.
pub const DEPLOY_HEIGHT: u64 = todo!();
/// `Zenith` contract address for the Pecorino testnet host chain.
pub const HOST_ZENITH: Address = todo!();
/// `Orders` contract address for the Pecorino testnet host chain.
pub const HOST_ORDERS: Address = todo!();
/// `Passage` contract address for the Pecorino testnet host chain.
pub const HOST_PASSAGE: Address = todo!();
/// `Transactor` contract address for the Pecorino testnet host chain.
pub const HOST_TRANSACTOR: Address = todo!();

/// USDC token for the Pecorino testnet host chain.
pub const HOST_USDC: Address = todo!();
/// USDT token for the Pecorino testnet host chain.
pub const HOST_USDT: Address = todo!();
/// WBTC token for the Pecorino testnet host chain.
pub const HOST_WBTC: Address = todo!();

/// USDC token for the Pecorino testnet RU chain.
pub const RU_USDC: Address = todo!();
/// USDT token for the Pecorino testnet RU chain.
pub const RU_USDT: Address = todo!();
/// WBTC token for the Pecorino testnet RU chain.
pub const RU_WBTC: Address = todo!();

/// Chain ID for the Pecorino testnet RU chain.
pub const ROLLUP_ID: u64 = todo!();
/// `Orders` contract address for the Pecorino testnet RU chain.
pub const RU_ORDERS: Address = todo!();
/// `Passage` contract address for the Pecorino testnet RU chain.
pub const RU_PASSAGE: Address = todo!();
/// Base fee recipient address for the Pecorino testnet RU chain.
pub const BASE_FEE_RECIPIENT: Address = todo!();

/// Host system tokens for Pecorino.
pub const HOST_TOKENS: crate::PredeployTokens =
    crate::PredeployTokens::new(HOST_USDC, HOST_USDT, HOST_WBTC);

/// RU system tokens for Pecorino.
pub const RU_TOKENS: crate::PredeployTokens =
    crate::PredeployTokens::new(RU_USDC, RU_USDT, RU_WBTC);

/// Host system constants for Pecorino.
pub const HOST: crate::HostConstants = crate::HostConstants::new(
    HOST_ID,
    DEPLOY_HEIGHT,
    HOST_ZENITH,
    HOST_ORDERS,
    HOST_PASSAGE,
    HOST_TRANSACTOR,
    HOST_TOKENS,
);

/// RU system constants for Pecorino.
pub const ROLLUP: crate::RollupConstants =
    crate::RollupConstants::new(ROLLUP_ID, RU_ORDERS, RU_PASSAGE, BASE_FEE_RECIPIENT, RU_TOKENS);

/// Signet system constants for Pecorino.
pub const PECORINO: SignetSystemConstants = crate::SignetSystemConstants::new(HOST, ROLLUP);
