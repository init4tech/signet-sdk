//! Signet bundle types.
use crate::send::SignetEthBundleError;
use alloy::{
    consensus::TxEnvelope,
    eips::Decodable2718,
    network::Network,
    primitives::{Bytes, B256},
    providers::Provider,
    rlp::Buf,
    rpc::types::mev::EthSendBundle,
};
use serde::{Deserialize, Serialize};
use signet_types::{SignedFill, SignedPermitError};
use signet_zenith::HostOrders::HostOrdersInstance;
use trevm::{revm::Database, BundleError};

/// Bundle of transactions for `signet_sendBundle`.
///
/// The Signet bundle contains the following:
///
/// - A standard [`EthSendBundle`] with the transactions to simulate.
/// - A signed permit2 fill to be applied on the Host chain with the bundle.
///
/// This is based on the flashbots `eth_sendBundle` bundle. See [their docs].
///
/// [their docs]: https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignetEthBundle {
    /// The bundle of transactions to simulate. Same structure as a Flashbots [`EthSendBundle`] bundle.
    #[serde(flatten)]
    pub bundle: EthSendBundle,
    /// Host fills to be applied with the bundle, represented as a signed
    /// permit2 fill.
    #[serde(default)]
    pub host_fills: Option<SignedFill>,
}

impl SignetEthBundle {
    /// Returns the transactions in this bundle.
    #[allow(clippy::missing_const_for_fn)] // false positive
    pub fn txs(&self) -> &[Bytes] {
        &self.bundle.txs
    }

    /// Returns the block number for this bundle.
    pub const fn block_number(&self) -> u64 {
        self.bundle.block_number
    }

    /// Returns the minimum timestamp for this bundle.
    pub const fn min_timestamp(&self) -> Option<u64> {
        self.bundle.min_timestamp
    }

    /// Returns the maximum timestamp for this bundle.
    pub const fn max_timestamp(&self) -> Option<u64> {
        self.bundle.max_timestamp
    }

    /// Returns the reverting tx hashes for this bundle.
    pub fn reverting_tx_hashes(&self) -> &[B256] {
        self.bundle.reverting_tx_hashes.as_slice()
    }

    /// Returns the replacement uuid for this bundle.
    pub fn replacement_uuid(&self) -> Option<&str> {
        self.bundle.replacement_uuid.as_deref()
    }

    /// Checks if the bundle is valid at a given timestamp.
    pub fn is_valid_at_timestamp(&self, timestamp: u64) -> bool {
        let min_timestamp = self.bundle.min_timestamp.unwrap_or(0);
        let max_timestamp = self.bundle.max_timestamp.unwrap_or(u64::MAX);
        timestamp >= min_timestamp && timestamp <= max_timestamp
    }

    /// Checks if the bundle is valid at a given block number.
    pub const fn is_valid_at_block_number(&self, block_number: u64) -> bool {
        self.bundle.block_number == block_number
    }

    /// Decode and validate the transactions in the bundle.
    pub fn decode_and_validate_txs<Db: Database>(
        &self,
    ) -> Result<Vec<TxEnvelope>, BundleError<Db>> {
        // Decode and validate the transactions in the bundle
        let txs = self
            .txs()
            .iter()
            .map(|tx| TxEnvelope::decode_2718(&mut tx.chunk()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| BundleError::TransactionDecodingError(err))?;

        if txs.iter().any(|tx| tx.is_eip4844()) {
            return Err(BundleError::UnsupportedTransactionType);
        }

        Ok(txs)
    }

    /// Check that this can be syntactically used as a fill.
    pub fn validate_fills_offchain(&self, timestamp: u64) -> Result<(), SignedPermitError> {
        if let Some(host_fills) = &self.host_fills {
            host_fills.validate(timestamp)
        } else {
            Ok(())
        }
    }

    /// Check that this fill is valid on-chain as of the current block. This
    /// checks that the tokens can actually be transferred.
    ///
    /// # WARNING:
    ///
    /// This function will send an RPC request to the provider containing the
    /// fills. It MUST NOT be used with an untrusted provider.
    pub async fn alloy_validate_fills_onchain<Db, P, N>(
        &self,
        orders: HostOrdersInstance<P, N>,
    ) -> Result<(), SignetEthBundleError<Db>>
    where
        Db: Database,
        P: Provider<N>,
        N: Network,
    {
        if let Some(host_fills) = self.host_fills.clone() {
            orders.try_fill(host_fills.outputs, host_fills.permit).await.map_err(Into::into)
        } else {
            Ok(())
        }
    }
}

/// Response for `signet_sendBundle`.
///
/// This is based on the flashbots `eth_sendBundle` response. See [their docs].
///
/// [their docs]: https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignetEthBundleResponse {
    /// The bundle hash of the sent bundle.
    ///
    /// This is calculated as keccak256(tx_hashes) where tx_hashes are the
    /// concatenated transaction hashes.
    pub bundle_hash: B256,
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy::primitives::{Address, U256};
    use signet_zenith::HostOrders::{
        Output, Permit2Batch, PermitBatchTransferFrom, TokenPermissions,
    };

    #[test]
    fn send_bundle_ser_roundtrip() {
        let bundle = SignetEthBundle {
            bundle: EthSendBundle {
                txs: vec![b"tx1".into(), b"tx2".into()],
                block_number: 1,
                min_timestamp: Some(2),
                max_timestamp: Some(3),
                reverting_tx_hashes: vec![B256::repeat_byte(4), B256::repeat_byte(5)],
                replacement_uuid: Some("uuid".to_owned()),
                ..Default::default()
            },
            host_fills: Some(SignedFill {
                permit: Permit2Batch {
                    permit: PermitBatchTransferFrom {
                        permitted: vec![TokenPermissions {
                            token: Address::repeat_byte(66),
                            amount: U256::from(17),
                        }],
                        nonce: U256::from(18),
                        deadline: U256::from(19),
                    },
                    owner: Address::repeat_byte(77),
                    signature: Bytes::from(b"abcd"),
                },
                outputs: vec![Output {
                    token: Address::repeat_byte(88),
                    amount: U256::from(20),
                    recipient: Address::repeat_byte(99),
                    chainId: 100,
                }],
            }),
        };

        let serialized = serde_json::to_string(&bundle).unwrap();
        let deserialized: SignetEthBundle = serde_json::from_str(&serialized).unwrap();

        assert_eq!(bundle, deserialized);
    }

    #[test]
    fn send_bundle_resp_ser_roundtrip() {
        let resp = SignetEthBundleResponse { bundle_hash: B256::repeat_byte(1) };

        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: SignetEthBundleResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(resp, deserialized);
    }
}
