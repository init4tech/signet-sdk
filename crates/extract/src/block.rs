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
    pub fn ingest_block_submitted(
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
