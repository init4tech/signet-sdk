use alloy::{
    consensus::{BlockHeader, Header, ReceiptEnvelope},
    primitives::{B256, B64, U256},
};
pub use signet_constants::test_utils::*;
use signet_evm::ExecutionOutcome;
use signet_extract::{BlockAndReceipts, Extractable};
use signet_types::primitives::{RecoveredBlock, SealedBlock, SealedHeader};

/// A simple chain of blocks with receipts.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Chain {
    /// The blocks
    pub blocks: Vec<RecoveredBlock>,

    pub execution_outcome: ExecutionOutcome,
}

impl core::fmt::Debug for Chain {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Chain").field("blocks", &self.blocks.len()).finish_non_exhaustive()
    }
}

impl Chain {
    /// Create a new chain from a block
    pub fn from_block(block: RecoveredBlock, execution_outcome: ExecutionOutcome) -> Self {
        Self { blocks: vec![block], execution_outcome }
    }

    pub fn append_block(&mut self, block: RecoveredBlock, outcome: ExecutionOutcome) {
        self.blocks.push(block);
        self.execution_outcome.append(outcome);
    }
}

impl Extractable for Chain {
    type Block = RecoveredBlock;
    type Receipt = ReceiptEnvelope;

    fn blocks_and_receipts(
        &self,
    ) -> impl Iterator<Item = BlockAndReceipts<'_, Self::Block, Self::Receipt>> {
        self.blocks
            .iter()
            .zip(self.execution_outcome.receipts().iter())
            .map(|(block, receipts)| BlockAndReceipts { block, receipts })
    }

    fn first_number(&self) -> Option<u64> {
        self.blocks.first().map(|b| b.number())
    }

    fn tip_number(&self) -> Option<u64> {
        self.blocks.last().map(|b| b.number())
    }

    fn len(&self) -> usize {
        self.blocks.len()
    }

    fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

/// Make a chain with `count` fake blocks numbered `0..count`.
pub fn fake_chain(count: u64) -> Chain {
    let blocks: Vec<_> = (0..count).map(fake_block).collect();
    let receipts = vec![vec![]; count as usize];
    let execution_outcome = ExecutionOutcome::new(Default::default(), receipts, 0);
    Chain { blocks, execution_outcome }
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
    SealedBlock::new(sealed, vec![]).recover_unchecked(vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_chain_metadata() {
        let chain = fake_chain(0);
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
        assert_eq!(chain.first_number(), None);
        assert_eq!(chain.tip_number(), None);
    }

    #[test]
    fn single_block_metadata() {
        let chain = fake_chain(1);
        assert!(!chain.is_empty());
        assert_eq!(chain.len(), 1);
        assert_eq!(chain.first_number(), Some(0));
        assert_eq!(chain.tip_number(), Some(0));
    }

    #[test]
    fn multi_block_metadata() {
        let chain = fake_chain(5);
        assert!(!chain.is_empty());
        assert_eq!(chain.len(), 5);
        assert_eq!(chain.first_number(), Some(0));
        assert_eq!(chain.tip_number(), Some(4));
    }
}
