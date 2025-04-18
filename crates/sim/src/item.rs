use alloy::{
    consensus::{Signed, Transaction, TxEip1559, TxEnvelope},
    eips::Decodable2718,
    signers::Signature,
};
use signet_bundle::SignetEthBundle;

/// An item that can be simulated.
#[derive(Debug, Clone)]
pub enum SimItem {
    /// A bundle to be simulated.
    Bundle(SignetEthBundle),

    /// A transaction to be simulated.
    Tx(TxEnvelope),
}

impl From<SignetEthBundle> for SimItem {
    fn from(bundle: SignetEthBundle) -> Self {
        Self::Bundle(bundle)
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
    pub fn calculate_total_fee(&self) -> u128 {
        match self {
            Self::Bundle(bundle) => {
                let mut total_tx_fee = 0;
                for tx in bundle.bundle.txs.iter() {
                    let Ok(tx) = TxEnvelope::decode_2718(&mut tx.as_ref()) else {
                        continue;
                    };
                    total_tx_fee += tx.effective_gas_price(None) * tx.gas_limit() as u128;
                }
                total_tx_fee
            }
            Self::Tx(tx) => tx.effective_gas_price(None) * tx.gas_limit() as u128,
        }
    }
}

#[cfg(any(test, feature = "test-utils"))]
impl SimItem {
    /// Create an invalid test item. This will be a [`TxEnvelope`] containing
    /// an EIP-1559 transaction with an invalid signature and hash.
    pub fn invalid_test_item() -> Self {
        TxEnvelope::Eip1559(Signed::new_unchecked(
            TxEip1559::default(),
            Signature::test_signature(),
            Default::default(),
        ))
        .into()
    }
}
