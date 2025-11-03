use core::fmt;

use crate::Events;
use alloy::{
    consensus::{TxEip1559, TxReceipt},
    primitives::{Log, TxHash, U256},
};
use signet_types::{
    primitives::{Transaction, TransactionSigned},
    MagicSig, MagicSigInfo,
};
use signet_zenith::{Passage, RollupOrders, Transactor, Zenith};

/// A single event extracted from the host chain.
///
/// This struct contains a reference to the transaction that caused the event,
/// the receipt that the event was extracted from, the index of the log in the
/// receipt's logs, and the extracted event itself.
///
/// Events may be either the enum type [`Events`], or a specific event type.
#[derive(Copy, PartialEq, Eq)]
pub struct ExtractedEvent<'a, R, E = Events> {
    /// The transaction that caused the event
    pub tx: &'a TransactionSigned,
    /// The receipt that the event was extracted from.
    pub receipt: &'a R,
    /// The index of the log in the receipt's logs.
    pub log_index: usize,
    /// The extracted event.
    pub event: E,
}

impl<R, E> fmt::Debug for ExtractedEvent<'_, R, E>
where
    E: Into<Events> + fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtractedEvent")
            .field("tx", &self.tx)
            .field("log_index", &self.log_index)
            .field("event", &self.event)
            .finish_non_exhaustive()
    }
}

// NB: manual impl because of incorrect auto-derive bound on `R: Clone`
impl<R, E> Clone for ExtractedEvent<'_, R, E>
where
    E: Clone,
{
    fn clone(&self) -> Self {
        ExtractedEvent {
            tx: self.tx,
            receipt: self.receipt,
            log_index: self.log_index,
            event: self.event.clone(),
        }
    }
}

impl<R, E> std::ops::Deref for ExtractedEvent<'_, R, E> {
    type Target = E;

    fn deref(&self) -> &Self::Target {
        &self.event
    }
}

impl<R, E> ExtractedEvent<'_, R, E> {
    /// Get the transaction hash of the extracted event.
    pub fn tx_hash(&self) -> TxHash {
        *self.tx.hash()
    }
}

impl<R, E> ExtractedEvent<'_, R, E>
where
    R: TxReceipt<Log = Log>,
{
    /// Borrow the raw log from the receipt.
    pub fn raw_log(&self) -> &Log {
        &self.receipt.logs()[self.log_index]
    }
}

impl<'a, R> ExtractedEvent<'a, R, Events> {
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
    pub fn try_into_enter_token(self) -> Result<ExtractedEvent<'a, R, Passage::EnterToken>, Self> {
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

    /// Get a refernce to the inner event, if it is an [`Enter`].
    ///
    /// [`Enter`]: Passage::Enter
    pub const fn as_enter(&self) -> Option<&Passage::Enter> {
        self.event.as_enter()
    }

    /// Attempt to convert this event into an [`Enter`]. If the event is not an
    /// [`Enter`], it will be returned as an error.
    ///
    /// [`Enter`]: Passage::Enter
    pub fn try_into_enter(self) -> Result<ExtractedEvent<'a, R, Passage::Enter>, Self> {
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

    /// Get a refernce to the inner event, if it is an [`BlockSubmitted`].
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
    ) -> Result<ExtractedEvent<'a, R, Zenith::BlockSubmitted>, Self> {
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

    /// Get a refernce to the inner event, if it is an [`Transactor::Transact`].
    pub const fn as_transact(&self) -> Option<&Transactor::Transact> {
        self.event.as_transact()
    }

    /// Attempt to convert this event into an [`Transact`]. If the event is not
    /// an [`Transact`], it will be returned as an error.
    ///
    /// [`Transact`]: Transactor::Transact
    pub fn try_into_transact(self) -> Result<ExtractedEvent<'a, R, Transactor::Transact>, Self> {
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

    /// Get a refernce to the inner event, if it is an [`RollupOrders::Filled`].
    pub const fn as_filled(&self) -> Option<&RollupOrders::Filled> {
        self.event.as_filled()
    }

    /// Attempt to convert this event into an [`RollupOrders::Filled`]. If the
    /// event is not an [`RollupOrders::Filled`], it will be returned as an
    /// error.
    pub fn try_into_filled(self) -> Result<ExtractedEvent<'a, R, RollupOrders::Filled>, Self> {
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

impl<R> ExtractedEvent<'_, R, Transactor::Transact> {
    /// Create a magic signature for the transact event, containing sender
    /// information.
    pub fn magic_sig(&self) -> MagicSig {
        MagicSig {
            ty: MagicSigInfo::Transact { sender: self.event.host_sender() },
            txid: self.tx_hash(),
            event_idx: self.log_index,
        }
    }

    /// Create the signature for the transact event.
    fn signature(&self) -> alloy::primitives::Signature {
        self.magic_sig().into()
    }

    /// Make the transaction that corresponds to this transact event,
    /// using the provided nonce.
    pub fn make_transaction(&self, nonce: u64) -> TransactionSigned {
        TransactionSigned::new_unhashed(
            Transaction::Eip1559(TxEip1559 {
                chain_id: self.rollup_chain_id(),
                nonce,
                gas_limit: self.gas.to::<u64>(),
                max_fee_per_gas: self.maxFeePerGas.to::<u128>(),
                max_priority_fee_per_gas: 0,
                to: self.to.into(),
                value: self.value,
                access_list: Default::default(),
                input: self.data.clone(),
            }),
            self.signature(),
        )
    }
}

impl<R> ExtractedEvent<'_, R, Passage::Enter> {
    /// Get the magic signature for the enter event.
    pub fn magic_sig(&self) -> MagicSig {
        MagicSig { ty: MagicSigInfo::Enter, txid: self.tx_hash(), event_idx: self.log_index }
    }
}

impl<R> ExtractedEvent<'_, R, Passage::EnterToken> {
    /// Get the magic signature for the enter token event.
    pub fn magic_sig(&self) -> MagicSig {
        MagicSig { ty: MagicSigInfo::EnterToken, txid: self.tx_hash(), event_idx: self.log_index }
    }
}

impl<R> ExtractedEvent<'_, R, Zenith::BlockSubmitted> {
    /// Get the header of the block that was submitted.
    pub fn ru_header(&self, host_block_number: u64) -> Zenith::BlockHeader {
        Zenith::BlockHeader::from_block_submitted(self.event, U256::from(host_block_number))
    }

    /// True if the transaction is an EIP-4844 transaction.
    pub const fn is_eip4844(&self) -> bool {
        self.tx.is_eip4844()
    }
}
