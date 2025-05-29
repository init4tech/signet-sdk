//! Many of these types are re-produced from the `reth-primitives` crate family.

use alloy::{
    consensus::{
        Block as AlloyBlock, BlockBody as AlloyBlockBody, BlockHeader, EthereumTxEnvelope,
        EthereumTypedTransaction, Header, TxEip4844,
    },
    primitives::{Address, BlockHash, BlockNumber, Bloom, Bytes, B256, B64, U256},
};
use std::sync::OnceLock;

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
    pub fn new(header: H) -> Self {
        Self { hash: OnceLock::new(), header }
    }

    /// Get the header
    pub fn header(&self) -> &H {
        &self.header
    }
}

impl SealedHeader {
    /// Get the block hash of the sealed header.
    pub fn hash(&self) -> BlockHash {
        self.hash.get_or_init(|| BlockHash::from(self.header.hash_slow())).clone()
    }

    /// Split the sealed header into its components.
    pub fn split(self) -> (BlockHash, Header) {
        let hash = self.hash();
        (hash, self.header)
    }
}

impl<H: BlockHeader> BlockHeader for SealedHeader<H> {
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
    pub fn new_unchecked(header: SealedHeader<H>, body: AlloyBlockBody<T, H>) -> Self {
        Self { header, body }
    }
}

impl<T, H: BlockHeader> BlockHeader for SealedBlock<T, H> {
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
    pub fn new(block: SealedBlock<T, H>, senders: Vec<Address>) -> Self {
        Self { block, senders }
    }
}

impl<T, H: BlockHeader> BlockHeader for RecoveredBlock<T, H> {
    fn parent_hash(&self) -> B256 {
        self.block.parent_hash()
    }

    fn ommers_hash(&self) -> B256 {
        self.block.ommers_hash()
    }

    fn beneficiary(&self) -> Address {
        self.block.beneficiary()
    }

    fn state_root(&self) -> B256 {
        self.block.state_root()
    }

    fn transactions_root(&self) -> B256 {
        self.block.transactions_root()
    }

    fn receipts_root(&self) -> B256 {
        self.block.receipts_root()
    }

    fn withdrawals_root(&self) -> Option<B256> {
        self.block.withdrawals_root()
    }

    fn logs_bloom(&self) -> Bloom {
        self.block.logs_bloom()
    }

    fn difficulty(&self) -> U256 {
        self.block.difficulty()
    }

    fn number(&self) -> BlockNumber {
        self.block.number()
    }

    fn gas_limit(&self) -> u64 {
        self.block.gas_limit()
    }

    fn gas_used(&self) -> u64 {
        self.block.gas_used()
    }

    fn timestamp(&self) -> u64 {
        self.block.timestamp()
    }

    fn mix_hash(&self) -> Option<B256> {
        self.block.mix_hash()
    }

    fn nonce(&self) -> Option<B64> {
        self.block.nonce()
    }

    fn base_fee_per_gas(&self) -> Option<u64> {
        self.block.base_fee_per_gas()
    }

    fn blob_gas_used(&self) -> Option<u64> {
        self.block.blob_gas_used()
    }

    fn excess_blob_gas(&self) -> Option<u64> {
        self.block.excess_blob_gas()
    }

    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.block.parent_beacon_block_root()
    }

    fn requests_hash(&self) -> Option<B256> {
        self.block.requests_hash()
    }

    fn extra_data(&self) -> &Bytes {
        self.block.extra_data()
    }
}

/// Typed Transaction type without a signature
pub type Transaction = EthereumTypedTransaction<TxEip4844>;

/// Signed transaction.
pub type TransactionSigned = EthereumTxEnvelope<TxEip4844>;

/// Ethereum full block.
///
/// Withdrawals can be optionally included at the end of the RLP encoded message.
pub type Block<T = TransactionSigned, H = Header> = AlloyBlock<T, H>;
