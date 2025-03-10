use reth::{primitives::EthPrimitives, providers::providers::ProviderNodeTypes};
use reth_chainspec::ChainSpec;
use std::{iter::StepBy, ops::RangeInclusive};

/// Convenience trait for specifying the [`ProviderNodeTypes`] implementation
/// required for signet RPC functionality.
pub trait Pnt: ProviderNodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives> {}

impl<T> Pnt for T where T: ProviderNodeTypes<ChainSpec = ChainSpec, Primitives = EthPrimitives> {}

/// An iterator that yields _inclusive_ block ranges of a given step size
#[derive(Debug)]
pub(crate) struct BlockRangeInclusiveIter {
    iter: StepBy<RangeInclusive<u64>>,
    step: u64,
    end: u64,
}

impl BlockRangeInclusiveIter {
    pub(crate) fn new(range: RangeInclusive<u64>, step: u64) -> Self {
        Self { end: *range.end(), iter: range.step_by(step as usize + 1), step }
    }
}

impl Iterator for BlockRangeInclusiveIter {
    type Item = (u64, u64);

    fn next(&mut self) -> Option<Self::Item> {
        let start = self.iter.next()?;
        let end = (start + self.step).min(self.end);
        if start > end {
            return None;
        }
        Some((start, end))
    }
}
