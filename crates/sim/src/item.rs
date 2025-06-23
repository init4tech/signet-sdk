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
    Bundle {
        /// The bundle to be simulated.
        bundle: SignetEthBundle,
        /// The identifier for the bundle.
        identifier: SimIdentifier,
    },

    /// A transaction to be simulated.
    Tx {
        /// The transaction to be simulated.
        tx: TxEnvelope,
        /// The identifier for the transaction.
        identifier: SimIdentifier,
    },
}

impl From<SignetEthBundle> for SimItem {
    fn from(bundle: SignetEthBundle) -> Self {
        let id = bundle.replacement_uuid().expect("accepted bundles should have IDs").to_string();
        Self::Bundle { bundle, identifier: SimIdentifier::Bundle(id) }
    }
}

impl From<TxEnvelope> for SimItem {
    fn from(tx: TxEnvelope) -> Self {
        let id = *tx.hash();
        Self::Tx { tx, identifier: SimIdentifier::Tx(id) }
    }
}

impl SimItem {
    /// Get the bundle if it is a bundle.
    pub const fn as_bundle(&self) -> Option<&SignetEthBundle> {
        match self {
            Self::Bundle { bundle, .. } => Some(bundle),
            Self::Tx { .. } => None,
        }
    }

    /// Get the transaction if it is a transaction.
    pub const fn as_tx(&self) -> Option<&TxEnvelope> {
        match self {
            Self::Bundle { .. } => None,
            Self::Tx { tx, .. } => Some(tx),
        }
    }

    /// Calculate the maximum gas fee payable, this may be used as a heuristic
    /// to determine simulation order.
    pub fn calculate_total_fee(&self, basefee: u64) -> u128 {
        match self {
            Self::Bundle { bundle, .. } => {
                let mut total_tx_fee = 0;
                for tx in bundle.bundle.txs.iter() {
                    let Ok(tx) = TxEnvelope::decode_2718(&mut tx.as_ref()) else {
                        continue;
                    };
                    total_tx_fee += tx.effective_gas_price(Some(basefee)) * tx.gas_limit() as u128;
                }
                total_tx_fee
            }
            Self::Tx { tx, .. } => tx.effective_gas_price(Some(basefee)) * tx.gas_limit() as u128,
        }
    }
}

// Testing functions
impl SimItem {
    /// Returns a unique identifier for this item, which can be used to
    /// distinguish it from other items.
    pub const fn identifier(&self) -> &SimIdentifier {
        match self {
            Self::Bundle { identifier, .. } => identifier,
            Self::Tx { identifier, .. } => identifier,
        }
    }
}

/// A simulation cache item identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SimIdentifier {
    /// A bundle identifier.
    Bundle(String),
    /// A transaction identifier.
    Tx(TxHash),
}

impl From<TxHash> for SimIdentifier {
    fn from(tx_hash: TxHash) -> Self {
        Self::Tx(tx_hash)
    }
}

impl SimIdentifier {
    /// Create a new [`SimIdentifier::Bundle`].
    pub fn bundle(id: impl Into<String>) -> Self {
        Self::Bundle(id.into())
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
}
