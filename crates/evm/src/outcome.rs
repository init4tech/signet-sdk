use alloy::{
    consensus::{ReceiptEnvelope, TxReceipt},
    primitives::BlockNumber,
};
use trevm::revm::database::BundleState;

/// The outcome of a block execution, containing the bundle state,
/// receipts, and the first block number in the execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionOutcome<T = ReceiptEnvelope> {
    /// The bundle state after execution.
    bundle: BundleState,
    /// The receipts generated during execution, grouped by block.
    receipts: Vec<Vec<T>>,
    /// The first block number in the execution.
    first_block: u64,
}

impl<T> Default for ExecutionOutcome<T> {
    fn default() -> Self {
        Self { bundle: BundleState::default(), receipts: vec![], first_block: 0 }
    }
}

impl<T> ExecutionOutcome<T> {
    /// Create a new execution outcome.
    pub const fn new(bundle: BundleState, receipts: Vec<Vec<T>>, first_block: u64) -> Self {
        Self { bundle, receipts, first_block }
    }

    /// Decompose the execution outcome into its parts.
    pub fn into_parts(self) -> (BundleState, Vec<Vec<T>>, u64) {
        (self.bundle, self.receipts, self.first_block)
    }

    /// Append another execution outcome to this one.
    pub fn append(&mut self, other: Self) {
        self.bundle.extend(other.bundle);
        self.receipts.extend(other.receipts);
    }

    /// Number of blocks in the execution outcome.
    pub fn len(&self) -> usize {
        self.receipts.len()
    }

    /// Check if the execution outcome is empty.
    pub fn is_empty(&self) -> bool {
        self.receipts.is_empty()
    }

    /// Return first block of the execution outcome
    pub const fn first_block(&self) -> BlockNumber {
        self.first_block
    }

    /// Return last block of the execution outcome
    pub fn last_block(&self) -> BlockNumber {
        (self.first_block + self.len() as u64).saturating_sub(1)
    }

    /// Get the bundle state.
    pub const fn bundle(&self) -> &BundleState {
        &self.bundle
    }

    /// Get the receipts.
    pub fn receipts(&self) -> &[Vec<T>] {
        &self.receipts
    }

    /// Get the receipts for a specific block number. Will return an empty
    /// slice if the block number is out of range.
    pub fn receipts_by_block(&self, block_number: BlockNumber) -> &[T] {
        self.receipts
            .get((block_number - self.first_block) as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Extend one state from another
    ///
    /// For state this is very sensitive operation and should be used only when
    /// we know that other state was build on top of this one.
    pub fn extend(&mut self, other: Self) {
        self.bundle.extend(other.bundle);
        self.receipts.extend(other.receipts);
    }
}
impl<T: TxReceipt> ExecutionOutcome<T> {
    /// Get an iterator over the logs
    pub fn logs(&self) -> impl Iterator<Item = &T::Log> {
        self.receipts.iter().flat_map(|r| r.iter()).flat_map(|receipt| receipt.logs())
    }

    /// Get an iterator over the logs for a specific block number.
    pub fn logs_by_block(&self, block_number: BlockNumber) -> impl Iterator<Item = &T::Log> {
        self.receipts_by_block(block_number).iter().flat_map(|receipt| receipt.logs())
    }
}

// Some code in this file has been copied and modified from reth
// <https://github.com/paradigmxyz/reth>
// The original license is included below:
//
// The MIT License (MIT)
//
// Copyright (c) 2022-2024 Reth Contributors
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
