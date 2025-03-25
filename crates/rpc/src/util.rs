use reth::{primitives::EthPrimitives, providers::providers::ProviderNodeTypes};
use reth_chainspec::ChainSpec;
use std::{iter::StepBy, ops::RangeInclusive};

macro_rules! await_jh_option {
    ($h:expr) => {
        match $h.await {
            Ok(Some(res)) => res,
            _ => return Err("task panicked or cancelled".to_string()),
        }
    };
}
pub(crate) use await_jh_option;

/// Convenience trait for specifying the [`ProviderNodeTypes`] implementation
/// required for Signet RPC functionality.
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

// Some code in this file has been copied and modified from reth
// <https://github.com/paradigmxyz/reth>
// The original license is included below:
//
// The MIT License (MIT)
//
// Copyright (c) 2022-2025 Reth Contributors
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//.
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.
