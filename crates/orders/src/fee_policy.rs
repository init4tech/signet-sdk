use crate::{BundleSubmitter, FillSubmitter, OrdersAndFills, TxBuilder};
use alloy::primitives::Address;
use alloy::{
    eips::eip2718::Encodable2718,
    network::{Ethereum, Network, TransactionBuilder},
    primitives::Bytes,
    providers::{fillers::FillerControlFlow, SendableTx},
    rpc::types::mev::EthSendBundle,
    transports::{RpcError, TransportErrorKind},
};
use futures_util::{stream, StreamExt, TryStreamExt};
use signet_bundle::SignetEthBundle;
use signet_constants::SignetSystemConstants;
#[cfg(doc)]
use signet_types::SignedFill;
use tracing::{error, instrument};

/// Errors returned by [`FeePolicySubmitter`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FeePolicyError {
    /// No fills provided for submission.
    #[error("no fills provided for submission")]
    NoFills,
    /// RPC call failed.
    #[error("RPC error: {0}")]
    Rpc(#[source] RpcError<TransportErrorKind>),
    /// Transaction is incomplete (missing required properties).
    #[error("transaction missing required properties: {0:?}")]
    IncompleteTransaction(Vec<(&'static str, Vec<&'static str>)>),
    /// Bundle submission failed.
    #[error("failed to submit bundle: {0}")]
    Submission(#[source] Box<dyn core::error::Error + Send + Sync>),
}

impl From<FillerControlFlow> for FeePolicyError {
    fn from(filler_control_flow: FillerControlFlow) -> Self {
        match filler_control_flow {
            FillerControlFlow::Missing(missing) => Self::IncompleteTransaction(missing),
            FillerControlFlow::Finished | FillerControlFlow::Ready => {
                error!("fill returned Builder but status is {filler_control_flow:?}");
                Self::IncompleteTransaction(Vec::new())
            }
        }
    }
}

/// A [`FillSubmitter`] that wraps a [`BundleSubmitter`] and handles fee policy.
///
/// This submitter converts [`SignedFill`]s into transactions with appropriate gas pricing, builds
/// a [`SignetEthBundle`], and submits via the wrapped submitter.
///
/// The providers must be configured with appropriate fillers for gas, nonce, chain ID, and wallet
/// signing (e.g., via `ProviderBuilder::with_gas_estimation()` and `ProviderBuilder::wallet()`).
/// Note that the provider's nonce filler must correctly increment nonces across all transactions
/// built within a single [`FillSubmitter::submit_fills`] call.
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
        let fill_iter = fills
            .get(&self.constants.ru_chain_id())
            .map(|fill| fill.to_fill_tx(self.constants.ru_orders()))
            .into_iter();
        let order_iter = orders
            .iter()
            .map(|order| order.to_initiate_tx(signer_address, self.constants.ru_orders()));
        let rollup_txs: Vec<Bytes> = stream::iter(fill_iter.chain(order_iter))
            .then(|tx_request| sign_and_encode_tx(&self.ru_provider, tx_request, signer_address))
            .try_collect()
            .await?;

        // Build host transaction request: fill only (if present)
        let host_txs = match fills.get(&self.constants.host_chain_id()) {
            Some(fill) => {
                let tx_request = fill.to_fill_tx(self.constants.host_orders());
                vec![sign_and_encode_tx(&self.host_provider, tx_request, signer_address).await?]
            }
            None => vec![],
        };

        // NOTE: We could retrieve a header up front, then use number+1. We could also check that
        // the timestamp in the orders are valid for current.timestamp + calculator.slot_duration.
        let target_block =
            self.ru_provider.get_block_number().await.map_err(FeePolicyError::Rpc)? + 1;

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
    let sendable = provider.fill(tx_request).await.map_err(FeePolicyError::Rpc)?;

    let envelope = match sendable {
        SendableTx::Envelope(envelope) => envelope,
        SendableTx::Builder(tx) => {
            return Err(FeePolicyError::from(provider.status(&tx)));
        }
    };

    Ok(Bytes::from(envelope.encoded_2718()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_policy_error_display_no_fills() {
        let err = FeePolicyError::NoFills;
        assert_eq!(err.to_string(), "no fills provided for submission");
    }

    #[test]
    fn fee_policy_error_display_incomplete_transaction() {
        let err = FeePolicyError::IncompleteTransaction(vec![("gas", vec!["gas_limit"])]);
        assert!(err.to_string().contains("missing required properties"));
    }

    #[test]
    fn fee_policy_error_from_filler_control_flow_missing() {
        let missing = vec![("nonce", vec!["nonce"])];
        let control_flow = FillerControlFlow::Missing(missing.clone());
        let err = FeePolicyError::from(control_flow);
        match err {
            FeePolicyError::IncompleteTransaction(m) => assert_eq!(m, missing),
            _ => panic!("expected IncompleteTransaction"),
        }
    }

    #[test]
    fn fee_policy_error_from_filler_control_flow_finished() {
        let control_flow = FillerControlFlow::Finished;
        let err = FeePolicyError::from(control_flow);
        match err {
            FeePolicyError::IncompleteTransaction(m) => assert!(m.is_empty()),
            _ => panic!("expected IncompleteTransaction"),
        }
    }

    #[test]
    fn fee_policy_error_from_filler_control_flow_ready() {
        let control_flow = FillerControlFlow::Ready;
        let err = FeePolicyError::from(control_flow);
        match err {
            FeePolicyError::IncompleteTransaction(m) => assert!(m.is_empty()),
            _ => panic!("expected IncompleteTransaction"),
        }
    }
}
