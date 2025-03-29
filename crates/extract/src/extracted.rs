use alloy::{
    eips::Typed2718,
    primitives::{Log, TxHash, U256},
};
use reth::primitives::{Receipt, TransactionSigned};
use signet_zenith::{Passage, RollupOrders, Transactor, Zenith};

use crate::Events;

/// A single event extracted from the host chain.
///
/// This struct contains a reference to the transaction that caused the event,
/// the receipt that the event was extracted from, the index of the log in the
/// receipt's logs, and the extracted event itself.
///
/// Events may be either the enum type [`Events`], or a specific event type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedEvent<'a, T = Events> {
    /// The transaction that caused the event
    pub tx: &'a TransactionSigned,
    /// The receipt that the event was extracted from.
    pub receipt: &'a Receipt,
    /// The index of the log in the receipt's logs.
    pub log_index: usize,
    /// The extracted event.
    pub event: T,
}

impl<T> std::ops::Deref for ExtractedEvent<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.event
    }
}

impl<T> ExtractedEvent<'_, T> {
    /// Get the transaction hash of the extracted event.
    pub fn tx_hash(&self) -> TxHash {
        *self.tx.hash()
    }

    /// Borrow the raw log from the receipt.
    pub fn raw_log(&self) -> &Log {
        &self.receipt.logs[self.log_index]
    }
}

impl<'a> ExtractedEvent<'a, Events> {
    /// True if the event is an [`Passage::EnterToken`].
    pub const fn is_enter_token(&self) -> bool {
        self.event.is_enter_token()
    }

    /// Attempt to convert this event into an [`Passage::EnterToken`]. If the
    /// event is not an [`Passage::EnterToken`], it will return `None`.
    pub const fn as_enter_token(&self) -> Option<&Passage::EnterToken> {
        self.event.as_enter_token()
    }

    /// Attempt to convert this event into an [`EnterToken`]. If the event is
    /// not an [`EnterToken`], it returns an error.
    ///
    /// [`EnterToken`]: Passage::EnterToken
    pub fn try_into_enter_token(self) -> Result<ExtractedEvent<'a, Passage::EnterToken>, Self> {
        match self.event {
            Events::EnterToken(event) => Ok(ExtractedEvent {
                tx: self.tx,
                receipt: self.receipt,
                log_index: self.log_index,
                event,
            }),
            _ => Err(self),
        }
    }

    /// True if the event is an [`Passage::Enter`].
    pub const fn is_enter(&self) -> bool {
        self.event.is_enter()
    }

    /// Get a reference to the inner event, if it is an [`Enter`].
    ///
    /// [`Enter`]: Passage::Enter
    pub const fn as_enter(&self) -> Option<&Passage::Enter> {
        self.event.as_enter()
    }

    /// Attempt to convert this event into an [`Enter`]. If the event is not an
    /// [`Enter`], it will be returned as an error.
    ///
    /// [`Enter`]: Passage::Enter
    pub fn try_into_enter(self) -> Result<ExtractedEvent<'a, Passage::Enter>, Self> {
        match self.event {
            Events::Enter(event) => Ok(ExtractedEvent {
                tx: self.tx,
                receipt: self.receipt,
                log_index: self.log_index,
                event,
            }),
            _ => Err(self),
        }
    }

    /// True if the event is an [`Zenith::BlockSubmitted`].
    pub const fn is_block_submitted(&self) -> bool {
        self.event.is_block_submitted()
    }

    /// Get a reference to the inner event, if it is an [`BlockSubmitted`].
    ///
    /// [`BlockSubmitted`]: Zenith::BlockSubmitted
    pub const fn as_block_submitted(&self) -> Option<&Zenith::BlockSubmitted> {
        self.event.as_block_submitted()
    }

    /// Attempt to convert this event into an [`BlockSubmitted`]. If the event
    /// is not an [`BlockSubmitted`], it will be returned as an error.
    ///
    /// [`BlockSubmitted`]: Zenith::BlockSubmitted
    pub fn try_into_block_submitted(
        self,
    ) -> Result<ExtractedEvent<'a, Zenith::BlockSubmitted>, Self> {
        match self.event {
            Events::BlockSubmitted(event) => Ok(ExtractedEvent {
                tx: self.tx,
                receipt: self.receipt,
                log_index: self.log_index,
                event,
            }),
            _ => Err(self),
        }
    }

    /// True if the event is an [`Transactor::Transact`].
    pub const fn is_transact(&self) -> bool {
        self.event.is_transact()
    }

    /// Get a reference to the inner event, if it is an [`Transactor::Transact`].
    pub const fn as_transact(&self) -> Option<&Transactor::Transact> {
        self.event.as_transact()
    }

    /// Attempt to convert this event into an [`Transact`]. If the event is not
    /// an [`Transact`], it will be returned as an error.
    ///
    /// [`Transact`]: Transactor::Transact
    pub fn try_into_transact(self) -> Result<ExtractedEvent<'a, Transactor::Transact>, Self> {
        match self.event {
            Events::Transact(event) => Ok(ExtractedEvent {
                tx: self.tx,
                receipt: self.receipt,
                log_index: self.log_index,
                event,
            }),
            _ => Err(self),
        }
    }

    /// True if the event is an [`RollupOrders::Filled`].
    pub const fn is_filled(&self) -> bool {
        self.event.is_filled()
    }

    /// Get a reference to the inner event, if it is an [`RollupOrders::Filled`].
    pub const fn as_filled(&self) -> Option<&RollupOrders::Filled> {
        self.event.as_filled()
    }

    /// Attempt to convert this event into an [`RollupOrders::Filled`]. If the
    /// event is not an [`RollupOrders::Filled`], it will be returned as an
    /// error.
    pub fn try_into_filled(self) -> Result<ExtractedEvent<'a, RollupOrders::Filled>, Self> {
        match self.event {
            Events::Filled(event) => Ok(ExtractedEvent {
                tx: self.tx,
                receipt: self.receipt,
                log_index: self.log_index,
                event,
            }),
            _ => Err(self),
        }
    }
}

impl ExtractedEvent<'_, Zenith::BlockSubmitted> {
    /// Get the header of the block that was submitted.
    pub fn ru_header(&self, host_block_number: u64) -> Zenith::BlockHeader {
        Zenith::BlockHeader::from_block_submitted(self.event, U256::from(host_block_number))
    }

    /// True if the transaction is an EIP-4844 transaction.
    pub fn is_eip4844(&self) -> bool {
        self.tx.is_eip4844()
    }
}
