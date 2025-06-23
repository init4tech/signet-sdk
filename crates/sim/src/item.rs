use std::sync::OnceLock;

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
        identifier: OnceLock<SimIdentifier>,
    },

    /// A transaction to be simulated.
    Tx {
        /// The transaction to be simulated.
        tx: TxEnvelope,
        /// The identifier for the transaction.
        identifier: OnceLock<SimIdentifier>,
    },
}

impl From<SignetEthBundle> for SimItem {
    fn from(bundle: SignetEthBundle) -> Self {
        Self::Bundle { bundle, identifier: OnceLock::new() }
    }
}

impl From<TxEnvelope> for SimItem {
    fn from(tx: TxEnvelope) -> Self {
        Self::Tx { tx, identifier: OnceLock::new() }
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
    /// Create an invalid test item. This will be a [`TxEnvelope`] containing
    /// an EIP-1559 transaction with an invalid signature and hash.
    #[doc(hidden)]
    pub fn invalid_item() -> Self {
        TxEnvelope::Eip1559(alloy::consensus::Signed::new_unchecked(
            alloy::consensus::TxEip1559::default(),
            alloy::signers::Signature::test_signature(),
            Default::default(),
        ))
        .into()
    }

    /// Create an invalid test item with a given gas limit and max priority fee
    /// per gas. As [`Self::invalid_test_item`] but with a custom gas limit and
    /// `max_priority_fee_per_gas`.
    #[doc(hidden)]
    pub fn invalid_item_with_score(gas_limit: u64, mpfpg: u128) -> Self {
        let tx = alloy::consensus::TxEip1559 {
            gas_limit,
            max_priority_fee_per_gas: mpfpg,
            max_fee_per_gas: alloy::consensus::constants::GWEI_TO_WEI as u128,
            ..Default::default()
        };

        let tx = TxEnvelope::Eip1559(alloy::consensus::Signed::new_unchecked(
            tx,
            alloy::signers::Signature::test_signature(),
            Default::default(),
        ));
        tx.into()
    }

    /// Create an invalid test item with a given gas limit and max priority fee
    /// per gas, and a random tx hash. As [`Self::invalid_test_item`] but with
    /// a custom gas limit and `max_priority_fee_per_gas`, and a random hash
    /// to avoid getting deduped by the seen items cache.
    #[doc(hidden)]
    #[cfg(test)]
    pub fn invalid_item_with_score_and_hash(gas_limit: u64, mpfpg: u128) -> Self {
        use alloy::primitives::B256;

        let tx = alloy::consensus::TxEip1559 {
            gas_limit,
            max_priority_fee_per_gas: mpfpg,
            max_fee_per_gas: alloy::consensus::constants::GWEI_TO_WEI as u128,
            ..Default::default()
        };

        let tx = TxEnvelope::Eip1559(alloy::consensus::Signed::new_unchecked(
            tx,
            alloy::signers::Signature::test_signature(),
            B256::random(),
        ));

        tx.into()
    }

    /// Returns a unique identifier for this item, which can be used to
    /// distinguish it from other items.
    pub fn identifier(&self) -> &SimIdentifier {
        match self {
            Self::Bundle { bundle, identifier } => identifier.get_or_init(|| {
                SimIdentifier::bundle(bundle.bundle.replacement_uuid.clone().unwrap_or_default())
            }),
            Self::Tx { tx, identifier } => identifier.get_or_init(|| SimIdentifier::tx(*tx.hash())),
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

    pub const fn is_bundle(&self) -> bool {
        matches!(self, Self::Bundle(_))
    }

    pub const fn is_tx(&self) -> bool {
        matches!(self, Self::Tx(_))
    }
}
