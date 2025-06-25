use std::{
    borrow::{Borrow, Cow},
    hash::Hash,
};

use alloy::{
    consensus::{Transaction, TxEnvelope},
    eips::Decodable2718,
    primitives::TxHash,
};
use signet_bundle::SignetEthBundle;

/// An item that can be simulated.
#[derive(Debug, Clone, PartialEq)]
pub enum SimItem {
    /// A bundle to be simulated.
    Bundle(SignetEthBundle),
    /// A transaction to be simulated.
    Tx(TxEnvelope),
}

impl TryFrom<SignetEthBundle> for SimItem {
    type Error = crate::CacheError;

    fn try_from(bundle: SignetEthBundle) -> Result<Self, Self::Error> {
        if bundle.replacement_uuid().is_some() {
            Ok(Self::Bundle(bundle))
        } else {
            Err(crate::CacheError::BundleWithoutReplacementUuid)
        }
    }
}

impl From<TxEnvelope> for SimItem {
    fn from(tx: TxEnvelope) -> Self {
        Self::Tx(tx)
    }
}

impl SimItem {
    /// Get the bundle if it is a bundle.
    pub const fn as_bundle(&self) -> Option<&SignetEthBundle> {
        match self {
            Self::Bundle(bundle) => Some(bundle),
            Self::Tx(_) => None,
        }
    }

    /// Get the transaction if it is a transaction.
    pub const fn as_tx(&self) -> Option<&TxEnvelope> {
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
                for tx in bundle.bundle.txs.iter() {
                    let Ok(tx) = TxEnvelope::decode_2718(&mut tx.as_ref()) else {
                        continue;
                    };
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
            Self::Tx(tx) => SimIdentifier::Tx(*tx.hash()),
        }
    }

    /// Returns an unique, owned identifier for this item.
    pub fn identifier_owned(&self) -> SimIdentifier<'static> {
        match self {
            Self::Bundle(bundle) => {
                SimIdentifier::Bundle(Cow::Owned(bundle.replacement_uuid().unwrap().to_string()))
            }
            Self::Tx(tx) => SimIdentifier::Tx(*tx.hash()),
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