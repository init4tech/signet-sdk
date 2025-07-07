use crate::{Events, Extractable, ExtractedEvent};
use alloy::consensus::BlockHeader;
use signet_types::{constants::SignetSystemConstants, AggregateFills};
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

impl<C: Extractable> Default for HostEvents<'_, C> {
    fn default() -> Self {
        Self { submitted: None, enters: vec![], transacts: vec![], enter_tokens: vec![] }
    }
}

impl<'a, C: Extractable> HostEvents<'a, C> {
    /// Add an enter event to the host events.
    pub fn ingest_enter(&mut self, event: ExtractedEvent<'a, C::Receipt, Passage::Enter>) {
        self.enters.push(event);
    }

    /// Add a transact event to the host events.
    pub fn ingest_transact(&mut self, event: ExtractedEvent<'a, C::Receipt, Transactor::Transact>) {
        self.transacts.push(event);
    }

    /// Add an enter token event to the host events.
    pub fn ingest_enter_token(
        &mut self,
        event: ExtractedEvent<'a, C::Receipt, Passage::EnterToken>,
    ) {
        self.enter_tokens.push(event);
    }

    /// Add a filled event to the host events.
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
    /// The host block.
    pub host_block: &'a C::Block,
    /// The rollup chain ID.
    pub chain_id: u64,
    /// The rollup block number.
    pub ru_height: u64,

    /// Events
    pub events: HostEvents<'a, C>,

    /// The net fills extracted from the host block.
    pub(crate) context: AggregateFills,
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
    /// Ingest an event into the host events.
    pub fn ingest_event(
        &mut self,
        constants: &SignetSystemConstants,
        event: ExtractedEvent<'a, C::Receipt, Events>,
    ) {
        match event.event {
            Events::Enter(_) => {
                self.events.ingest_enter(event.try_into_enter().expect("checked by match guard"));
            }
            Events::EnterToken(enter) => {
                if constants.is_host_token(enter.token) {
                    self.events.ingest_enter_token(
                        event.try_into_enter_token().expect("checked by match guard"),
                    );
                }
            }
            Events::Filled(fill) => {
                // Fill the swap, ignoring overflows
                // host swaps are pre-filtered to only include the
                // host chain, so no need to check the chain id
                self.context.add_fill(constants.host_chain_id(), &fill);
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
            host_block,
            chain_id: 0,
            ru_height: 0,
            events: HostEvents {
                submitted: None,
                enters: vec![],
                transacts: vec![],
                enter_tokens: vec![],
            },
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
