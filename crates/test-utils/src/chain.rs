use alloy::{
    consensus::{Header, ReceiptEnvelope},
    primitives::{B256, B64, U256},
};
pub use signet_constants::test_utils::*;
use signet_evm::ExecutionOutcome;
use signet_extract::Extractable;
use signet_types::primitives::{
    BlockBody, RecoveredBlock, SealedBlock, SealedHeader, TransactionSigned,
};

/// A simple chain of blocks with receipts.
#[derive(Clone, PartialEq, Eq)]
pub struct Chain<T = TransactionSigned, H = Header> {
    /// The blocks
    pub blocks: Vec<RecoveredBlock<T, H>>,

    pub execution_outcome: ExecutionOutcome,
}

impl Default for Chain<TransactionSigned, Header> {
    fn default() -> Self {
        Self { blocks: vec![], execution_outcome: Default::default() }
    }
}

impl core::fmt::Debug for Chain<TransactionSigned, Header> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Chain").field("blocks", &self.blocks.len()).finish_non_exhaustive()
    }
}

impl<T, H> Chain<T, H> {
    /// Create a new chain from a block
    pub fn from_block(block: RecoveredBlock<T, H>, execution_outcome: ExecutionOutcome) -> Self {
        Self { blocks: vec![block], execution_outcome }
    }

    pub fn append_block(&mut self, block: RecoveredBlock<T, H>, outcome: ExecutionOutcome) {
        self.blocks.push(block);
        self.execution_outcome.append(outcome);
    }
}

impl Extractable for Chain {
    type Block = RecoveredBlock;
    type Receipt = ReceiptEnvelope;

    fn blocks_and_receipts(&self) -> impl Iterator<Item = (&Self::Block, &Vec<Self::Receipt>)> {
        self.blocks.iter().zip(self.execution_outcome.receipts().iter())
    }
}

/// Make a fake block with a specific number.
pub fn fake_block(number: u64) -> RecoveredBlock {
    let header = Header {
        difficulty: U256::from(0x4000_0000),
        number,
        mix_hash: B256::repeat_byte(0xed),
        nonce: B64::repeat_byte(0xbe),
        timestamp: 1716555576, // no particular significance other than divisible by 12
        excess_blob_gas: Some(0),
        ..Default::default()
    };
    let sealed = SealedHeader::new(header);
    let block = SealedBlock::new_unchecked(sealed, BlockBody::default());
    RecoveredBlock::new(block, vec![])
}
