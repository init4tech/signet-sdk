use alloy::{
    consensus::{Transaction, TxEnvelope},
    eips::Decodable2718,
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
}
