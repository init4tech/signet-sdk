use alloy::{
    primitives::{Log, LogData},
    sol_types::SolEventInterface,
};
use zenith_types::{
    Passage::{self, PassageEvents},
    RollupOrders::{self, RollupOrdersEvents},
    Transactor::{self, TransactorEvents},
    Zenith::{self, ZenithEvents},
};

/// Events that we expect to find on the host chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Events {
    /// An [`Passage::EnterToken`] event.
    EnterToken(Passage::EnterToken),
    /// An [`Passage::Enter`] event.
    Enter(Passage::Enter),
    /// A [`Zenith::BlockSubmitted`] event.
    BlockSubmitted(Zenith::BlockSubmitted),
    /// A [`Transactor::Transact`] event.
    Transact(Transactor::Transact),
    /// A [`RollupOrders::Filled`] event.
    Filled(RollupOrders::Filled),
}

impl From<Passage::EnterToken> for Events {
    fn from(e: Passage::EnterToken) -> Self {
        Events::EnterToken(e)
    }
}

impl From<Passage::Enter> for Events {
    fn from(e: Passage::Enter) -> Self {
        Events::Enter(e)
    }
}

impl From<Zenith::BlockSubmitted> for Events {
    fn from(e: Zenith::BlockSubmitted) -> Self {
        Events::BlockSubmitted(e)
    }
}

impl From<RollupOrders::Filled> for Events {
    fn from(e: RollupOrders::Filled) -> Self {
        Events::Filled(e)
    }
}

impl From<Transactor::Transact> for Events {
    fn from(e: Transactor::Transact) -> Self {
        Events::Transact(e)
    }
}

impl Events {
    /// Decode a [`Passage`] event from a log.
    pub fn decode_passage(log: &Log<LogData>, filter_chain_id: u64) -> Option<Self> {
        let event = PassageEvents::decode_log(log, true).ok().map(|log| log.data)?;

        match event {
            PassageEvents::Enter(e) if e.rollup_chain_id() == filter_chain_id => {
                Some(Self::Enter(e))
            }
            PassageEvents::EnterToken(e) if e.rollup_chain_id() == filter_chain_id => {
                Some(Self::EnterToken(e))
            }
            _ => None,
        }
    }

    /// Decode a [`Zenith`] event from a log.
    pub fn decode_zenith(log: &Log<LogData>, filter_chain_id: u64) -> Option<Self> {
        let event = ZenithEvents::decode_log(log, true).ok().map(|log| log.data)?;

        match event {
            ZenithEvents::BlockSubmitted(e) if e.rollup_chain_id() == filter_chain_id => {
                Some(Self::BlockSubmitted(e))
            }
            _ => None,
        }
    }

    /// Decode a [`Transactor`] event from a log.
    pub fn decode_transactor(log: &Log<LogData>, filter_chain_id: u64) -> Option<Self> {
        let event = TransactorEvents::decode_log(log, true).ok().map(|log| log.data)?;

        match event {
            TransactorEvents::Transact(e) if e.rollup_chain_id() == filter_chain_id => {
                Some(Self::Transact(e))
            }
            _ => None,
        }
    }

    /// Decode an [`RollupOrders`] event from a log.
    pub fn decode_orders(log: &Log<LogData>, filter_chain_id: u64) -> Option<Self> {
        let event = RollupOrdersEvents::decode_log(log, true).ok().map(|log| log.data)?;

        match event {
            RollupOrdersEvents::Filled(mut e) => {
                e.outputs.retain(|o| o.chain_id() as u64 == filter_chain_id);
                Some(Self::Filled(e))
            }
            _ => None,
        }
    }

    /// True if this event is an [`Passage::EnterToken`] event.
    pub const fn is_enter_token(&self) -> bool {
        matches!(self, Events::EnterToken(_))
    }

    /// Falllible cast to an [`Passage::EnterToken`] event.
    pub const fn as_enter_token(&self) -> Option<&Passage::EnterToken> {
        match &self {
            Events::EnterToken(e) => Some(e),
            _ => None,
        }
    }

    /// True if this event is an [`Passage::Enter`] event.
    pub const fn is_enter(&self) -> bool {
        matches!(self, Events::Enter(_))
    }

    /// Falllible cast to an [`Passage::Enter`] event.
    pub const fn as_enter(&self) -> Option<&Passage::Enter> {
        match &self {
            Events::Enter(e) => Some(e),
            _ => None,
        }
    }

    /// True if this event is an [`Zenith::BlockSubmitted`] event.
    pub const fn is_block_submitted(&self) -> bool {
        matches!(self, Events::BlockSubmitted(_))
    }

    /// Falllible cast to an [`Zenith::BlockSubmitted`] event
    pub const fn as_block_submitted(&self) -> Option<&Zenith::BlockSubmitted> {
        match &self {
            Events::BlockSubmitted(e) => Some(e),
            _ => None,
        }
    }

    /// True if this event is an [`Transactor::Transact`] event
    pub const fn is_transact(&self) -> bool {
        matches!(self, Events::Transact(_))
    }

    /// Falllible cast to an [`Transactor::Transact`] event
    pub const fn as_transact(&self) -> Option<&Transactor::Transact> {
        match &self {
            Events::Transact(e) => Some(e),
            _ => None,
        }
    }

    /// True if this event is an [`RollupOrders::Filled`] event
    pub const fn is_filled(&self) -> bool {
        matches!(self, Events::Filled(_))
    }

    /// Falllible cast to an [`RollupOrders::Filled`] event
    pub const fn as_filled(&self) -> Option<&RollupOrders::Filled> {
        match &self {
            Events::Filled(e) => Some(e),
            _ => None,
        }
    }
}
