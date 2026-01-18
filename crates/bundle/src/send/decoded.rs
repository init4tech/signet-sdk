use alloy::{
    consensus::{transaction::Recovered, Transaction, TxEnvelope},
    primitives::{Address, TxHash, U256},
    serde::OtherFields,
};

/// Transaction requirement info for a single transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxRequirement {
    /// Signer address
    pub signer: Address,
    /// Nonce
    pub nonce: u64,
    /// Max spend (max_fee_per_gas * gas_limit) + value
    pub balance: U256,
}

/// Version of [`SignetEthBundle`] with decoded transactions.
///
/// [`SignetEthBundle`]: crate::send::bundle::SignetEthBundle
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredBundle {
    /// Transactions in this bundle.
    pub(crate) txs: Vec<Recovered<TxEnvelope>>,

    /// Host transactions to be included in the host bundle.
    pub(crate) host_txs: Vec<Recovered<TxEnvelope>>,

    /// Block number for which this bundle is valid
    pub(crate) block_number: u64,

    /// unix timestamp when this bundle becomes active
    pub(crate) min_timestamp: Option<u64>,

    /// unix timestamp how long this bundle stays valid
    pub(crate) max_timestamp: Option<u64>,

    /// list of hashes of possibly reverting txs
    pub(crate) reverting_tx_hashes: Vec<TxHash>,

    /// UUID that can be used to cancel/replace this bundle
    pub(crate) replacement_uuid: Option<String>,

    /// A list of tx hashes that are allowed to be discarded
    pub(crate) dropping_tx_hashes: Vec<TxHash>,

    /// The percent that should be refunded to refund recipient
    pub(crate) refund_percent: Option<u8>,

    /// The address that receives the refund
    pub(crate) refund_recipient: Option<Address>,

    /// A list of tx hashes used to determine the refund
    pub(crate) refund_tx_hashes: Vec<TxHash>,

    /// Additional fields that are specific to the builder
    pub(crate) extra_fields: OtherFields,
}

impl RecoveredBundle {
    /// Instantiator. Generally recommend instantiating via conversion from
    /// [`SignetEthBundle`] via [`SignetEthBundle::try_into_recovered`] or
    /// [`SignetEthBundle::try_to_recovered`]. This allows instantiating empty
    /// bundles, which are otherwise disallowed and is used for testing.
    ///
    /// [`SignetEthBundle`]: crate::send::bundle::SignetEthBundle
    /// [`SignetEthBundle::try_into_recovered`]: crate::send::bundle::SignetEthBundle::try_into_recovered
    /// [`SignetEthBundle::try_to_recovered`]: crate::send::bundle::SignetEthBundle::try_to_recovered
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub const fn new_unchecked(
        txs: Vec<Recovered<TxEnvelope>>,
        host_txs: Vec<Recovered<TxEnvelope>>,
        block_number: u64,
        min_timestamp: Option<u64>,
        max_timestamp: Option<u64>,
        reverting_tx_hashes: Vec<TxHash>,
        replacement_uuid: Option<String>,
        dropping_tx_hashes: Vec<TxHash>,
        refund_percent: Option<u8>,
        refund_recipient: Option<Address>,
        refund_tx_hashes: Vec<TxHash>,
        extra_fields: OtherFields,
    ) -> Self {
        Self {
            txs,
            host_txs,
            block_number,
            min_timestamp,
            max_timestamp,
            reverting_tx_hashes,
            replacement_uuid,
            dropping_tx_hashes,
            refund_percent,
            refund_recipient,
            refund_tx_hashes,
            extra_fields,
        }
    }

    /// Get the transactions.
    pub const fn txs(&self) -> &[Recovered<TxEnvelope>] {
        self.txs.as_slice()
    }

    /// Get the host transactions.
    pub const fn host_txs(&self) -> &[Recovered<TxEnvelope>] {
        self.host_txs.as_slice()
    }

