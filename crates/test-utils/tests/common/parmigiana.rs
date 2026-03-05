//! Parmigiana testnet test harness.
//!
//! This module provides utilities for running integration tests against
//! the Parmigiana testnet. All configuration values are sourced from
//! [`signet_constants::parmigiana`] constants.

// Allow dead code since this is a test utility module meant to be imported
// by other integration tests that will use these types and functions.
#![allow(dead_code)]

use alloy::{
    network::{Ethereum, EthereumWallet},
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
};
use signet_constants::parmigiana::{
    self, HOST, HOST_CHAIN_ID, HOST_ORDERS, HOST_PASSAGE, HOST_TRANSACTOR, HOST_ZENITH, PARMIGIANA,
    ROLLUP, RU_CHAIN_ID, RU_ORDERS, RU_PASSAGE, TX_CACHE_URL,
};
use signet_test_utils::users::{TEST_SIGNERS, TEST_USERS};
use std::env;

/// Environment variable for the Parmigiana host RPC URL.
pub const HOST_RPC_ENV: &str = "PARMIGIANA_HOST_RPC_URL";
/// Environment variable for the Parmigiana rollup RPC URL.
pub const RU_RPC_ENV: &str = "PARMIGIANA_RU_RPC_URL";

/// Default host RPC URL if not specified via environment.
pub const DEFAULT_HOST_RPC: &str = "https://rpc.parmigiana-host.signet.sh";
/// Default rollup RPC URL if not specified via environment.
pub const DEFAULT_RU_RPC: &str = "https://rpc.parmigiana.signet.sh";

/// Error types for Parmigiana test setup.
#[derive(Debug, thiserror::Error)]
pub enum ParmTestError {
    /// RPC connection failed.
    #[error("RPC connection failed: {0}")]
    RpcConnection(String),
    /// Account not funded.
    #[error("Test account {0} is not funded on Parmigiana")]
    AccountNotFunded(Address),
    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Test context for Parmigiana integration tests.
///
/// Provides access to RPC providers, funded test accounts, and
/// contract addresses for the Parmigiana testnet.
pub struct ParmTestContext<H, R>
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    /// Provider for the host chain.
    pub host_provider: H,
    /// Provider for the rollup chain.
    pub ru_provider: R,
    /// Test signers (deterministic keys from signet-test-utils).
    pub signers: &'static [PrivateKeySigner; 10],
    /// Test user addresses (derived from signers).
    pub users: &'static [Address; 10],
    /// Host RPC URL being used.
    pub host_rpc_url: String,
    /// Rollup RPC URL being used.
    pub ru_rpc_url: String,
}

impl<H, R> ParmTestContext<H, R>
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    /// Returns a copy of the Parmigiana constants.
    #[inline]
    pub fn constants(&self) -> signet_constants::SignetConstants {
        PARMIGIANA
    }

    /// Returns a copy of the host chain constants.
    #[inline]
    pub fn host(&self) -> signet_constants::HostConstants {
        HOST
    }

    /// Returns a copy of the rollup chain constants.
    #[inline]
    pub fn rollup(&self) -> signet_constants::RollupConstants {
        ROLLUP
    }

    /// Returns the host chain ID.
    #[inline]
    pub const fn host_chain_id(&self) -> u64 {
        HOST_CHAIN_ID
    }

    /// Returns the rollup chain ID.
    #[inline]
    pub const fn ru_chain_id(&self) -> u64 {
        RU_CHAIN_ID
    }

    /// Returns the Zenith contract address on the host chain.
    #[inline]
    pub const fn zenith(&self) -> Address {
        HOST_ZENITH
    }

    /// Returns the Orders contract address on the host chain.
    #[inline]
    pub const fn host_orders(&self) -> Address {
        HOST_ORDERS
    }

    /// Returns the Passage contract address on the host chain.
    #[inline]
    pub const fn host_passage(&self) -> Address {
        HOST_PASSAGE
    }

    /// Returns the Transactor contract address on the host chain.
    #[inline]
    pub const fn host_transactor(&self) -> Address {
        HOST_TRANSACTOR
    }

    /// Returns the Orders contract address on the rollup chain.
    #[inline]
    pub const fn ru_orders(&self) -> Address {
        RU_ORDERS
    }

    /// Returns the Passage contract address on the rollup chain.
    #[inline]
    pub const fn ru_passage(&self) -> Address {
        RU_PASSAGE
    }

    /// Returns the transaction cache URL.
    #[inline]
    pub const fn tx_cache_url(&self) -> &'static str {
        TX_CACHE_URL
    }

    /// Returns the first test signer.
    #[inline]
    pub fn primary_signer(&self) -> &PrivateKeySigner {
        &self.signers[0]
    }

    /// Returns the first test user address.
    #[inline]
    pub fn primary_user(&self) -> Address {
        self.users[0]
    }

    /// Creates an Ethereum wallet from a test signer by index.
    pub fn wallet(&self, index: usize) -> EthereumWallet {
        EthereumWallet::from(self.signers[index].clone())
    }

    /// Gets the balance of a test user on the host chain.
    pub async fn host_balance(&self, index: usize) -> Result<U256, ParmTestError> {
        self.host_provider
            .get_balance(self.users[index])
            .await
            .map_err(|e| ParmTestError::RpcConnection(e.to_string()))
    }

    /// Gets the balance of a test user on the rollup chain.
    pub async fn ru_balance(&self, index: usize) -> Result<U256, ParmTestError> {
        self.ru_provider
            .get_balance(self.users[index])
            .await
            .map_err(|e| ParmTestError::RpcConnection(e.to_string()))
    }
}

