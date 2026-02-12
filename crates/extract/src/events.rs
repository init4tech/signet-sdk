use alloy::{
    primitives::{Log, LogData},
    sol_types::SolEventInterface,
};
use signet_zenith::{
    Passage::{self, PassageEvents},
    RollupOrders::{self, RollupOrdersEvents},
    Transactor::{self, TransactorEvents},
    Zenith::{self, ZenithEvents},
};

/// Events that we expect to find on the host chain.
///
/// These events are used by the Signet node to update the state of the rollup
/// chain. Each one of these events is expected to be emitted by a different
/// host chain contract.
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
        let event = PassageEvents::decode_log(log).ok().map(|log| log.data)?;

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
        let event = ZenithEvents::decode_log(log).ok().map(|log| log.data)?;

        match event {
            ZenithEvents::BlockSubmitted(e) if e.rollup_chain_id() == filter_chain_id => {
                Some(Self::BlockSubmitted(e))
            }
            _ => None,
        }
    }

    /// Decode a [`Transactor`] event from a log.
    pub fn decode_transactor(log: &Log<LogData>, filter_chain_id: u64) -> Option<Self> {
        let event = TransactorEvents::decode_log(log).ok().map(|log| log.data)?;

        match event {
            TransactorEvents::Transact(e) if e.rollup_chain_id() == filter_chain_id => {
                Some(Self::Transact(e))
            }
            _ => None,
        }
    }

    /// Decode an [`RollupOrders`] event from a log.
    pub fn decode_orders(log: &Log<LogData>, filter_chain_id: u64) -> Option<Self> {
        let event = RollupOrdersEvents::decode_log(log).ok().map(|log| log.data)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, Address, U256};

    fn make_enter_event() -> Passage::Enter {
        Passage::Enter {
            rollupChainId: U256::from(519u64),
            rollupRecipient: Address::ZERO,
            amount: U256::from(1000u64),
        }
    }

    fn make_enter_token_event() -> Passage::EnterToken {
        Passage::EnterToken {
            rollupChainId: U256::from(519u64),
            rollupRecipient: Address::ZERO,
            token: address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
            amount: U256::from(1000u64),
        }
    }

    fn make_block_submitted_event() -> Zenith::BlockSubmitted {
        Zenith::BlockSubmitted {
            sequencer: Address::ZERO,
            rollupChainId: U256::from(519u64),
            gasLimit: U256::from(30_000_000u64),
            rewardAddress: Address::ZERO,
            blockDataHash: [0u8; 32].into(),
        }
    }

    fn make_transact_event() -> Transactor::Transact {
        Transactor::Transact {
            rollupChainId: U256::from(519u64),
            sender: Address::ZERO,
            to: Address::ZERO,
            data: Default::default(),
            value: U256::ZERO,
            gas: U256::from(21000u64),
            maxFeePerGas: U256::from(1_000_000_000u64),
        }
    }

    fn make_filled_event() -> RollupOrders::Filled {
        RollupOrders::Filled { outputs: vec![] }
    }

    // Test From implementations
    #[test]
    fn from_enter_token() {
        let e = make_enter_token_event();
        let events: Events = e.into();
        assert!(events.is_enter_token());
        assert!(events.as_enter_token().is_some());
    }

    #[test]
    fn from_enter() {
        let e = make_enter_event();
        let events: Events = e.into();
        assert!(events.is_enter());
        assert!(events.as_enter().is_some());
    }

    #[test]
    fn from_block_submitted() {
        let e = make_block_submitted_event();
        let events: Events = e.into();
        assert!(events.is_block_submitted());
        assert!(events.as_block_submitted().is_some());
    }

    #[test]
    fn from_filled() {
        let e = make_filled_event();
        let events: Events = e.clone().into();
        assert!(events.is_filled());
        assert!(events.as_filled().is_some());
    }

    #[test]
    fn from_transact() {
        let e = make_transact_event();
        let events: Events = e.clone().into();
        assert!(events.is_transact());
        assert!(events.as_transact().is_some());
    }

    // Test is_* methods return false for other variants
    #[test]
    fn enter_is_not_other_types() {
        let events: Events = make_enter_event().into();
        assert!(!events.is_enter_token());
        assert!(!events.is_block_submitted());
        assert!(!events.is_transact());
        assert!(!events.is_filled());
    }

    #[test]
    fn enter_token_is_not_other_types() {
        let events: Events = make_enter_token_event().into();
        assert!(!events.is_enter());
        assert!(!events.is_block_submitted());
        assert!(!events.is_transact());
        assert!(!events.is_filled());
    }

    #[test]
    fn block_submitted_is_not_other_types() {
        let events: Events = make_block_submitted_event().into();
        assert!(!events.is_enter());
        assert!(!events.is_enter_token());
        assert!(!events.is_transact());
        assert!(!events.is_filled());
    }

    #[test]
    fn transact_is_not_other_types() {
        let events: Events = make_transact_event().into();
        assert!(!events.is_enter());
        assert!(!events.is_enter_token());
        assert!(!events.is_block_submitted());
        assert!(!events.is_filled());
    }

    #[test]
    fn filled_is_not_other_types() {
        let events: Events = make_filled_event().into();
        assert!(!events.is_enter());
        assert!(!events.is_enter_token());
        assert!(!events.is_block_submitted());
        assert!(!events.is_transact());
    }

    // Test as_* returns None for wrong variants
    #[test]
    fn as_enter_returns_none_for_other() {
        let events: Events = make_enter_token_event().into();
        assert!(events.as_enter().is_none());
    }

    #[test]
    fn as_enter_token_returns_none_for_other() {
        let events: Events = make_enter_event().into();
        assert!(events.as_enter_token().is_none());
    }

    #[test]
    fn as_block_submitted_returns_none_for_other() {
        let events: Events = make_enter_event().into();
        assert!(events.as_block_submitted().is_none());
    }

    #[test]
    fn as_transact_returns_none_for_other() {
        let events: Events = make_enter_event().into();
        assert!(events.as_transact().is_none());
    }

    #[test]
    fn as_filled_returns_none_for_other() {
        let events: Events = make_enter_event().into();
        assert!(events.as_filled().is_none());
    }

    // Test equality
    #[test]
    fn events_equality() {
        let e1: Events = make_enter_event().into();
        let e2: Events = make_enter_event().into();
        assert_eq!(e1, e2);
    }

    #[test]
    fn events_inequality() {
        let e1: Events = make_enter_event().into();
        let e2: Events = make_enter_token_event().into();
        assert_ne!(e1, e2);
    }

    // Test Debug trait
    #[test]
    fn events_debug() {
        let events: Events = make_enter_event().into();
        let debug_str = format!("{:?}", events);
        assert!(debug_str.contains("Enter"));
    }

    // Test Clone trait
    #[test]
    fn events_clone() {
        let e1: Events = make_enter_event().into();
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }
}
