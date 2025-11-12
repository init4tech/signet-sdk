//! Signet bundle types.
use alloy::{
    consensus::TxEnvelope,
    eips::Decodable2718,
    primitives::{Bytes, B256},
    rlp::Buf,
    rpc::types::mev::EthSendBundle,
};
use serde::{Deserialize, Serialize};
use trevm::{
    inspectors::{Layered, TimeLimit},
    revm::{inspector::NoOpInspector, Database},
    BundleError,
};

/// The inspector type required by the Signet bundle driver.
pub type BundleInspector<I = NoOpInspector> = Layered<TimeLimit, I>;

/// Bundle of transactions for `signet_sendBundle`.
///
/// The Signet bundle contains the following:
///
/// - A standard [`EthSendBundle`] with the transactions to simulate.
/// - Host transactions to be included in the host bundle.
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

    /// Host transactions to be included in the host bundle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub host_txs: Vec<Bytes>,
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

    /// Decode and validate the host transactions in the bundle.
    pub fn decode_and_validate_host_txs<Db: Database>(
        &self,
    ) -> Result<Vec<TxEnvelope>, BundleError<Db>> {
        // Decode and validate the host transactions in the bundle
        let txs = self
            .host_txs
            .iter()
            .map(|tx| TxEnvelope::decode_2718(&mut tx.chunk()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| BundleError::TransactionDecodingError(err))?;

        Ok(txs)
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
            host_txs: vec![b"host_tx1".into(), b"host_tx2".into()],
        };

        let serialized = serde_json::to_string(&bundle).unwrap();
        let deserialized: SignetEthBundle = serde_json::from_str(&serialized).unwrap();

        assert_eq!(bundle, deserialized);
    }

    #[test]
    fn send_bundle_ser_roundtrip_no_host_no_fills() {
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
            host_txs: vec![],
        };

        let serialized = serde_json::to_string(&bundle).unwrap();
        let deserialized: SignetEthBundle = serde_json::from_str(&serialized).unwrap();

        assert_eq!(bundle, deserialized);
    }

    #[test]
    fn test_deser_bundle_no_host_no_fills() {
        let json = r#"
        {"txs":["0x747831","0x747832"],"blockNumber":"0x1","minTimestamp":2,"maxTimestamp":3,"revertingTxHashes":["0x0404040404040404040404040404040404040404040404040404040404040404","0x0505050505050505050505050505050505050505050505050505050505050505"],"replacementUuid":"uuid"}"#;

        let deserialized: SignetEthBundle = serde_json::from_str(json).unwrap();

        assert!(deserialized.host_txs.is_empty());
    }
}
