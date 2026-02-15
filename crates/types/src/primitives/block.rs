//! Signet block primitives.
//!
//! These types wrap alloy consensus types to provide a simplified block
//! representation for the signet rollup. Unlike Ethereum blocks, signet
//! blocks have no ommers or withdrawals.

use alloy::{
    consensus::{
        transaction::Recovered, Block as AlloyBlock, BlockHeader, EthereumTxEnvelope,
        EthereumTypedTransaction, Header, TxEip4844,
    },
    primitives::{Address, BlockNumber, Bloom, Bytes, Sealed, B256, B64, U256},
};

/// Delegates all required [`BlockHeader`] methods to an inner field.
macro_rules! delegate_block_header {
    ($($accessor:ident).+) => {
        fn parent_hash(&self) -> B256 { self.$($accessor).+.parent_hash() }
        fn ommers_hash(&self) -> B256 { self.$($accessor).+.ommers_hash() }
        fn beneficiary(&self) -> Address { self.$($accessor).+.beneficiary() }
        fn state_root(&self) -> B256 { self.$($accessor).+.state_root() }
        fn transactions_root(&self) -> B256 { self.$($accessor).+.transactions_root() }
        fn receipts_root(&self) -> B256 { self.$($accessor).+.receipts_root() }
        fn withdrawals_root(&self) -> Option<B256> { self.$($accessor).+.withdrawals_root() }
        fn logs_bloom(&self) -> Bloom { self.$($accessor).+.logs_bloom() }
        fn difficulty(&self) -> U256 { self.$($accessor).+.difficulty() }
        fn number(&self) -> BlockNumber { self.$($accessor).+.number() }
        fn gas_limit(&self) -> u64 { self.$($accessor).+.gas_limit() }
        fn gas_used(&self) -> u64 { self.$($accessor).+.gas_used() }
        fn timestamp(&self) -> u64 { self.$($accessor).+.timestamp() }
        fn mix_hash(&self) -> Option<B256> { self.$($accessor).+.mix_hash() }
        fn nonce(&self) -> Option<B64> { self.$($accessor).+.nonce() }
        fn base_fee_per_gas(&self) -> Option<u64> { self.$($accessor).+.base_fee_per_gas() }
        fn blob_gas_used(&self) -> Option<u64> { self.$($accessor).+.blob_gas_used() }
        fn excess_blob_gas(&self) -> Option<u64> { self.$($accessor).+.excess_blob_gas() }
        fn parent_beacon_block_root(&self) -> Option<B256> { self.$($accessor).+.parent_beacon_block_root() }
        fn requests_hash(&self) -> Option<B256> { self.$($accessor).+.requests_hash() }
        fn extra_data(&self) -> &Bytes { self.$($accessor).+.extra_data() }
    };
}

/// A sealed header with a cached block hash.
///
/// This is a type alias for [`Sealed<Header>`], which eagerly computes and
/// stores the header hash on construction.
pub type SealedHeader = Sealed<Header>;

/// Ethereum sealed block type.
///
/// Parameterized on the transaction type `T`:
/// - `SealedBlock<TransactionSigned>` — a block with signed transactions
/// - `SealedBlock<Recovered<TransactionSigned>>` — a block with sender-recovered
///   transactions (see [`RecoveredBlock`])
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedBlock<T = TransactionSigned> {
    /// The sealed header of the block.
    pub header: SealedHeader,
    /// The transactions in the block.
    pub transactions: Vec<T>,
}

impl<T> SealedBlock<T> {
    /// Create a new sealed block.
    pub const fn new(header: SealedHeader, transactions: Vec<T>) -> Self {
        Self { header, transactions }
    }

    /// Create a new empty sealed block for testing.
    #[doc(hidden)]
    pub fn blank_for_testing() -> Self {
        Self { header: Sealed::new(Header::default()), transactions: Vec::new() }
    }

    /// Create a new empty sealed block with the given header for testing.
    #[doc(hidden)]
    pub fn blank_with_header(header: Header) -> Self {
        Self { header: Sealed::new(header), transactions: Vec::new() }
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
    /// Zip transactions with recovered senders to produce a [`RecoveredBlock`].
    pub fn recover(self, senders: Vec<Address>) -> RecoveredBlock {
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
        Self { header: Sealed::new(Header::default()), transactions: Vec::new() }
    }
}

impl RecoveredBlock {
    /// Iterate over the sender addresses of all transactions.
    pub fn senders(&self) -> impl ExactSizeIterator<Item = Address> + '_ {
        self.transactions.iter().map(Recovered::signer)
    }
}

impl<T> BlockHeader for SealedBlock<T> {
    delegate_block_header!(header);
}

/// Typed Transaction type without a signature.
pub type Transaction = EthereumTypedTransaction<TxEip4844>;

/// Signed transaction.
pub type TransactionSigned = EthereumTxEnvelope<TxEip4844>;

/// Ethereum full block type (from alloy).
pub type Block = AlloyBlock<TransactionSigned>;
