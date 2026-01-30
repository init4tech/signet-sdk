use crate::{BundleSubmitter, FillSubmitter, OrdersAndFills, TxBuilder};
use alloy::primitives::Address;
use alloy::{
    eips::eip2718::Encodable2718,
    network::{Ethereum, Network, TransactionBuilder},
    primitives::Bytes,
    providers::SendableTx,
    rpc::types::mev::EthSendBundle,
    transports::{RpcError, TransportErrorKind},
};
use signet_bundle::SignetEthBundle;
use signet_constants::SignetSystemConstants;
use tracing::instrument;

/// Errors returned by [`FeePolicySubmitter`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FeePolicyError {
    /// No fills provided for submission.
    #[error("no fills provided for submission")]
    NoFills,
    /// Failed to get block number from provider.
    #[error("failed to get block number: {0}")]
    BlockNumber(#[source] RpcError<TransportErrorKind>),
    /// Failed to fill transaction.
    #[error("failed to fill transaction: {0}")]
    FillTransaction(#[source] RpcError<TransportErrorKind>),
    /// Transaction fill returned builder instead of envelope.
    #[error("transaction fill did not return signed envelope")]
    NotEnvelope,
    /// Bundle submission failed.
    #[error("failed to submit bundle: {0}")]
    Submission(#[source] Box<dyn core::error::Error + Send + Sync>),
}

/// A [`FillSubmitter`] that wraps a [`BundleSubmitter`] and handles fee policy.
///
/// This submitter converts [`SignedFill`]s into transactions with appropriate gas pricing, builds
/// a [`SignetEthBundle`], and submits via the wrapped submitter.
///
/// The providers must be configured with appropriate fillers for gas, nonce, chain ID, and wallet
/// signing (e.g., via `ProviderBuilder::wallet()`).
#[derive(Debug, Clone)]
pub struct FeePolicySubmitter<RuP, HostP, B> {
    ru_provider: RuP,
    host_provider: HostP,
    submitter: B,
    constants: SignetSystemConstants,
}

impl<RuP, HostP, B> FeePolicySubmitter<RuP, HostP, B> {
    /// Create a new `FeePolicySubmitter`.
    pub const fn new(
        ru_provider: RuP,
        host_provider: HostP,
        submitter: B,
        constants: SignetSystemConstants,
    ) -> Self {
        Self { ru_provider, host_provider, submitter, constants }
    }

    /// Get a reference to the rollup provider.
    pub const fn ru_provider(&self) -> &RuP {
        &self.ru_provider
    }

    /// Get a reference to the host provider.
    pub const fn host_provider(&self) -> &HostP {
        &self.host_provider
    }

    /// Get a reference to the inner submitter.
    pub const fn submitter(&self) -> &B {
        &self.submitter
    }

    /// Get a reference to the system constants.
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }
}

impl<RuP, HostP, B> FillSubmitter for FeePolicySubmitter<RuP, HostP, B>
where
    RuP: TxBuilder<Ethereum>,
    HostP: TxBuilder<Ethereum>,
    B: BundleSubmitter + Send + Sync,
{
    type Response = B::Response;
    type Error = FeePolicyError;

    #[instrument(skip_all, fields(order_count = orders.len(), fill_count = fills.len()))]
    async fn submit_fills(
        &self,
        OrdersAndFills { orders, fills, signer_address }: OrdersAndFills,
    ) -> Result<Self::Response, Self::Error> {
        if fills.is_empty() {
            return Err(FeePolicyError::NoFills);
        }

        // Build rollup transaction requests: fill (if present, must come first) then initiates
        let mut rollup_txs = Vec::with_capacity(orders.len() + 1);
        if let Some(fill) = fills.get(&self.constants.ru_chain_id()) {
            let tx_request = fill.to_fill_tx(self.constants.ru_orders());
            rollup_txs
                .push(sign_and_encode_tx(&self.ru_provider, tx_request, signer_address).await?);
        }
        for order in &orders {
            let tx_request = order.to_initiate_tx(signer_address, self.constants.ru_orders());
            rollup_txs
                .push(sign_and_encode_tx(&self.ru_provider, tx_request, signer_address).await?);
        }

        // Build host transaction request: fill only (if present)
        let host_txs = match fills.get(&self.constants.host_chain_id()) {
            Some(fill) => {
                let tx_request = fill.to_fill_tx(self.constants.host_orders());
                vec![sign_and_encode_tx(&self.host_provider, tx_request, signer_address).await?]
            }
            None => vec![],
        };

        let target_block =
            self.ru_provider.get_block_number().await.map_err(FeePolicyError::BlockNumber)? + 1;

        let bundle = SignetEthBundle::new(
            EthSendBundle { txs: rollup_txs, block_number: target_block, ..Default::default() },
            host_txs,
        );

        self.submitter
            .submit_bundle(bundle)
            .await
            .map_err(|error| FeePolicyError::Submission(Box::new(error)))
    }
}

/// Sign and encode a transaction request for inclusion in a bundle.
#[instrument(skip_all)]
async fn sign_and_encode_tx<N, P>(
    provider: &P,
    mut tx_request: N::TransactionRequest,
    signer_address: Address,
) -> Result<Bytes, FeePolicyError>
where
    N: Network,
    P: TxBuilder<N>,
    N::TxEnvelope: Encodable2718,
{
    tx_request = tx_request.with_from(signer_address);
    let sendable = provider.fill(tx_request).await.map_err(FeePolicyError::FillTransaction)?;

    let SendableTx::Envelope(envelope) = sendable else {
        return Err(FeePolicyError::NotEnvelope);
    };

    Ok(Bytes::from(envelope.encoded_2718()))
}
