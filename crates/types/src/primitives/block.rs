//! Many of these types are re-produced from the `reth-primitives` crate family.

use alloy::{
    consensus::{
        Block as AlloyBlock, BlockBody as AlloyBlockBody, BlockHeader, EthereumTxEnvelope,
        EthereumTypedTransaction, Header, TxEip4844,
    },
    primitives::{Address, BlockHash, BlockNumber, Bloom, Bytes, B256, B64, U256},
};
use std::sync::OnceLock;

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

/// A type alias for the block body used in Ethereum blocks.
pub type BlockBody<T = TransactionSigned, H = Header> = AlloyBlockBody<T, H>;

/// A Sealed header type
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SealedHeader<H = Header> {
    /// Block hash
    hash: OnceLock<BlockHash>,
    /// Locked Header fields.
    header: H,
}

impl<H> SealedHeader<H> {
    /// Create a new sealed header.
    pub const fn new(header: H) -> Self {
        Self { hash: OnceLock::new(), header }
    }

    /// Get the header
    pub const fn header(&self) -> &H {
        &self.header
    }
}

impl SealedHeader {
    /// Get the block hash of the sealed header.
    pub fn hash(&self) -> BlockHash {
        *self.hash.get_or_init(|| BlockHash::from(self.header.hash_slow()))
    }

    /// Split the sealed header into its components.
    pub fn split(self) -> (BlockHash, Header) {
        let hash = self.hash();
        (hash, self.header)
    }
}

impl<H: BlockHeader> BlockHeader for SealedHeader<H> {
    delegate_block_header!(header);
}

/// Ethereum sealed block type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SealedBlock<T = TransactionSigned, H = Header> {
    /// The sealed header of the block.
    pub header: SealedHeader<H>,
    /// The transactions in the block.
    pub body: AlloyBlockBody<T, H>,
}

impl<T, H> SealedBlock<T, H> {
    /// Create a new sealed block without checking the header hash.
    pub const fn new_unchecked(header: SealedHeader<H>, body: AlloyBlockBody<T, H>) -> Self {
        Self { header, body }
    }

    /// Create a new empty sealed block for testing.
    #[doc(hidden)]
    pub fn blank_for_testing() -> Self
    where
        H: Default,
    {
        Self { header: SealedHeader::new(H::default()), body: AlloyBlockBody::default() }
    }

    /// Create a new empty sealed block with the given header for testing.
    #[doc(hidden)]
    pub fn blank_with_header(header: H) -> Self {
        Self { header: SealedHeader::new(header), body: AlloyBlockBody::default() }
    }

    /// Get the transactions in the block.
    fn transactions(&self) -> &[T] {
        &self.body.transactions
    }
}

impl<T, H: BlockHeader> BlockHeader for SealedBlock<T, H> {
    delegate_block_header!(header);
}

/// A [`SealedBlock`] with the senders of the transactions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecoveredBlock<T = TransactionSigned, H = Header> {
    /// The block.
    pub block: SealedBlock<T, H>,
    /// The senders
    pub senders: Vec<Address>,
}

impl<T, H> RecoveredBlock<T, H> {
    /// Create a new recovered block.
    pub const fn new(block: SealedBlock<T, H>, senders: Vec<Address>) -> Self {
        Self { block, senders }
    }

    /// Create a new empty recovered block for testing.
    #[doc(hidden)]
    pub fn blank_for_testing() -> Self
    where
        H: Default,
    {
        Self { block: SealedBlock::blank_for_testing(), senders: Vec::new() }
    }

    /// Create a new empty recovered block with the given header for testing.
    #[doc(hidden)]
    pub fn blank_with_header(header: H) -> Self {
        Self { block: SealedBlock::blank_with_header(header), senders: Vec::new() }
    }

    /// Get the transactions in the block.
    pub fn transactions(&self) -> &[T] {
        self.block.transactions()
    }
}

impl<T, H: BlockHeader> BlockHeader for RecoveredBlock<T, H> {
    delegate_block_header!(block);
}

/// Typed Transaction type without a signature
pub type Transaction = EthereumTypedTransaction<TxEip4844>;

/// Signed transaction.
pub type TransactionSigned = EthereumTxEnvelope<TxEip4844>;

/// Ethereum full block.
///
/// Withdrawals can be optionally included at the end of the RLP encoded message.
pub type Block<T = TransactionSigned, H = Header> = AlloyBlock<T, H>;