    /// Get an iterator draining the transactions.
    pub fn drain_txns(&mut self) -> impl Iterator<Item = Recovered<TxEnvelope>> + '_ {
        self.txs.drain(..)
    }

    /// Get an iterator draining the host transactions.
    pub fn drain_host_txns(&mut self) -> impl Iterator<Item = Recovered<TxEnvelope>> + '_ {
        self.host_txs.drain(..)
    }

    /// Get an iterator over the transaction requirements:
    /// - signer address
    /// - nonce
    /// - min_balance ((max_fee_per_gas * gas_limit) + value)
    pub fn tx_reqs(&self) -> impl Iterator<Item = TxRequirement> + '_ {
        self.txs.iter().map(|tx| {
            let balance = U256::from(tx.max_fee_per_gas() * tx.gas_limit() as u128) + tx.value();
            TxRequirement { signer: tx.signer(), nonce: tx.nonce(), balance }
        })
    }

    /// Get an iterator over the host transaction requirements:
    /// - signer address
    /// - nonce
    /// - min_balance ((max_fee_per_gas * gas_limit) + value)
    pub fn host_tx_reqs(&self) -> impl Iterator<Item = TxRequirement> + '_ {
        self.host_txs.iter().map(|tx| {
            let balance = U256::from(tx.max_fee_per_gas() * tx.gas_limit() as u128) + tx.value();
            TxRequirement { signer: tx.signer(), nonce: tx.nonce(), balance }
        })
    }

    /// Getter for block_number, a standard bundle prop.
    pub const fn block_number(&self) -> u64 {
        self.block_number
    }

    /// Get the valid timestamp range for this bundle.
    pub const fn valid_timestamp_range(&self) -> std::ops::RangeInclusive<u64> {
        let min = if let Some(min) = self.min_timestamp { min } else { 0 };
        let max = if let Some(max) = self.max_timestamp { max } else { u64::MAX };
        min..=max
    }

    /// Getter for min_timestamp, a standard bundle prop.
    pub const fn raw_min_timestamp(&self) -> Option<u64> {
        self.min_timestamp
    }

    /// Getter for [`Self::raw_min_timestamp`], with default of 0.
    pub const fn min_timestamp(&self) -> u64 {
        if let Some(min) = self.min_timestamp {
            min
        } else {
            0
        }
    }

    /// Getter for max_timestamp, a standard bundle prop.
    pub const fn raw_max_timestamp(&self) -> Option<u64> {
        self.max_timestamp
    }

    /// Getter for [`Self::raw_max_timestamp`], with default of `u64::MAX`.
    pub const fn max_timestamp(&self) -> u64 {
        if let Some(max) = self.max_timestamp {
            max
        } else {
            u64::MAX
        }
    }

    /// Getter for reverting_tx_hashes, a standard bundle prop.
    pub const fn reverting_tx_hashes(&self) -> &[TxHash] {
        self.reverting_tx_hashes.as_slice()
    }

    /// Getter for replacement_uuid, a standard bundle prop.
    pub const fn replacement_uuid(&self) -> Option<&str> {
        if let Some(ref uuid) = self.replacement_uuid {
            Some(uuid.as_str())
        } else {
            None
        }
    }

    /// Getter for dropping_tx_hashes, a standard bundle prop.
    pub const fn dropping_tx_hashes(&self) -> &[TxHash] {
        self.dropping_tx_hashes.as_slice()
    }

    /// Getter for refund_percent, a standard bundle prop.
    pub const fn refund_percent(&self) -> Option<u8> {
        self.refund_percent
    }

    /// Getter for refund_recipient, a standard bundle prop.
    pub const fn refund_recipient(&self) -> Option<Address> {
        self.refund_recipient
    }

    /// Getter for refund_tx_hashes, a standard bundle prop.
    pub const fn refund_tx_hashes(&self) -> &[TxHash] {
        self.refund_tx_hashes.as_slice()
    }

    /// Getter for extra_fields, a standard bundle prop.
    pub const fn extra_fields(&self) -> &OtherFields {
        &self.extra_fields
    }

    /// Checks if the bundle is valid at a given timestamp.
    pub fn is_valid_at_timestamp(&self, timestamp: u64) -> bool {
        let min_timestamp = self.min_timestamp.unwrap_or(0);
        let max_timestamp = self.max_timestamp.unwrap_or(u64::MAX);

        (min_timestamp..=max_timestamp).contains(&timestamp)
    }

    /// Checks if the bundle is valid at a given block number.
    pub const fn is_valid_at_block_number(&self, block_number: u64) -> bool {
        self.block_number == block_number
    }
}
