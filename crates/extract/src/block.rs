use crate::{Events, Extractable, ExtractedEvent};
use alloy::consensus::BlockHeader;
use signet_types::AggregateFills;
use signet_zenith::{Passage, Transactor, Zenith};

/// Events extracted from a host block.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct HostEvents<'a, C: Extractable> {
    /// The submitted event.
    pub submitted: Option<ExtractedEvent<'a, C::Receipt, Zenith::BlockSubmitted>>,

    /// The enters.
    pub enters: Vec<ExtractedEvent<'a, C::Receipt, Passage::Enter>>,
    /// The transacts.
    pub transacts: Vec<ExtractedEvent<'a, C::Receipt, Transactor::Transact>>,
    /// The enter tokens.
    pub enter_tokens: Vec<ExtractedEvent<'a, C::Receipt, Passage::EnterToken>>,
}

// NB: manual implementation because derived version incorrectly bounds `Vec<T>`
// where T: Default`
impl<C: Extractable> Default for HostEvents<'_, C> {
    fn default() -> Self {
        Self { submitted: None, enters: vec![], transacts: vec![], enter_tokens: vec![] }
    }
}

impl<'a, C: Extractable> HostEvents<'a, C> {
    /// Add [`Passage::Enter`] event to the host events.
    pub fn ingest_enter(&mut self, event: ExtractedEvent<'a, C::Receipt, Passage::Enter>) {
        self.enters.push(event);
    }

    /// Add an [`Passage::EnterToken`] event to the host events.
    pub fn ingest_enter_token(
        &mut self,
        event: ExtractedEvent<'a, C::Receipt, Passage::EnterToken>,
    ) {
        self.enter_tokens.push(event);
    }

    /// Add a [`Transactor::Transact`] event to the host events.
    pub fn ingest_transact(&mut self, event: ExtractedEvent<'a, C::Receipt, Transactor::Transact>) {
        self.transacts.push(event);
    }

    /// Add a [`Zenith::BlockSubmitted`] event to the host events.
    pub const fn ingest_block_submitted(
        &mut self,
        event: ExtractedEvent<'a, C::Receipt, Zenith::BlockSubmitted>,
    ) {
        self.submitted = Some(event);
    }
}

/// The output of the block extraction process. This struct contains borrows
/// from a block object, the extracted events, and a [`AggregateFills`]
/// populated with the fills present in the host block.
#[derive(Debug, Clone)]
pub struct Extracts<'a, C: Extractable> {
    /// The `chain_id` of the host chain.
    pub host_chain_id: u64,
    /// The host block.
    pub host_block: &'a C::Block,

    /// The rollup chain ID.
    pub chain_id: u64,
    /// The rollup block number.
    pub ru_height: u64,

    /// Events
    pub events: HostEvents<'a, C>,

    /// The net fills extracted from the host block.
    context: AggregateFills,
}

impl<'a, C: Extractable> Extracts<'a, C> {
    /// Create a new [`Extracts`] from the given host block and chain ID.
    pub fn new(
        host_chain_id: u64,
        host_block: &'a C::Block,
        chain_id: u64,
        ru_height: u64,
    ) -> Self {
        Self {
            host_chain_id,
            host_block,
            chain_id,
            ru_height,
            events: Default::default(),
            context: Default::default(),
        }
    }
}

impl<C: Extractable> Extracts<'_, C> {
    /// True if the host block contains a [`BlockSubmitted`] event.
    ///
    /// [`BlockSubmitted`]: Zenith::BlockSubmitted
    pub const fn contains_block(&self) -> bool {
        self.events.submitted.is_some()
    }

    /// Get the transacts.
    pub fn transacts(&self) -> impl Iterator<Item = &Transactor::Transact> + '_ {
        self.events.transacts.iter().map(|e| &e.event)
    }

    /// Get the enters.
    pub fn enters(&self) -> impl Iterator<Item = Passage::Enter> + '_ {
        self.events.enters.iter().map(|e| e.event)
    }

    /// Get the enter tokens.
    pub fn enter_tokens(&self) -> impl Iterator<Item = Passage::EnterToken> + '_ {
        self.events.enter_tokens.iter().map(|e| e.event)
    }

    /// Get a clone of the market context.
    pub fn aggregate_fills(&self) -> AggregateFills {
        self.context.clone()
    }

    /// Get the host block number.
    pub fn host_block_number(&self) -> u64 {
        self.host_block.number()
    }

    /// Get the host block timestamp.
    pub fn host_block_timestamp(&self) -> u64 {
        self.host_block.timestamp()
    }

    /// Get the header of the block that was submitted (if any).
    pub fn ru_header(&self) -> Option<Zenith::BlockHeader> {
        self.events.submitted.as_ref().map(|s| s.ru_header(self.host_block_number()))
    }
}

impl<'a, C: Extractable> Extracts<'a, C> {
    /// Ingest an [`Events`] into the host events, updating the [`HostEvents`]
    /// or the [`AggregateFills`].
    pub fn ingest_event(&mut self, event: ExtractedEvent<'a, C::Receipt, Events>) {
        match event.event {
            Events::Enter(_) => {
                self.events.ingest_enter(event.try_into_enter().expect("checked by match guard"));
            }
            Events::EnterToken(_) => {
                // NB: It is assumed that the `EnterToken` event has already
                // been filtered to only include host tokens during the
                // extraction process.
                self.events.ingest_enter_token(
                    event.try_into_enter_token().expect("checked by match guard"),
                );
            }
            Events::Filled(fill) => {
                // Fill the swap, ignoring overflows
                // host swaps are pre-filtered to only include the
                // host chain, so no need to check the chain id
                self.context.add_fill(self.host_chain_id, &fill);
            }
            Events::Transact(_) => {
                self.events
                    .ingest_transact(event.try_into_transact().expect("checked by match guard"));
            }
            Events::BlockSubmitted(_) => {
                self.events.ingest_block_submitted(
                    event.try_into_block_submitted().expect("checked by match guard"),
                );
            }
        }
    }

