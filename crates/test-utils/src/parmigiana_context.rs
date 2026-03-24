//! Parmigiana testnet test harness.
//!
//! This module provides utilities for running integration tests against
//! the Parmigiana testnet. All configuration values are sourced from
//! [`signet_constants::parmigiana`] constants.

use crate::users::{TEST_SIGNERS, TEST_USERS};
use alloy::{
    consensus::TxEnvelope,
    network::ReceiptResponse,
    network::{Ethereum, EthereumWallet},
    primitives::{Address, TxHash, B256, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    transports::TransportError,
};
use signet_bundle::SignetEthBundle;
use signet_constants::parmigiana::{HOST_CHAIN_ID, RU_CHAIN_ID};
use signet_tx_cache::{
    types::{BundleResponse, TransactionResponse},
    TxCache, TxCacheError,
};
use std::time::Duration;
use tokio::time::{sleep, Instant};

const DEFAULT_BUNDLE_TARGET_BLOCK_OFFSET: u64 = 2;

/// Host chain RPC URL for the Parmigiana testnet.
pub const HOST_RPC_URL: &str = "https://host-rpc.parmigiana.signet.sh";
/// Rollup HTTP RPC URL for the Parmigiana testnet.
pub const RU_RPC_URL: &str = "https://rpc.parmigiana.signet.sh";
/// Rollup WebSocket RPC URL for the Parmigiana testnet.
pub const RU_WS_URL: &str = "wss://rpc.parmigiana.signet.sh";

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
    /// Failed to fetch a nonce on the host chain.
    #[error("failed to fetch host transaction count: {0}")]
    HostTransactionCount(TransportError),
    /// Failed to fetch a receipt on the host chain.
    #[error("failed to fetch host receipt: {0}")]
    HostReceipt(TransportError),
    /// Failed to fetch a transaction by hash on the host chain.
    #[error("failed to fetch host transaction by hash: {0}")]
    HostTransactionByHash(TransportError),
    /// Failed to fetch the current block number on the host chain.
    #[error("failed to fetch host block number: {0}")]
    HostBlockNumber(TransportError),
    /// A transaction timed out before it was mined on the host chain.
    #[error(
        "timed out waiting for host receipt for tx {tx_hash:#x}; latest_block={latest_block}; seen_in_pool={seen_in_pool}"
    )]
    HostReceiptTimeout {
        /// Transaction hash being tracked.
        tx_hash: TxHash,
        /// Latest observed host block number.
        latest_block: u64,
        /// Whether the transaction was still visible in the node's mempool.
        seen_in_pool: bool,
    },
    /// The transaction was mined on the host chain but did not succeed.
    #[error("host transaction {tx_hash:#x} failed")]
    HostTransactionFailed {
        /// Transaction hash being tracked.
        tx_hash: TxHash,
    },
    /// Failed to fetch a balance on the rollup chain.
    #[error("failed to fetch rollup balance: {0}")]
    RollupBalance(TransportError),
    /// Failed to fetch a nonce on the rollup chain.
    #[error("failed to fetch rollup transaction count: {0}")]
    RollupTransactionCount(TransportError),
    /// Failed to fetch a receipt on the rollup chain.
    #[error("failed to fetch rollup receipt: {0}")]
    RollupReceipt(TransportError),
    /// Failed to fetch a transaction by hash on the rollup chain.
    #[error("failed to fetch rollup transaction by hash: {0}")]
    RollupTransactionByHash(TransportError),
    /// Failed to fetch the current block number on the rollup chain.
    #[error("failed to fetch rollup block number: {0}")]
    RollupBlockNumber(TransportError),
    /// A transaction timed out before it was mined on the rollup chain.
    #[error(
        "timed out waiting for receipt for tx {tx_hash:#x}; latest_block={latest_block}; seen_in_pool={seen_in_pool}"
    )]
    RollupReceiptTimeout {
        /// Transaction hash being tracked.
        tx_hash: TxHash,
        /// Latest observed rollup block number.
        latest_block: u64,
        /// Whether the transaction was still visible in the node's mempool.
        seen_in_pool: bool,
    },
    /// The transaction was mined but did not succeed.
    #[error("rollup transaction {tx_hash:#x} failed")]
    RollupTransactionFailed {
        /// Transaction hash being tracked.
        tx_hash: TxHash,
    },
    /// The mined receipt did not include a block number.
    #[error("rollup receipt for tx {tx_hash:#x} was missing a block number")]
    MissingReceiptBlockNumber {
        /// Transaction hash being tracked.
        tx_hash: TxHash,
    },
    /// The mined receipt did not include a block hash.
    #[error("rollup receipt for tx {tx_hash:#x} was missing a block hash")]
    MissingReceiptBlockHash {
        /// Transaction hash being tracked.
        tx_hash: TxHash,
    },
    /// The mined receipt did not include a transaction index.
    #[error("rollup receipt for tx {tx_hash:#x} was missing a transaction index")]
    MissingReceiptTransactionIndex {
        /// Transaction hash being tracked.
        tx_hash: TxHash,
    },
    /// Failed to submit a transaction to tx-cache.
    #[error("failed to submit transaction to tx-cache: {0}")]
    TxCacheSubmit(TxCacheError),
    /// Failed to query tx-cache.
    #[error("failed to query tx-cache: {0}")]
    TxCacheQuery(TxCacheError),
    /// The transaction never appeared in tx-cache before the timeout elapsed.
    #[error("timed out waiting for tx {tx_hash:#x} to appear in tx-cache")]
    TxCacheTimeout {
        /// Transaction hash being tracked.
        tx_hash: TxHash,
    },
    /// Failed to submit a bundle to tx-cache.
    #[error("failed to submit bundle to tx-cache: {0}")]
    BundleCacheSubmit(TxCacheError),
}

