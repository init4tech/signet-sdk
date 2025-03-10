use alloy::{
    eips::Typed2718,
    primitives::{Log, TxHash, B256, U256},
};
use reth::primitives::{Receipt, TransactionSigned};
use zenith_types::{Passage, RollupOrders, Transactor, Zenith};

use crate::Events;

/// Extraction Result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedEvent<'a, T = Events> {
    /// The transaction that caused the event
    pub tx: &'a TransactionSigned,
    /// The transaction hash.
    ///
    /// NB: this is memoized here because reth doesn't produce a hash for the
    /// transaction during historical sync
    pub tx_hash: B256,
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
        self.tx_hash
    }

    /// Borrow the raw log from the receipt.
    pub fn raw_log(&self) -> &Log {
        &self.receipt.logs[self.log_index]
    }
}

impl<'a> ExtractedEvent<'a, Events> {
    /// True if the event is an [`Passage::EnterToken`].
    pub fn is_enter_token(&self) -> bool {
        self.event.is_enter_token()
    }

    /// Attempt to convert this event into an [`Passage::EnterToken`]. If the
    /// event is not an [`Passage::EnterToken`], it will return `None`.
    pub fn as_enter_token(&self) -> Option<&Passage::EnterToken> {
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
                tx_hash: self.tx_hash,
                receipt: self.receipt,
                log_index: self.log_index,
                event,
            }),
            _ => Err(self),
        }
    }

    /// True if the event is an [`Passage::Enter`].
    pub fn is_enter(&self) -> bool {
        self.event.is_enter()
    }

    /// Get a refernce to the inner event, if it is an [`Enter`].
    ///
    /// [`Enter`]: Passage::Enter
    pub fn as_enter(&self) -> Option<&Passage::Enter> {
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
                tx_hash: self.tx_hash,
                receipt: self.receipt,
                log_index: self.log_index,
                event,
            }),
            _ => Err(self),
        }
    }

    /// True if the event is an [`Zenith::BlockSubmitted`].
    pub fn is_block_submitted(&self) -> bool {
        self.event.is_block_submitted()
    }

    /// Get a refernce to the inner event, if it is an [`BlockSubmitted`].
    ///
    /// [`BlockSubmitted`]: Zenith::BlockSubmitted
    pub fn as_block_submitted(&self) -> Option<&Zenith::BlockSubmitted> {
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
                tx_hash: self.tx_hash,
                receipt: self.receipt,
                log_index: self.log_index,
                event,
            }),
            _ => Err(self),
        }
    }

    /// True if the event is an [`Transactor::Transact`].
    pub fn is_transact(&self) -> bool {
        self.event.is_transact()
    }

    /// Get a refernce to the inner event, if it is an [`Transactor::Transact`].
    pub fn as_transact(&self) -> Option<&Transactor::Transact> {
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
                tx_hash: self.tx_hash,
                receipt: self.receipt,
                log_index: self.log_index,
                event,
            }),
            _ => Err(self),
        }
    }

    /// True if the event is an [`RollupOrders::Filled`].
    pub fn is_filled(&self) -> bool {
        self.event.is_filled()
    }

    /// Get a refernce to the inner event, if it is an [`RollupOrders::Filled`].
    pub fn as_filled(&self) -> Option<&RollupOrders::Filled> {
        self.event.as_filled()
    }

    /// Attempt to convert this event into an [`RollupOrders::Filled`]. If the
    /// event is not an [`RollupOrders::Filled`], it will be returned as an
    /// error.
    pub fn try_into_filled(self) -> Result<ExtractedEvent<'a, RollupOrders::Filled>, Self> {
        match self.event {
            Events::Filled(event) => Ok(ExtractedEvent {
                tx: self.tx,
                tx_hash: self.tx_hash,
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
