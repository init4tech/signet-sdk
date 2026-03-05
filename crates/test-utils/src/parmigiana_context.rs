//! Parmigiana testnet test harness.
//!
//! This module provides utilities for running integration tests against
//! the Parmigiana testnet. All configuration values are sourced from
//! [`signet_constants::parmigiana`] constants.

use crate::users::{TEST_SIGNERS, TEST_USERS};
use alloy::{
    network::{Ethereum, EthereumWallet},
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    transports::TransportError,
};
use signet_constants::parmigiana::{HOST_CHAIN_ID, RU_CHAIN_ID};

/// Host chain RPC URL for the Parmigiana testnet.
pub const HOST_RPC_URL: &str = "https://host-rpc.parmigiana.signet.sh";
/// Rollup HTTP RPC URL for the Parmigiana testnet.
pub const RU_RPC_URL: &str = "https://rpc.parmigiana.signet.sh";
/// Rollup WebSocket RPC URL for the Parmigiana testnet.
pub const RU_WS_URL: &str = "ws://rpc.parmigiana.signet.sh";

/// Transport protocol for the rollup RPC connection.
#[derive(Debug, Clone, Copy, Default)]
pub enum RollupTransport {
    /// HTTP transport (`https://rpc.parmigiana.signet.sh`).
    #[default]
    Https,
    /// WebSocket transport (`wss://rpc.parmigiana.signet.sh`).
    Wss,
}

impl RollupTransport {
    /// Returns the RPC URL for this transport.
    pub const fn url(&self) -> &'static str {
        match self {
            Self::Https => RU_RPC_URL,
            Self::Wss => RU_WS_URL,
        }
    }
}

/// Error types for Parmigiana test setup.
#[derive(Debug, thiserror::Error)]
pub enum ParmTestError {
    /// Failed to connect to the host RPC.
    #[error("failed to connect to host RPC: {0}")]
    HostConnect(TransportError),
    /// Failed to connect to the rollup RPC.
    #[error("failed to connect to rollup RPC: {0}")]
    RollupConnect(TransportError),
    /// Failed to fetch the host chain ID.
    #[error("failed to fetch host chain ID: {0}")]
    HostChainId(TransportError),
    /// Failed to fetch the rollup chain ID.
    #[error("failed to fetch rollup chain ID: {0}")]
    RollupChainId(TransportError),
    /// Host chain ID does not match the expected value.
    #[error("host chain ID mismatch: expected {expected}, got {actual}")]
    HostChainIdMismatch {
        /// Expected chain ID.
        expected: u64,
        /// Actual chain ID.
        actual: u64,
    },
    /// Rollup chain ID does not match the expected value.
    #[error("rollup chain ID mismatch: expected {expected}, got {actual}")]
    RollupChainIdMismatch {
        /// Expected chain ID.
        expected: u64,
        /// Actual chain ID.
        actual: u64,
    },
    /// Failed to fetch a balance on the host chain.
    #[error("failed to fetch host balance: {0}")]
    HostBalance(TransportError),
    /// Failed to fetch a balance on the rollup chain.
    #[error("failed to fetch rollup balance: {0}")]
    RollupBalance(TransportError),
}

/// Test context for Parmigiana integration tests.
///
/// Provides access to RPC providers and funded test accounts for the
/// Parmigiana testnet. Contract addresses and chain constants are available
/// directly from [`signet_constants::parmigiana`].
pub struct ParmigianaContext<H, R>
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
}

impl<H, R> ParmigianaContext<H, R>
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    /// Returns the first test signer.
    pub fn primary_signer(&self) -> &PrivateKeySigner {
        &self.signers[0]
    }

    /// Returns the first test user address.
    pub fn primary_user(&self) -> Address {
        self.users[0]
    }

    /// Creates an Ethereum wallet from a test signer by index.
    ///
    /// # Panics
    ///
    /// Panics if `index` >= 10.
    pub fn wallet(&self, index: usize) -> EthereumWallet {
        EthereumWallet::from(self.signers[index].clone())
    }

    /// Gets the balance of a test user on the host chain.
    ///
    /// # Panics
    ///
    /// Panics if `index` >= 10.
    pub async fn host_balance(&self, index: usize) -> Result<U256, ParmTestError> {
        self.host_provider.get_balance(self.users[index]).await.map_err(ParmTestError::HostBalance)
    }

    /// Gets the balance of a test user on the rollup chain.
    ///
    /// # Panics
    ///
    /// Panics if `index` >= 10.
    pub async fn ru_balance(&self, index: usize) -> Result<U256, ParmTestError> {
        self.ru_provider.get_balance(self.users[index]).await.map_err(ParmTestError::RollupBalance)
    }
}

/// Sets up the Parmigiana test context.
///
/// Creates RPC providers for both the host and rollup chains and provides
/// access to test accounts. Test accounts must be pre-funded on the
/// Parmigiana testnet.
pub async fn new_parmigiana_context(
    ru_transport: RollupTransport,
) -> Result<ParmigianaContext<impl Provider<Ethereum>, impl Provider<Ethereum>>, ParmTestError> {
    let host_provider =
        ProviderBuilder::new().connect(HOST_RPC_URL).await.map_err(ParmTestError::HostConnect)?;

    let ru_provider = ProviderBuilder::new()
        .connect(ru_transport.url())
        .await
        .map_err(ParmTestError::RollupConnect)?;

    let host_chain = host_provider.get_chain_id().await.map_err(ParmTestError::HostChainId)?;

    if host_chain != HOST_CHAIN_ID {
        return Err(ParmTestError::HostChainIdMismatch {
            expected: HOST_CHAIN_ID,
            actual: host_chain,
        });
    }

    let ru_chain = ru_provider.get_chain_id().await.map_err(ParmTestError::RollupChainId)?;

    if ru_chain != RU_CHAIN_ID {
        return Err(ParmTestError::RollupChainIdMismatch {
            expected: RU_CHAIN_ID,
            actual: ru_chain,
        });
    }

    Ok(ParmigianaContext { host_provider, ru_provider, signers: &TEST_SIGNERS, users: &TEST_USERS })
}
