//! Bundle submitter that sends test bundles to Parmigiana on a configurable
//! interval.
//!
//! This module provides a [`BundleSubmitter`] that periodically creates simple
//! ETH-transfer bundles and submits them to a Signet RPC endpoint via
//! `signet_sendBundle`. It is intended for integration testing and load
//! generation against the Parmigiana testnet.

use crate::{
    parmigiana_context::{ParmTestError, ParmigianaContext, RollupTransport},
    specs::{signed_simple_send, simple_bundle},
    users::{TEST_SIGNERS, TEST_USERS},
};
use alloy::{
    network::Ethereum,
    primitives::{Address, U256},
    providers::Provider,
};
use signet_bundle::SignetEthBundle;
use signet_constants::parmigiana::RU_CHAIN_ID;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Default bundle RPC URL for the Parmigiana testnet.
pub const BUNDLE_RPC_URL: &str = "https://rpc.parmigiana.signet.sh";

/// Default send amount for test transactions (0.0001 ETH).
const DEFAULT_SEND_AMOUNT: U256 = U256::from_limbs([100_000_000_000_000u64, 0, 0, 0]);

/// Default submission interval (12 seconds, matching block time).
const DEFAULT_INTERVAL: Duration = Duration::from_secs(12);

/// Error types for bundle submission.
#[derive(Debug, thiserror::Error)]
pub enum BundleSubmitError {
    /// Failed to set up the Parmigiana context.
    #[error("parmigiana context setup failed: {0}")]
    Context(#[from] ParmTestError),
    /// Failed to fetch the current nonce.
    #[error("failed to fetch nonce: {0}")]
    Nonce(alloy::transports::TransportError),
    /// Failed to fetch the current block number.
    #[error("failed to fetch block number: {0}")]
    BlockNumber(alloy::transports::TransportError),
    /// HTTP request to the bundle RPC failed.
    #[error("bundle RPC request failed: {0}")]
    Request(#[from] reqwest::Error),
    /// Bundle RPC returned a non-success status.
    #[error("bundle RPC returned error status {status}: {body}")]
    RpcStatus {
        /// HTTP status code.
        status: u16,
        /// Response body.
        body: String,
    },
    /// Failed to serialize the RPC request.
    #[error("failed to serialize RPC request: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Configuration for the [`BundleSubmitter`].
#[derive(Debug, Clone)]
pub struct BundleSubmitterConfig {
    /// Interval between bundle submissions.
    pub interval: Duration,
    /// Index into [`TEST_SIGNERS`] (0–9) for the sending wallet.
    pub wallet_index: usize,
    /// RPC URL for `signet_sendBundle` submissions.
    pub bundle_rpc_url: String,
    /// Amount of ETH (in wei) to send in each test transaction.
    pub send_amount: U256,
    /// Recipient address for test transactions.
    pub recipient: Address,
}

impl Default for BundleSubmitterConfig {
    fn default() -> Self {
        Self {
            interval: DEFAULT_INTERVAL,
            wallet_index: 0,
            bundle_rpc_url: BUNDLE_RPC_URL.to_string(),
            send_amount: DEFAULT_SEND_AMOUNT,
            recipient: TEST_USERS[1],
        }
    }
}

/// Builder for [`BundleSubmitterConfig`].
#[derive(Debug, Default)]
pub struct BundleSubmitterConfigBuilder {
    interval: Option<Duration>,
    wallet_index: Option<usize>,
    bundle_rpc_url: Option<String>,
    send_amount: Option<U256>,
    recipient: Option<Address>,
}

impl BundleSubmitterConfigBuilder {
    /// Set the submission interval.
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    /// Set the wallet index (0–9).
    pub fn wallet_index(mut self, index: usize) -> Self {
        self.wallet_index = Some(index);
        self
    }

    /// Set the bundle RPC URL.
    pub fn bundle_rpc_url(mut self, url: String) -> Self {
        self.bundle_rpc_url = Some(url);
        self
    }

    /// Set the send amount in wei.
    pub fn send_amount(mut self, amount: U256) -> Self {
        self.send_amount = Some(amount);
        self
    }

    /// Set the recipient address.
    pub fn recipient(mut self, addr: Address) -> Self {
        self.recipient = Some(addr);
        self
    }

    /// Build the configuration, using defaults for unset fields.
    pub fn build(self) -> BundleSubmitterConfig {
        let defaults = BundleSubmitterConfig::default();
        BundleSubmitterConfig {
            interval: self.interval.unwrap_or(defaults.interval),
            wallet_index: self.wallet_index.unwrap_or(defaults.wallet_index),
            bundle_rpc_url: self.bundle_rpc_url.unwrap_or(defaults.bundle_rpc_url),
            send_amount: self.send_amount.unwrap_or(defaults.send_amount),
            recipient: self.recipient.unwrap_or(defaults.recipient),
        }
    }
}

impl BundleSubmitterConfig {
    /// Create a new builder for [`BundleSubmitterConfig`].
    pub fn builder() -> BundleSubmitterConfigBuilder {
        BundleSubmitterConfigBuilder::default()
    }
}

/// A bundle submitter that periodically sends test bundles to a Signet RPC
/// endpoint.
///
/// Uses a [`ParmigianaContext`] for network access and [`TEST_SIGNERS`] for
/// transaction signing.
pub struct BundleSubmitter<H, R>
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    /// The Parmigiana test context.
    ctx: ParmigianaContext<H, R>,
    /// Submitter configuration.
    config: BundleSubmitterConfig,
    /// HTTP client for RPC requests.
    client: reqwest::Client,
    /// Current nonce for the sending wallet.
    nonce: u64,
}

impl<H, R> BundleSubmitter<H, R>
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    /// Create a new bundle submitter with the given context and config.
    pub fn new(ctx: ParmigianaContext<H, R>, config: BundleSubmitterConfig) -> Self {
        Self { ctx, config, client: reqwest::Client::new(), nonce: 0 }
    }

    /// Fetch the current nonce for the configured wallet from the network.
    pub async fn refresh_nonce(&mut self) -> Result<u64, BundleSubmitError> {
        let sender = TEST_USERS[self.config.wallet_index];
        let nonce = self
            .ctx
            .ru_provider
            .get_transaction_count(sender)
            .await
            .map_err(BundleSubmitError::Nonce)?;
        self.nonce = nonce;
        debug!(nonce, %sender, "refreshed nonce");
        Ok(nonce)
    }

    /// Get the current block number on the rollup chain.
    pub async fn current_block(&self) -> Result<u64, BundleSubmitError> {
        self.ctx.ru_provider.get_block_number().await.map_err(BundleSubmitError::BlockNumber)
    }

    /// Create a test bundle targeting the next block.
    pub async fn create_test_bundle(&self) -> Result<SignetEthBundle, BundleSubmitError> {
        let block_number = self.current_block().await?;
        let target_block = block_number + 1;

        let signer = &TEST_SIGNERS[self.config.wallet_index];
        let tx = signed_simple_send(
            signer,
            self.config.recipient,
            self.config.send_amount,
            self.nonce,
            RU_CHAIN_ID,
        );

        let bundle = simple_bundle(vec![tx], vec![], target_block);
        debug!(target_block, nonce = self.nonce, "created test bundle");
        Ok(bundle)
    }

    /// Submit a bundle to the RPC endpoint via `signet_sendBundle`.
    pub async fn submit_bundle(
        &self,
        bundle: &SignetEthBundle,
    ) -> Result<String, BundleSubmitError> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "signet_sendBundle",
            "params": [bundle],
            "id": 1
        });

        let response = self.client.post(&self.config.bundle_rpc_url).json(&request).send().await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(BundleSubmitError::RpcStatus { status: status.as_u16(), body });
        }

        info!(%status, "bundle submitted successfully");
        debug!(response = %body, "RPC response");
        Ok(body)
    }

    /// Run the submitter in a continuous loop at the configured interval.
    ///
    /// This method never returns under normal operation. Each iteration
    /// refreshes the nonce, creates a test bundle, submits it, and then
    /// sleeps for the configured interval.
    pub async fn run(&mut self) -> Result<(), BundleSubmitError> {
        info!(
            interval = ?self.config.interval,
            wallet_index = self.config.wallet_index,
            rpc_url = %self.config.bundle_rpc_url,
            "starting bundle submitter"
        );

        loop {
            self.run_single_iteration().await;
            tokio::time::sleep(self.config.interval).await;
        }
    }

    /// Run the submitter for a fixed number of iterations.
    ///
    /// Useful for integration testing where you want to verify the submission
    /// flow without running indefinitely.
    pub async fn run_iterations(&mut self, count: usize) -> Vec<Result<String, BundleSubmitError>> {
        info!(
            count,
            interval = ?self.config.interval,
            wallet_index = self.config.wallet_index,
            "starting bundle submitter for limited iterations"
        );

        let mut results = Vec::with_capacity(count);
        for i in 0..count {
            debug!(iteration = i + 1, total = count, "running iteration");
            results.push(self.run_single_iteration_result().await);
            if i + 1 < count {
                tokio::time::sleep(self.config.interval).await;
            }
        }
        results
    }

    /// Execute a single submission cycle, logging errors without propagating.
    async fn run_single_iteration(&mut self) {
        if let Err(e) = self.refresh_nonce().await {
            warn!(error = %e, "failed to refresh nonce, retrying next interval");
            return;
        }

        let bundle = match self.create_test_bundle().await {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "failed to create test bundle");
                return;
            }
        };

        match self.submit_bundle(&bundle).await {
            Ok(_) => {
                self.nonce += 1;
                info!(nonce = self.nonce, "bundle submitted, nonce incremented");
            }
            Err(e) => error!(error = %e, "failed to submit bundle"),
        }
    }

    /// Execute a single submission cycle, returning the result.
    async fn run_single_iteration_result(&mut self) -> Result<String, BundleSubmitError> {
        self.refresh_nonce().await?;
        let bundle = self.create_test_bundle().await?;
        let result = self.submit_bundle(&bundle).await?;
        self.nonce += 1;
        Ok(result)
    }
}

/// Create a [`BundleSubmitter`] with default configuration.
pub async fn new_bundle_submitter(
) -> Result<BundleSubmitter<impl Provider<Ethereum>, impl Provider<Ethereum>>, BundleSubmitError> {
    new_bundle_submitter_with_config(BundleSubmitterConfig::default()).await
}

/// Create a [`BundleSubmitter`] with custom configuration.
pub async fn new_bundle_submitter_with_config(
    config: BundleSubmitterConfig,
) -> Result<BundleSubmitter<impl Provider<Ethereum>, impl Provider<Ethereum>>, BundleSubmitError> {
    let ctx = crate::parmigiana_context::new_parmigiana_context(RollupTransport::Https).await?;
    Ok(BundleSubmitter::new(ctx, config))
}
