use crate::CacheError;
use alloy::{
    consensus::{
        transaction::{Recovered, SignerRecoverable},
        Transaction, TxEnvelope,
    },
    primitives::TxHash,
};
use signet_bundle::{RecoveredBundle, SignetEthBundle};
use std::{
    borrow::{Borrow, Cow},
    hash::Hash,
};

/// An item that can be simulated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimItem {
    /// A bundle to be simulated.
    Bundle(Box<RecoveredBundle>),
    /// A transaction to be simulated.
    Tx(Box<Recovered<TxEnvelope>>),
}

impl TryFrom<SignetEthBundle> for SimItem {
    type Error = CacheError;

    fn try_from(bundle: SignetEthBundle) -> Result<Self, Self::Error> {
        bundle.try_into_recovered().map_err(CacheError::BundleRecover).and_then(TryInto::try_into)
    }
}

impl TryFrom<RecoveredBundle> for SimItem {
    type Error = CacheError;

    fn try_from(bundle: RecoveredBundle) -> Result<Self, Self::Error> {
        if bundle.replacement_uuid().is_some() {
            Ok(Self::Bundle(bundle.into()))
        } else {
            Err(CacheError::BundleWithoutReplacementUuid)
        }
    }
}

impl From<Recovered<TxEnvelope>> for SimItem {
    fn from(tx: Recovered<TxEnvelope>) -> Self {
        Self::Tx(tx.into())
    }
}

impl TryFrom<TxEnvelope> for SimItem {
    type Error = CacheError;

    fn try_from(tx: TxEnvelope) -> Result<Self, Self::Error> {
        tx.try_into_recovered().map_err(Into::into).map(Self::from)
    }
}

impl SimItem {
    /// Get the bundle if it is a bundle.
    pub const fn as_bundle(&self) -> Option<&RecoveredBundle> {
        match self {
            Self::Bundle(bundle) => Some(bundle),
            Self::Tx(_) => None,
        }
    }

    /// Get the transaction if it is a transaction.
    pub const fn as_tx(&self) -> Option<&Recovered<TxEnvelope>> {
        match self {
            Self::Bundle(_) => None,
            Self::Tx(tx) => Some(tx),
        }
    }

    /// Calculate the maximum gas fee payable, this may be used as a heuristic
    /// to determine simulation order.
    pub fn calculate_total_fee(&self, basefee: u64) -> u128 {
        match self {
            Self::Bundle(bundle) => {
                let mut total_tx_fee = 0;
                for tx in bundle.txs() {
                    total_tx_fee += tx.effective_gas_price(Some(basefee)) * tx.gas_limit() as u128;
                }
                total_tx_fee
            }
            Self::Tx(tx) => tx.effective_gas_price(Some(basefee)) * tx.gas_limit() as u128,
        }
    }
}

// Testing functions
impl SimItem {
    /// Returns a unique identifier for this item, which can be used to
    /// distinguish it from other items.
    pub fn identifier(&self) -> SimIdentifier<'_> {
        match self {
            Self::Bundle(bundle) => {
                SimIdentifier::Bundle(Cow::Borrowed(bundle.replacement_uuid().unwrap()))
            }
            Self::Tx(tx) => SimIdentifier::Tx(*tx.inner().hash()),
        }
    }

    /// Returns an unique, owned identifier for this item.
    pub fn identifier_owned(&self) -> SimIdentifier<'static> {
        match self {
            Self::Bundle(bundle) => {
                SimIdentifier::Bundle(Cow::Owned(bundle.replacement_uuid().unwrap().to_string()))
            }
            Self::Tx(tx) => SimIdentifier::Tx(*tx.inner().hash()),
        }
    }
}

/// A simulation cache item identifier.
#[derive(Debug, Clone)]
pub enum SimIdentifier<'a> {
    /// A bundle identifier.
    Bundle(Cow<'a, str>),
    /// A transaction identifier.
    Tx(TxHash),
}

impl core::fmt::Display for SimIdentifier<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bundle(id) => write!(f, "{id}"),
            Self::Tx(id) => write!(f, "{id}"),
        }
    }
}

impl PartialEq for SimIdentifier<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes().eq(other.as_bytes())
    }
}

impl Eq for SimIdentifier<'_> {}

impl Hash for SimIdentifier<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

impl Borrow<[u8]> for SimIdentifier<'_> {
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<TxHash> for SimIdentifier<'_> {
    fn from(tx_hash: TxHash) -> Self {
        Self::Tx(tx_hash)
    }
}

impl SimIdentifier<'_> {
    /// Create a new [`SimIdentifier::Bundle`].
    pub const fn bundle<'a>(id: Cow<'a, str>) -> SimIdentifier<'a> {
        SimIdentifier::Bundle(id)
    }

    /// Create a new [`SimIdentifier::Tx`].
    pub const fn tx(id: TxHash) -> Self {
        Self::Tx(id)
    }

    /// Check if this identifier is a bundle.
    pub const fn is_bundle(&self) -> bool {
        matches!(self, Self::Bundle(_))
    }

    /// Check if this identifier is a transaction.
    pub const fn is_tx(&self) -> bool {
        matches!(self, Self::Tx(_))
    }

    /// Get the identifier as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Bundle(id) => id.as_bytes(),
            Self::Tx(id) => id.as_ref(),
        }
    }
}
