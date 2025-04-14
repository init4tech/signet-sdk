use crate::BuiltBlock;
use alloy::{
    consensus::{Transaction, TxEnvelope},
    eips::Decodable2718,
};
use signet_bundle::SignetEthBundle;
use tokio::sync::oneshot;
use trevm::{Block, Cfg};

/// Instructions for the simulator.
pub enum SimInstruction {
    /// Enroll a bundle for simulation.
    AddItem { item: SimItem },

    /// Drop a bundle from the simulator by its UUID.
    DropByUuid { uuid: String },

    /// Update the configuration of the simulator. This should be used
    /// sparingly.
    UpdateCfg { cfg: Box<dyn Cfg>, on_update: oneshot::Sender<()> },

    /// Close the current block, returning the built block
    CloseBlock {
        next_block_env: Option<Box<dyn Block>>,
        finish_by: std::time::Instant,
        on_close: oneshot::Sender<BuiltBlock>,
    },
}

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
    pub fn as_bundle(&self) -> Option<&SignetEthBundle> {
        match self {
            Self::Bundle(bundle) => Some(bundle),
            Self::Tx(_) => None,
        }
    }

    /// Get the transaction if it is a transaction.
    pub fn as_tx(&self) -> Option<&TxEnvelope> {
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
