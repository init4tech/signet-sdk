//! Signet bundle types.
use alloy::{
    consensus::{
        transaction::{Recovered, SignerRecoverable},
        TxEnvelope,
    },
    eips::{eip2718::Eip2718Result, Decodable2718},
    primitives::{Address, Bytes, TxHash, B256},
    rlp::Buf,
    rpc::types::mev::EthSendBundle,
};
use serde::{Deserialize, Serialize};
use trevm::{
    inspectors::{Layered, TimeLimit},
    revm::{inspector::NoOpInspector, Database},
    BundleError,
};

use crate::{BundleRecoverError, RecoverError, RecoveredBundle};

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
    /// Creates a new [`SignetEthBundle`] from an existing [`EthSendBundle`].
    pub const fn new(bundle: EthSendBundle, host_txs: Vec<Bytes>) -> Self {
        Self { bundle, host_txs }
    }

    /// Decomposes the [`SignetEthBundle`] into its parts.
    pub fn into_parts(self) -> (EthSendBundle, Vec<Bytes>) {
        (self.bundle, self.host_txs)
    }

    /// Returns the transactions in this bundle.
    pub const fn txs(&self) -> &[Bytes] {
        self.bundle.txs.as_slice()
    }

    /// Returns the host transactions in this bundle.
    pub const fn host_txs(&self) -> &[Bytes] {
        self.host_txs.as_slice()
    }

    /// Get a mutable reference to the host transactions.
    pub const fn host_txs_mut(&mut self) -> &mut Vec<Bytes> {
        &mut self.host_txs
    }

    /// Return an iterator over decoded transactions in this bundle.
    pub fn decode_txs(&self) -> impl Iterator<Item = Eip2718Result<TxEnvelope>> + '_ {
        self.txs().iter().map(|tx| TxEnvelope::decode_2718(&mut tx.chunk()))
    }

    /// Return an iterator over decoded host transactions in this bundle.
    ///
    /// This may be empty if no host transactions were included.
    pub fn decode_host_txs(&self) -> impl Iterator<Item = Eip2718Result<TxEnvelope>> + '_ {
        self.host_txs.iter().map(|tx| TxEnvelope::decode_2718(&mut tx.chunk()))
    }

    /// Return an iterator over recovered transactions in this bundle. This
    /// iterator may include errors.
    pub fn recover_txs(
        &self,
    ) -> impl Iterator<Item = Result<Recovered<TxEnvelope>, BundleRecoverError>> + '_ {
        self.decode_txs().enumerate().map(|(index, res)| match res {
            Ok(tx) => {
                tx.try_into_recovered().map_err(|err| BundleRecoverError::new(err, false, index))
            }
            Err(err) => Err(BundleRecoverError::new(err, false, index)),
        })
    }

    /// Return an iterator over recovered host transactions in this bundle. This
    /// iterator may include errors.
    pub fn recover_host_txs(
        &self,
    ) -> impl Iterator<Item = Result<Recovered<TxEnvelope>, BundleRecoverError>> + '_ {
        self.decode_host_txs().enumerate().map(|(index, res)| match res {
            Ok(tx) => {
                tx.try_into_recovered().map_err(|err| BundleRecoverError::new(err, true, index))
            }
            Err(err) => Err(BundleRecoverError::new(err, true, index)),
        })
    }

    /// Create a [`RecoveredBundle`] from this bundle by decoding and recovering
    /// all transactions, taking ownership of the bundle.
    pub fn try_into_recovered(self) -> Result<RecoveredBundle, BundleRecoverError> {
        if self.txs().is_empty() {
            return Err(BundleRecoverError::new(RecoverError::EmptyBundle, false, 0));
        }

        let txs = self.recover_txs().collect::<Result<Vec<_>, _>>()?;

        let host_txs = self.recover_host_txs().collect::<Result<Vec<_>, _>>()?;

        Ok(RecoveredBundle {
            txs,
            host_txs,
            block_number: self.bundle.block_number,
            min_timestamp: self.bundle.min_timestamp,
            max_timestamp: self.bundle.max_timestamp,
            reverting_tx_hashes: self.bundle.reverting_tx_hashes,
            replacement_uuid: self.bundle.replacement_uuid,
            dropping_tx_hashes: self.bundle.dropping_tx_hashes,
            refund_percent: self.bundle.refund_percent,
            refund_recipient: self.bundle.refund_recipient,
            refund_tx_hashes: self.bundle.refund_tx_hashes,
            extra_fields: self.bundle.extra_fields,
        })
    }

    /// Create a [`RecoveredBundle`] from this bundle by decoding and recovering
    /// all transactions, cloning other fields as necessary.
    pub fn try_to_recovered(&self) -> Result<RecoveredBundle, BundleRecoverError> {
        self.clone().try_into_recovered()
    }

    /// Return an iterator over the signers of the transactions in this bundle.
    /// The iterator yields `Option<(TxHash, Address)>` for each transaction,
    /// where `None` indicates that the signer could not be recovered.
    ///
    /// Computing this may be expensive, as it requires decoding and recovering
    /// the signer for each transaction. It is recommended to memoize the
    /// results
    pub fn signers(&self) -> impl Iterator<Item = Option<(TxHash, Address)>> + '_ {
        self.txs().iter().map(|tx| {
            TxEnvelope::decode_2718(&mut tx.chunk())
                .ok()
                .and_then(|envelope| envelope.recover_signer().ok().map(|s| (*envelope.hash(), s)))
        })
    }

    /// Return an iterator over the signers of the transactions in this bundle,
    /// skipping any transactions where the signer could not be recovered.
    pub fn signers_lossy(&self) -> impl Iterator<Item = (TxHash, Address)> + '_ {
        self.signers().flatten()
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
    pub const fn reverting_tx_hashes(&self) -> &[B256] {
        self.bundle.reverting_tx_hashes.as_slice()
    }

    /// Returns the replacement uuid for this bundle.
    pub const fn replacement_uuid(&self) -> Option<&str> {
        let Some(uuid) = &self.bundle.replacement_uuid else { return None };

        Some(uuid.as_str())
    }

    /// Checks if the bundle is valid at a given timestamp.
    pub fn is_valid_at_timestamp(&self, timestamp: u64) -> bool {
        let min_timestamp = self.min_timestamp().unwrap_or(0);
        let max_timestamp = self.max_timestamp().unwrap_or(u64::MAX);

        (min_timestamp..=max_timestamp).contains(&timestamp)
    }

    /// Checks if the bundle is valid at a given block number.
    pub const fn is_valid_at_block_number(&self, block_number: u64) -> bool {
        self.bundle.block_number == block_number
    }

    /// Returns the dropping tx hashes for this bundle.
    ///
    /// Dropping tx hashes specify transactions that may be dropped from the
    /// bundle if they fail, without causing the entire bundle to fail.
    pub const fn dropping_tx_hashes(&self) -> &[B256] {
        self.bundle.dropping_tx_hashes.as_slice()
    }

    /// Returns the refund percentage for this bundle.
    ///
    /// The refund percent specifies what percentage of the bundle's profit
    /// should be returned to the [`refund_recipient`]. Valid values are 0-100.
    /// If not specified, the builder may use a default (typically 90%).
    ///
    /// This follows the [Flashbots refund semantics].
    ///
    /// [`refund_recipient`]: Self::refund_recipient
    /// [Flashbots refund semantics]: https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint
    pub const fn refund_percent(&self) -> Option<u8> {
        self.bundle.refund_percent
    }

    /// Returns the refund recipient address for this bundle.
    ///
    /// The refund recipient is the address that receives the refund percentage
    /// of the bundle's profit. If not specified, the builder typically defaults
    /// to the first transaction's origin address.
    ///
    /// This follows the [Flashbots refund semantics].
    ///
    /// [Flashbots refund semantics]: https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint
    pub const fn refund_recipient(&self) -> Option<Address> {
        self.bundle.refund_recipient
    }

    /// Returns the refund tx hashes for this bundle.
    ///
    /// The refund tx hashes specify which transactions in the bundle should be
    /// used to calculate the refund amount. If empty, all transactions in the
    /// bundle are used.
    ///
    /// This follows the [Flashbots refund semantics].
    ///
    /// [Flashbots refund semantics]: https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint
    pub const fn refund_tx_hashes(&self) -> &[B256] {
        self.bundle.refund_tx_hashes.as_slice()
    }

    /// Validates that the refund percentage is within valid bounds (0-100).
    ///
    /// Returns `true` if the refund percent is valid or not specified.
    /// Returns `false` if the refund percent exceeds 100.
    pub const fn is_valid_refund_percent(&self) -> bool {
        match self.bundle.refund_percent {
            Some(percent) => percent <= 100,
            None => true,
        }
    }

    /// Validates that all refund tx hashes exist in the bundle's transactions.
    ///
    /// This requires decoding transactions, which may be expensive. Returns
    /// `true` if all refund tx hashes are found in the bundle, or if no refund
    /// tx hashes are specified.
    pub fn is_valid_refund_tx_hashes(&self) -> bool {
        let refund_hashes = self.refund_tx_hashes();
        if refund_hashes.is_empty() {
            return true;
        }

        let bundle_hashes: Vec<_> =
            self.decode_txs().filter_map(|res| res.ok().map(|tx| *tx.hash())).collect();

        refund_hashes.iter().all(|hash| bundle_hashes.contains(hash))
    }

    /// Validates all refund-related fields in the bundle.
    ///
    /// Checks:
    /// - Refund percentage is 0-100 (if specified)
    /// - All refund tx hashes exist in the bundle (if specified)
    ///
    /// Returns `true` if all refund fields are valid.
    pub fn is_valid_refunds(&self) -> bool {
        self.is_valid_refund_percent() && self.is_valid_refund_tx_hashes()
    }

    /// Decode and validate the transactions in the bundle.
    pub fn decode_and_validate_txs<Db: Database>(
        &self,
    ) -> Result<Vec<TxEnvelope>, BundleError<Db>> {
        // Decode and validate the transactions in the bundle
        let txs = self
            .decode_txs()
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
        self.decode_host_txs()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| BundleError::TransactionDecodingError(err))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn send_bundle_ser_roundtrip() {
        let bundle = SignetEthBundle::new(
            EthSendBundle {
                txs: vec![b"tx1".into(), b"tx2".into()],
                block_number: 1,
                min_timestamp: Some(2),
                max_timestamp: Some(3),
                reverting_tx_hashes: vec![B256::repeat_byte(4), B256::repeat_byte(5)],
                replacement_uuid: Some("uuid".to_owned()),
                ..Default::default()
            },
            vec![b"host_tx1".into(), b"host_tx2".into()],
        );

        let serialized = serde_json::to_string(&bundle).unwrap();
        let deserialized: SignetEthBundle = serde_json::from_str(&serialized).unwrap();

        assert_eq!(bundle, deserialized);
    }

    #[test]
    fn send_bundle_ser_roundtrip_no_host_no_fills() {
        let bundle = SignetEthBundle::new(
            EthSendBundle {
                txs: vec![b"tx1".into(), b"tx2".into()],
                block_number: 1,
                min_timestamp: Some(2),
                max_timestamp: Some(3),
                reverting_tx_hashes: vec![B256::repeat_byte(4), B256::repeat_byte(5)],
                replacement_uuid: Some("uuid".to_owned()),
                ..Default::default()
            },
            vec![],
        );

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

    #[test]
    fn test_refund_percent_validation() {
        // Valid: no refund percent specified
        let bundle = SignetEthBundle::new(
            EthSendBundle {
                txs: vec![b"tx1".into()],
                block_number: 1,
                refund_percent: None,
                ..Default::default()
            },
            vec![],
        );
        assert!(bundle.is_valid_refund_percent());

        // Valid: 0%
        let bundle = SignetEthBundle::new(
            EthSendBundle {
                txs: vec![b"tx1".into()],
                block_number: 1,
                refund_percent: Some(0),
                ..Default::default()
            },
            vec![],
        );
        assert!(bundle.is_valid_refund_percent());

        // Valid: 100%
        let bundle = SignetEthBundle::new(
            EthSendBundle {
                txs: vec![b"tx1".into()],
                block_number: 1,
                refund_percent: Some(100),
                ..Default::default()
            },
            vec![],
        );
        assert!(bundle.is_valid_refund_percent());

        // Valid: typical 90%
        let bundle = SignetEthBundle::new(
            EthSendBundle {
                txs: vec![b"tx1".into()],
                block_number: 1,
                refund_percent: Some(90),
                ..Default::default()
            },
            vec![],
        );
        assert!(bundle.is_valid_refund_percent());
    }

    #[test]
    fn test_refund_fields_accessors() {
        let recipient = Address::repeat_byte(0x42);
        let refund_hash = B256::repeat_byte(0xab);

        let bundle = SignetEthBundle::new(
            EthSendBundle {
                txs: vec![b"tx1".into()],
                block_number: 1,
                refund_percent: Some(85),
                refund_recipient: Some(recipient),
                refund_tx_hashes: vec![refund_hash],
                ..Default::default()
            },
            vec![],
        );

        assert_eq!(bundle.refund_percent(), Some(85));
        assert_eq!(bundle.refund_recipient(), Some(recipient));
        assert_eq!(bundle.refund_tx_hashes(), &[refund_hash]);
    }

    #[test]
    fn test_refund_fields_serialization() {
        let recipient = Address::repeat_byte(0x22);
        let refund_hash = B256::repeat_byte(0x33);

        let bundle = SignetEthBundle::new(
            EthSendBundle {
                txs: vec![b"tx1".into()],
                block_number: 12345678,
                refund_percent: Some(90),
                refund_recipient: Some(recipient),
                refund_tx_hashes: vec![refund_hash],
                ..Default::default()
            },
            vec![],
        );

        let serialized = serde_json::to_string(&bundle).unwrap();

        // Verify refund fields are present in serialized output
        assert!(serialized.contains("refundPercent"));
        assert!(serialized.contains("refundRecipient"));
        assert!(serialized.contains("refundTxHashes"));

        // Verify roundtrip
        let deserialized: SignetEthBundle = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.refund_percent(), Some(90));
        assert_eq!(deserialized.refund_recipient(), Some(recipient));
        assert_eq!(deserialized.refund_tx_hashes(), &[refund_hash]);
    }

    #[test]
    fn test_deser_bundle_with_refund_fields() {
        let json = r#"{
            "txs": ["0x747831"],
            "blockNumber": "0xbc614e",
            "refundPercent": 90,
            "refundRecipient": "0x2222222222222222222222222222222222222222",
            "refundTxHashes": ["0x3333333333333333333333333333333333333333333333333333333333333333"]
        }"#;

        let deserialized: SignetEthBundle = serde_json::from_str(json).unwrap();

        assert_eq!(deserialized.refund_percent(), Some(90));
        assert_eq!(deserialized.refund_recipient(), Some(Address::repeat_byte(0x22)));
        assert_eq!(deserialized.refund_tx_hashes(), &[B256::repeat_byte(0x33)]);
    }

    /// Generate test vectors for TypeScript SDK.
    ///
    /// Run with: `cargo t -p signet-bundle -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn generate_eth_bundle_vectors() {
        use alloy::primitives::Address;

        let vectors = vec![
            (
                "minimal",
                SignetEthBundle::new(
                    EthSendBundle {
                        txs: vec![b"\x02\xf8test_tx_1".into()],
                        block_number: 12345678,
                        ..Default::default()
                    },
                    vec![],
                ),
            ),
            (
                "with_timestamps",
                SignetEthBundle::new(
                    EthSendBundle {
                        txs: vec![b"\x02\xf8test_tx_1".into()],
                        block_number: 12345678,
                        min_timestamp: Some(1700000000),
                        max_timestamp: Some(1700003600),
                        ..Default::default()
                    },
                    vec![],
                ),
            ),
            (
                "with_reverting_hashes",
                SignetEthBundle::new(
                    EthSendBundle {
                        txs: vec![b"\x02\xf8test_tx_1".into(), b"\x02\xf8test_tx_2".into()],
                        block_number: 12345678,
                        reverting_tx_hashes: vec![B256::repeat_byte(0xab), B256::repeat_byte(0xcd)],
                        ..Default::default()
                    },
                    vec![],
                ),
            ),
            (
                "with_host_txs",
                SignetEthBundle::new(
                    EthSendBundle {
                        txs: vec![b"\x02\xf8rollup_tx".into()],
                        block_number: 12345678,
                        ..Default::default()
                    },
                    vec![b"\x02\xf8host_tx_1".into(), b"\x02\xf8host_tx_2".into()],
                ),
            ),
            (
                "full_bundle",
                SignetEthBundle::new(
                    EthSendBundle {
                        txs: vec![b"\x02\xf8tx_1".into(), b"\x02\xf8tx_2".into()],
                        block_number: 12345678,
                        min_timestamp: Some(1700000000),
                        max_timestamp: Some(1700003600),
                        reverting_tx_hashes: vec![B256::repeat_byte(0xef)],
                        dropping_tx_hashes: vec![B256::repeat_byte(0x11)],
                        refund_percent: Some(90),
                        refund_recipient: Some(Address::repeat_byte(0x22)),
                        refund_tx_hashes: vec![B256::repeat_byte(0x33)],
                        ..Default::default()
                    },
                    vec![b"\x02\xf8host_tx".into()],
                ),
            ),
            (
                "replacement_bundle",
                SignetEthBundle::new(
                    EthSendBundle {
                        txs: vec![b"\x02\xf8replacement_tx".into()],
                        block_number: 12345678,
                        replacement_uuid: Some("550e8400-e29b-41d4-a716-446655440000".to_owned()),
                        ..Default::default()
                    },
                    vec![],
                ),
            ),
            // Refund-specific test vectors
            (
                "refund_basic",
                SignetEthBundle::new(
                    EthSendBundle {
                        txs: vec![b"\x02\xf8arb_tx".into()],
                        block_number: 12345678,
                        refund_percent: Some(90),
                        refund_recipient: Some(Address::repeat_byte(0x42)),
                        ..Default::default()
                    },
                    vec![],
                ),
            ),
            (
                "refund_with_tx_hashes",
                SignetEthBundle::new(
                    EthSendBundle {
                        txs: vec![b"\x02\xf8tx_1".into(), b"\x02\xf8tx_2".into()],
                        block_number: 12345678,
                        refund_percent: Some(50),
                        refund_recipient: Some(Address::repeat_byte(0xaa)),
                        refund_tx_hashes: vec![B256::repeat_byte(0xbb)],
                        ..Default::default()
                    },
                    vec![],
                ),
            ),
            (
                "refund_zero_percent",
                SignetEthBundle::new(
                    EthSendBundle {
                        txs: vec![b"\x02\xf8tx".into()],
                        block_number: 12345678,
                        refund_percent: Some(0),
                        ..Default::default()
                    },
                    vec![],
                ),
            ),
        ];

        let output: Vec<_> = vectors
            .into_iter()
            .map(|(name, bundle)| {
                serde_json::json!({
                    "name": name,
                    "bundle": bundle,
                })
            })
            .collect();

        println!("// SignetEthBundle vectors\n{}", serde_json::to_string_pretty(&output).unwrap());
    }
}