/// Sets up the Parmigiana test context.
///
/// This function creates RPC providers for both the host and rollup chains,
/// and provides access to test accounts. Test accounts must be pre-funded
/// on the Parmigiana testnet.
///
/// # Environment Variables
///
/// - `PARMIGIANA_HOST_RPC_URL`: Override the default host RPC URL
/// - `PARMIGIANA_RU_RPC_URL`: Override the default rollup RPC URL
///
/// # Example
///
/// ```ignore
/// use tests::common::parmigiana::{setup_parmigiana_test, cleanup_parmigiana_test};
///
/// #[tokio::test]
/// async fn test_parmigiana_connection() {
///     let ctx = setup_parmigiana_test().await.unwrap();
///     
///     // Use ctx.host_provider, ctx.ru_provider, ctx.signers, etc.
///     assert_eq!(ctx.host_chain_id(), signet_constants::parmigiana::HOST_CHAIN_ID);
///     
///     cleanup_parmigiana_test(&ctx);
/// }
/// ```
pub async fn setup_parmigiana_test(
) -> Result<ParmTestContext<impl Provider<Ethereum>, impl Provider<Ethereum>>, ParmTestError> {
    // Get RPC URLs from environment or use defaults
    let host_rpc_url = env::var(HOST_RPC_ENV).unwrap_or_else(|_| DEFAULT_HOST_RPC.to_string());
    let ru_rpc_url = env::var(RU_RPC_ENV).unwrap_or_else(|_| DEFAULT_RU_RPC.to_string());

    // Build providers
    let host_provider = ProviderBuilder::new()
        .connect(&host_rpc_url)
        .await
        .map_err(|e| ParmTestError::InvalidConfig(format!("Invalid host RPC URL: {e}")))?;

    let ru_provider = ProviderBuilder::new()
        .connect(&ru_rpc_url)
        .await
        .map_err(|e| ParmTestError::InvalidConfig(format!("Invalid rollup RPC URL: {e}")))?;

    // Verify chain IDs match expected values
    let host_chain = host_provider
        .get_chain_id()
        .await
        .map_err(|e| ParmTestError::RpcConnection(format!("Failed to get host chain ID: {e}")))?;

    if host_chain != parmigiana::HOST_CHAIN_ID {
        return Err(ParmTestError::InvalidConfig(format!(
            "Host chain ID mismatch: expected {}, got {host_chain}",
            parmigiana::HOST_CHAIN_ID
        )));
    }

    let ru_chain = ru_provider
        .get_chain_id()
        .await
        .map_err(|e| ParmTestError::RpcConnection(format!("Failed to get rollup chain ID: {e}")))?;

    if ru_chain != parmigiana::RU_CHAIN_ID {
        return Err(ParmTestError::InvalidConfig(format!(
            "Rollup chain ID mismatch: expected {}, got {ru_chain}",
            parmigiana::RU_CHAIN_ID
        )));
    }

    Ok(ParmTestContext {
        host_provider,
        ru_provider,
        signers: &TEST_SIGNERS,
        users: &TEST_USERS,
        host_rpc_url,
        ru_rpc_url,
    })
}

/// Cleans up resources after a Parmigiana test.
///
/// Currently performs minimal cleanup as providers are dropped automatically.
/// This function is provided for future extensibility and to maintain a
/// consistent setup/cleanup pattern.
pub fn cleanup_parmigiana_test<H, R>(_ctx: &ParmTestContext<H, R>)
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    // Providers are dropped automatically when ctx goes out of scope.
    // This function is provided for:
    // 1. Consistent API pattern with setup_parmigiana_test
    // 2. Future extensibility (e.g., cleanup temp files, reset state)
    // 3. Explicit indication that test cleanup was considered
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that constants are properly accessible.
    #[test]
    fn test_constants_accessible() {
        // Verify we can access all the key constants
        assert_eq!(parmigiana::HOST_CHAIN_ID, 3151908);
        assert_eq!(parmigiana::RU_CHAIN_ID, 88888);
        assert!(!parmigiana::HOST_ZENITH.is_zero());
        assert!(!parmigiana::HOST_ORDERS.is_zero());
        assert!(!parmigiana::HOST_PASSAGE.is_zero());
        assert!(!parmigiana::RU_ORDERS.is_zero());
        assert!(!parmigiana::RU_PASSAGE.is_zero());
    }
}
