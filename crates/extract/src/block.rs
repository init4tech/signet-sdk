use crate::ExtractedEvent;
use reth::primitives::{Block, RecoveredBlock};
use signet_types::AggregateFills;
use signet_zenith::{Passage, Transactor, Zenith};

/// The output of the block extraction process. This struct contains borrows
/// from a block object, the extracted events, and a [`AggregateFills`]
/// populated with the fills present in the host block.
#[derive(Debug, Clone)]
pub struct Extracts<'a> {
    /// The host block.
    pub host_block: &'a RecoveredBlock<Block>,
    /// The rollup chain ID.
    pub chain_id: u64,
    /// The rollup block number.
    pub ru_height: u64,
    /// The submitted event.
    pub submitted: Option<ExtractedEvent<'a, Zenith::BlockSubmitted>>,
    /// The enters.
    pub enters: Vec<ExtractedEvent<'a, Passage::Enter>>,
    /// The transacts.
    pub transacts: Vec<ExtractedEvent<'a, Transactor::Transact>>,
    /// The enter tokens.
    pub enter_tokens: Vec<ExtractedEvent<'a, Passage::EnterToken>>,
    /// The net fills extracted from the host block.
    pub(crate) context: AggregateFills,
}

impl Extracts<'_> {
    /// Get the header of the block that was submitted (if any).
    pub fn ru_header(&self) -> Option<Zenith::BlockHeader> {
        self.submitted.as_ref().map(|s| s.ru_header(self.host_block_number()))
    }

    /// Get the host block number.
    pub fn host_block_number(&self) -> u64 {
        self.host_block.number
    }

    /// Get the host block timestamp.
    pub fn host_block_timestamp(&self) -> u64 {
        self.host_block.timestamp
    }

    /// True if the host block contains a [`BlockSubmitted`] event.
    ///
    /// [`BlockSubmitted`]: Zenith::BlockSubmitted
    pub const fn contains_block(&self) -> bool {
        self.submitted.is_some()
    }

    /// Get the transacts.
    pub fn transacts(&self) -> impl Iterator<Item = &Transactor::Transact> + '_ {
        self.transacts.iter().map(|e| &e.event)
    }

    /// Get the enters.
    pub fn enters(&self) -> impl Iterator<Item = Passage::Enter> + '_ {
        self.enters.iter().map(|e| e.event)
    }

    /// Get the enter tokens.
    pub fn enter_tokens(&self) -> impl Iterator<Item = Passage::EnterToken> + '_ {
        self.enter_tokens.iter().map(|e| e.event)
    }

    /// Get a clone of the market context.
    pub fn aggregate_fills(&self) -> AggregateFills {
        self.context.clone()
    }
}

impl<'a> Extracts<'a> {
    /// Used for testing.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn empty(host_block: &'a RecoveredBlock<Block>) -> Self {
        Self {
            host_block,
            chain_id: 0,
            ru_height: 0,
            submitted: None,
            enters: vec![],
            transacts: vec![],
            enter_tokens: vec![],
            context: Default::default(),
        }
    }
}