/// Receipt metadata for a confirmed Parmigiana rollup transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfirmedRollupTransaction {
    /// Submitted transaction hash.
    pub tx_hash: TxHash,
    /// Block number that confirmed the transaction.
    pub block_number: u64,
    /// Block hash that confirmed the transaction.
    pub block_hash: B256,
    /// Index of the transaction inside the block.
    pub transaction_index: u64,
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

    /// Returns a test signer by index.
    ///
    /// # Panics
    ///
    /// Panics if `index` >= 10.
    pub fn signer(&self, index: usize) -> &PrivateKeySigner {
        &self.signers[index]
    }

    /// Returns a test user address by index.
    ///
    /// # Panics
    ///
    /// Panics if `index` >= 10.
    pub fn user(&self, index: usize) -> Address {
        self.users[index]
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

    /// Gets the next nonce for an address on the host chain.
    pub async fn host_transaction_count(&self, address: Address) -> Result<u64, ParmTestError> {
        self.host_provider
            .get_transaction_count(address)
            .pending()
            .await
            .map_err(ParmTestError::HostTransactionCount)
    }

    /// Gets the balance of a test user on the rollup chain.
    ///
    /// # Panics
    ///
    /// Panics if `index` >= 10.
    pub async fn ru_balance(&self, index: usize) -> Result<U256, ParmTestError> {
        self.ru_provider.get_balance(self.users[index]).await.map_err(ParmTestError::RollupBalance)
    }

    /// Gets the host chain ID from the live Parmigiana node.
    pub async fn host_chain_id(&self) -> Result<u64, ParmTestError> {
        self.host_provider.get_chain_id().await.map_err(ParmTestError::HostChainId)
    }

    /// Gets the rollup chain ID from the live Parmigiana node.
    pub async fn ru_chain_id(&self) -> Result<u64, ParmTestError> {
        self.ru_provider.get_chain_id().await.map_err(ParmTestError::RollupChainId)
    }

    /// Gets the balance of any address on the rollup chain.
    pub async fn ru_native_balance_of(&self, address: Address) -> Result<U256, ParmTestError> {
        self.ru_provider.get_balance(address).await.map_err(ParmTestError::RollupBalance)
    }

    /// Gets the next nonce for an address on the rollup chain.
    pub async fn ru_transaction_count(&self, address: Address) -> Result<u64, ParmTestError> {
        self.ru_provider
            .get_transaction_count(address)
            .pending()
            .await
            .map_err(ParmTestError::RollupTransactionCount)
    }

    /// Gets the current rollup block number.
    pub async fn ru_block_number(&self) -> Result<u64, ParmTestError> {
        self.ru_provider.get_block_number().await.map_err(ParmTestError::RollupBlockNumber)
    }

    /// Creates a tx-cache client configured for Parmigiana.
    pub fn tx_cache(&self) -> TxCache {
        TxCache::parmigiana()
    }

    /// Creates a Signet bundle targeting a future rollup block.
    pub async fn bundle_for_target_ru_block(
        &self,
        txs: Vec<TxEnvelope>,
        host_txs: Vec<TxEnvelope>,
        block_offset: u64,
    ) -> Result<SignetEthBundle, ParmTestError> {
        let block_number = self.ru_block_number().await? + block_offset;
        let mut bundle = SignetEthBundle::from_transactions(txs, host_txs, block_number);
        bundle.bundle.replacement_uuid = Some(uuid::Uuid::new_v4().to_string());
        Ok(bundle)
    }

    /// Creates a Signet bundle targeting the next usable rollup block window.
    pub async fn bundle_for_next_ru_block(
        &self,
        txs: Vec<TxEnvelope>,
        host_txs: Vec<TxEnvelope>,
    ) -> Result<SignetEthBundle, ParmTestError> {
        self.bundle_for_target_ru_block(txs, host_txs, DEFAULT_BUNDLE_TARGET_BLOCK_OFFSET).await
    }

    /// Creates a rollup-only Signet bundle targeting the next usable rollup
    /// block window.
    pub async fn rollup_bundle_for_next_ru_block(
        &self,
        txs: Vec<TxEnvelope>,
    ) -> Result<SignetEthBundle, ParmTestError> {
        self.bundle_for_next_ru_block(txs, vec![]).await
    }

    /// Forwards a signed rollup transaction to the Parmigiana tx-cache.
    pub async fn forward_rollup_transaction(
        &self,
        tx: TxEnvelope,
    ) -> Result<TransactionResponse, ParmTestError> {
        self.tx_cache().forward_raw_transaction(tx).await.map_err(ParmTestError::TxCacheSubmit)
    }

    /// Forwards a Signet bundle to the Parmigiana tx-cache.
    pub async fn forward_bundle(
        &self,
        bundle: SignetEthBundle,
    ) -> Result<BundleResponse, ParmTestError> {
        self.tx_cache().forward_bundle(bundle).await.map_err(ParmTestError::BundleCacheSubmit)
    }

    /// Waits until a transaction becomes visible in the Parmigiana tx-cache.
    pub async fn wait_for_transaction_in_cache(
        &self,
        tx_hash: TxHash,
        timeout: Duration,
    ) -> Result<(), ParmTestError> {
        let tx_cache = self.tx_cache();
        let started = Instant::now();
        loop {
            let mut cursor = None;
            let mut found = false;

            loop {
                let page = tx_cache
                    .get_transactions(cursor.clone())
                    .await
                    .map_err(ParmTestError::TxCacheQuery)?;
                if page.transactions.iter().any(|tx| *tx.hash() == tx_hash) {
                    found = true;
                    break;
                }

                let Some(next_cursor) = page.next_cursor().cloned() else {
                    break;
                };
                cursor = Some(next_cursor);
            }

            if found {
                return Ok(());
            }

            if started.elapsed() > timeout {
                return Err(ParmTestError::TxCacheTimeout { tx_hash });
            }

            sleep(Duration::from_secs(2)).await;
        }
    }

    /// Waits for a successful receipt on the Parmigiana host chain.
    pub async fn wait_for_successful_host_receipt(
        &self,
        tx_hash: TxHash,
        timeout: Duration,
    ) -> Result<ConfirmedRollupTransaction, ParmTestError> {
        let started = Instant::now();
        let receipt = loop {
            let receipt = self
                .host_provider
                .get_transaction_receipt(tx_hash)
                .await
                .map_err(ParmTestError::HostReceipt)?;
            if let Some(receipt) = receipt {
                break receipt;
            }

            if started.elapsed() > timeout {
                let latest_block = self
                    .host_provider
                    .get_block_number()
                    .await
                    .map_err(ParmTestError::HostBlockNumber)?;
                let seen_in_pool = self
                    .host_provider
                    .get_transaction_by_hash(tx_hash)
                    .await
                    .map_err(ParmTestError::HostTransactionByHash)?
                    .is_some();
                return Err(ParmTestError::HostReceiptTimeout {
                    tx_hash,
                    latest_block,
                    seen_in_pool,
                });
            }

            sleep(Duration::from_secs(2)).await;
        };

        if !receipt.status() {
            return Err(ParmTestError::HostTransactionFailed { tx_hash });
        }

        Ok(ConfirmedRollupTransaction {
            tx_hash,
            block_number: receipt
                .block_number()
                .ok_or(ParmTestError::MissingReceiptBlockNumber { tx_hash })?,
            block_hash: receipt
                .block_hash()
                .ok_or(ParmTestError::MissingReceiptBlockHash { tx_hash })?,
            transaction_index: receipt
                .transaction_index()
                .ok_or(ParmTestError::MissingReceiptTransactionIndex { tx_hash })?,
        })
    }

    /// Waits for a successful receipt on the Parmigiana rollup node.
    pub async fn wait_for_successful_ru_receipt(
        &self,
        tx_hash: TxHash,
        timeout: Duration,
    ) -> Result<ConfirmedRollupTransaction, ParmTestError> {
        let started = Instant::now();
        let receipt = loop {
            let receipt = self
                .ru_provider
                .get_transaction_receipt(tx_hash)
                .await
                .map_err(ParmTestError::RollupReceipt)?;
            if let Some(receipt) = receipt {
                break receipt;
            }

            if started.elapsed() > timeout {
                let latest_block = self
                    .ru_provider
                    .get_block_number()
                    .await
                    .map_err(ParmTestError::RollupBlockNumber)?;
                let seen_in_pool = self
                    .ru_provider
                    .get_transaction_by_hash(tx_hash)
                    .await
                    .map_err(ParmTestError::RollupTransactionByHash)?
                    .is_some();
                return Err(ParmTestError::RollupReceiptTimeout {
                    tx_hash,
                    latest_block,
                    seen_in_pool,
                });
            }

            sleep(Duration::from_secs(2)).await;
        };

        if !receipt.status() {
            return Err(ParmTestError::RollupTransactionFailed { tx_hash });
        }

        Ok(ConfirmedRollupTransaction {
            tx_hash,
            block_number: receipt
                .block_number()
                .ok_or(ParmTestError::MissingReceiptBlockNumber { tx_hash })?,
            block_hash: receipt
                .block_hash()
                .ok_or(ParmTestError::MissingReceiptBlockHash { tx_hash })?,
            transaction_index: receipt
                .transaction_index()
                .ok_or(ParmTestError::MissingReceiptTransactionIndex { tx_hash })?,
        })
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