    /// Used for testing.
    #[doc(hidden)]
    pub fn empty(host_block: &'a C::Block) -> Self {
        Self {
            host_chain_id: 0,
            host_block,
            chain_id: 0,
            ru_height: 0,
            events: Default::default(),
            context: Default::default(),
        }
    }
}

impl<'a, C: Extractable> core::ops::Deref for Extracts<'a, C> {
    type Target = HostEvents<'a, C>;

    fn deref(&self) -> &Self::Target {
        &self.events
    }
}

impl<'a, C: Extractable> core::ops::DerefMut for Extracts<'a, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::consensus::{Header, ReceiptWithBloom};
    use signet_types::primitives::{BlockBody, SealedBlock, SealedHeader};

    // Mock Extractable implementation for testing
    #[derive(Debug, Clone)]
    struct MockExtractable {
        blocks: Vec<SealedBlock>,
        receipts: Vec<Vec<ReceiptWithBloom>>,
    }

    impl crate::Extractable for MockExtractable {
        type Block = SealedBlock;
        type Receipt = ReceiptWithBloom;

        fn blocks_and_receipts(&self) -> impl Iterator<Item = (&Self::Block, &Vec<Self::Receipt>)> {
            self.blocks.iter().zip(self.receipts.iter())
        }
    }

    fn make_mock_block() -> SealedBlock {
        let header = Header { number: 100, timestamp: 1234567890, ..Default::default() };
        let sealed_header = SealedHeader::new(header);
        SealedBlock::new_unchecked(sealed_header, BlockBody::default())
    }

    // Test HostEvents Default impl
    #[test]
    fn host_events_default() {
        let events: HostEvents<'_, MockExtractable> = Default::default();
        assert!(events.submitted.is_none());
        assert!(events.enters.is_empty());
        assert!(events.transacts.is_empty());
        assert!(events.enter_tokens.is_empty());
    }

    // Test Extracts::new
    #[test]
    fn extracts_new() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);

        assert_eq!(extracts.host_chain_id, 1);
        assert_eq!(extracts.chain_id, 519);
        assert_eq!(extracts.ru_height, 50);
        assert!(!extracts.contains_block());
    }

    // Test Extracts::empty
    #[test]
    fn extracts_empty() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::empty(&block);

        assert_eq!(extracts.host_chain_id, 0);
        assert_eq!(extracts.chain_id, 0);
        assert_eq!(extracts.ru_height, 0);
        assert!(!extracts.contains_block());
    }

    // Test Extracts::contains_block
    #[test]
    fn extracts_contains_block_false_when_no_submitted() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        assert!(!extracts.contains_block());
    }

    // Test Extracts accessor methods
    #[test]
    fn extracts_host_block_number() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        assert_eq!(extracts.host_block_number(), 100);
    }

    #[test]
    fn extracts_host_block_timestamp() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        assert_eq!(extracts.host_block_timestamp(), 1234567890);
    }

    // Test iterator methods return empty when no events
    #[test]
    fn extracts_transacts_empty() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        assert_eq!(extracts.transacts().count(), 0);
    }

    #[test]
    fn extracts_enters_empty() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        assert_eq!(extracts.enters().count(), 0);
    }

    #[test]
    fn extracts_enter_tokens_empty() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        assert_eq!(extracts.enter_tokens().count(), 0);
    }

    // Test aggregate_fills returns default context
    #[test]
    fn extracts_aggregate_fills_default() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        let fills = extracts.aggregate_fills();
        // AggregateFills is created via Default, so it's in its initial state
        let default_fills = AggregateFills::default();
        assert_eq!(format!("{:?}", fills), format!("{:?}", default_fills));
    }

    // Test ru_header returns None when no block submitted
    #[test]
    fn extracts_ru_header_none() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        assert!(extracts.ru_header().is_none());
    }

    // Test Deref
    #[test]
    fn extracts_deref() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        // Test that we can access HostEvents fields through Deref
        assert!(extracts.submitted.is_none());
    }

    // Test Clone
    #[test]
    fn extracts_clone() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        let cloned = extracts.clone();
        assert_eq!(cloned.host_chain_id, extracts.host_chain_id);
        assert_eq!(cloned.chain_id, extracts.chain_id);
        assert_eq!(cloned.ru_height, extracts.ru_height);
    }

    // Test Debug
    #[test]
    fn extracts_debug() {
        let block = make_mock_block();
        let extracts: Extracts<'_, MockExtractable> = Extracts::new(1, &block, 519, 50);
        let debug_str = format!("{:?}", extracts);
        assert!(debug_str.contains("Extracts"));
    }

    #[test]
    fn host_events_debug() {
        let events: HostEvents<'_, MockExtractable> = Default::default();
        let debug_str = format!("{:?}", events);
        assert!(debug_str.contains("HostEvents"));
    }
}
