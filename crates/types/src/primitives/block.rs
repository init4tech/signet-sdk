//! Signet block primitives.
//!
//! These types wrap alloy consensus types to provide a simplified block
//! representation for the signet rollup. Unlike Ethereum blocks, signet
//! blocks have no ommers or withdrawals.

use super::header::SignetHeaderV1;
use alloy::{
    consensus::crypto::RecoveryError,
    consensus::{
        transaction::{Recovered, SignerRecoverable},
        Block as AlloyBlock, BlockHeader, EthereumTxEnvelope, EthereumTypedTransaction, Header,
        TxEip4844,
    },
    primitives::{Address, BlockNumber, Bloom, Bytes, B256, B64, U256},
};

/// Signet sealed block type.
///
/// Parameterized on the transaction type `T`:
/// - `SealedBlock<TransactionSigned>` — a block with signed transactions
/// - `SealedBlock<Recovered<TransactionSigned>>` — a block with sender-recovered
///   transactions (see [`RecoveredBlock`])
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedBlock<T = TransactionSigned> {
    /// The validated signet header.
    pub header: SignetHeaderV1,
    /// The transactions in the block.
    pub transactions: Vec<T>,
}

impl<T> SealedBlock<T> {
    /// Create a new sealed block.
    pub const fn new(header: SignetHeaderV1, transactions: Vec<T>) -> Self {
        Self { header, transactions }
    }

    /// Create a new empty sealed block for testing.
    #[doc(hidden)]
    pub fn blank_for_testing() -> Self {
        let v1 = SignetHeaderV1::try_from(Header::default()).expect("default header is valid V1");
        Self { header: v1, transactions: Vec::new() }
    }

    /// Create a new empty sealed block with the given V1 header for testing.
    #[doc(hidden)]
    pub const fn blank_with_header(header: SignetHeaderV1) -> Self {
        Self { header, transactions: Vec::new() }
    }

    /// Get the transactions in the block.
    pub fn transactions(&self) -> &[T] {
        &self.transactions
    }
}

impl Default for SealedBlock {
    fn default() -> Self {
        Self::blank_for_testing()
    }
}

impl SealedBlock {
    /// Recover transaction signers by verifying each signature.
    ///
    /// Returns an error if any transaction signature is invalid.
    pub fn recover(self) -> Result<RecoveredBlock, RecoveryError> {
        let transactions = self
            .transactions
            .into_iter()
            .map(|tx| {
                let sender = tx.recover_signer()?;
                Ok(Recovered::new_unchecked(tx, sender))
            })
            .collect::<Result<Vec<_>, RecoveryError>>()?;
        Ok(SealedBlock { header: self.header, transactions })
    }

    /// Zip transactions with pre-verified senders to produce a
    /// [`RecoveredBlock`].
    ///
    /// # Panics
    ///
    /// Panics if `senders.len() != self.transactions.len()`.
    pub fn recover_unchecked(self, senders: Vec<Address>) -> RecoveredBlock {
        assert_eq!(
            self.transactions.len(),
            senders.len(),
            "senders length mismatch: expected {}, got {}",
            self.transactions.len(),
            senders.len(),
        );
        let transactions = self
            .transactions
            .into_iter()
            .zip(senders)
            .map(|(tx, sender)| Recovered::new_unchecked(tx, sender))
            .collect();
        SealedBlock { header: self.header, transactions }
    }
}

/// A [`SealedBlock`] with sender-recovered transactions.
///
/// Each transaction is paired with its recovered signer address via
/// [`Recovered<TransactionSigned>`].
pub type RecoveredBlock = SealedBlock<Recovered<TransactionSigned>>;

impl Default for RecoveredBlock {
    fn default() -> Self {
        let v1 = SignetHeaderV1::try_from(Header::default()).expect("default header is valid V1");
        Self { header: v1, transactions: Vec::new() }
    }
}

impl RecoveredBlock {
    /// Iterate over the sender addresses of all transactions.
    pub fn senders(&self) -> impl ExactSizeIterator<Item = Address> + '_ {
        self.transactions.iter().map(Recovered::signer)
    }
}

impl<T> BlockHeader for SealedBlock<T> {
    fn parent_hash(&self) -> B256 {
        self.header.parent_hash()
    }
    fn ommers_hash(&self) -> B256 {
        self.header.ommers_hash()
    }
    fn beneficiary(&self) -> Address {
        self.header.beneficiary()
    }
    fn state_root(&self) -> B256 {
        self.header.state_root()
    }
    fn transactions_root(&self) -> B256 {
        self.header.transactions_root()
    }
    fn receipts_root(&self) -> B256 {
        self.header.receipts_root()
    }
    fn withdrawals_root(&self) -> Option<B256> {
        self.header.withdrawals_root()
    }
    fn logs_bloom(&self) -> Bloom {
        self.header.logs_bloom()
    }
    fn difficulty(&self) -> U256 {
        self.header.difficulty()
    }
    fn number(&self) -> BlockNumber {
        self.header.number()
    }
    fn gas_limit(&self) -> u64 {
        self.header.gas_limit()
    }
    fn gas_used(&self) -> u64 {
        self.header.gas_used()
    }
    fn timestamp(&self) -> u64 {
        self.header.timestamp()
    }
    fn mix_hash(&self) -> Option<B256> {
        self.header.mix_hash()
    }
    fn nonce(&self) -> Option<B64> {
        self.header.nonce()
    }
    fn base_fee_per_gas(&self) -> Option<u64> {
        self.header.base_fee_per_gas()
    }
    fn blob_gas_used(&self) -> Option<u64> {
        self.header.blob_gas_used()
    }
    fn excess_blob_gas(&self) -> Option<u64> {
        self.header.excess_blob_gas()
    }
    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.header.parent_beacon_block_root()
    }
    fn requests_hash(&self) -> Option<B256> {
        self.header.requests_hash()
    }
    fn extra_data(&self) -> &Bytes {
        self.header.extra_data()
    }
}

/// Typed Transaction type without a signature.
pub type Transaction = EthereumTypedTransaction<TxEip4844>;

/// Signed transaction.
pub type TransactionSigned = EthereumTxEnvelope<TxEip4844>;

/// Ethereum full block type (from alloy).
pub type Block = AlloyBlock<TransactionSigned>;
