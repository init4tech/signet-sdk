//! Bespoke conversion utilities for converting between alloy and reth types.

use alloy::consensus::ReceiptEnvelope;
use signet_evm::ExecutionOutcome;
use signet_types::primitives::{RecoveredBlock, SealedBlock};

/// Utility trait to convert a type to a Reth primitive type.
/// This is used mainly where we need to convert to a reth primitive type
/// because reth does not support the alloy equivalents.
pub trait ToRethPrimitive {
    /// The Reth primitive type that the type can be converted to.
    type RethPrimitive;

    /// Convert the type to a Reth primitive type.
    fn to_reth(self) -> Self::RethPrimitive;
}

// Reth does not preserve envelope status for receipts, so
// the DB model will not support envelopes.
impl ToRethPrimitive for ReceiptEnvelope {
    type RethPrimitive = reth::primitives::Receipt;

    fn to_reth(self) -> Self::RethPrimitive {
        let success = self.is_success();
        let cumulative_gas_used = self.cumulative_gas_used();
        let tx_type = match self.tx_type() {
            alloy::consensus::TxType::Legacy => reth::primitives::TxType::Legacy,
            alloy::consensus::TxType::Eip2930 => reth::primitives::TxType::Eip2930,
            alloy::consensus::TxType::Eip1559 => reth::primitives::TxType::Eip1559,
            alloy::consensus::TxType::Eip4844 => reth::primitives::TxType::Eip4844,
            alloy::consensus::TxType::Eip7702 => reth::primitives::TxType::Eip7702,
        };

        let r = match self {
            ReceiptEnvelope::Legacy(r)
            | ReceiptEnvelope::Eip2930(r)
            | ReceiptEnvelope::Eip1559(r)
            | ReceiptEnvelope::Eip4844(r) => r,
            _ => panic!("unsupported receipt type"),
        };

        reth::primitives::Receipt { tx_type, success, cumulative_gas_used, logs: r.receipt.logs }
    }
}

impl ToRethPrimitive for SealedBlock {
    type RethPrimitive = reth::primitives::SealedBlock<reth::primitives::Block>;

    fn to_reth(self) -> Self::RethPrimitive {
        let (hash, header) = self.header.split();
        reth::primitives::SealedBlock::new_unchecked(
            reth::primitives::Block::new(header, self.body),
            hash,
        )
    }
}

impl ToRethPrimitive for RecoveredBlock {
    type RethPrimitive = reth::primitives::RecoveredBlock<reth::primitives::Block>;

    fn to_reth(self) -> Self::RethPrimitive {
        let hash = self.block.header.hash();
        reth::primitives::RecoveredBlock::new(self.block.to_reth().into_block(), self.senders, hash)
    }
}

impl ToRethPrimitive for crate::chain::Chain {
    type RethPrimitive = reth::providers::Chain;

    fn to_reth(self) -> Self::RethPrimitive {
        reth::providers::Chain::new(self.blocks.to_reth(), self.execution_outcome.to_reth(), None)
    }
}

impl<T> ToRethPrimitive for Vec<T>
where
    T: ToRethPrimitive,
{
    type RethPrimitive = Vec<T::RethPrimitive>;

    fn to_reth(self) -> Self::RethPrimitive {
        self.into_iter().map(ToRethPrimitive::to_reth).collect()
    }
}

impl ToRethPrimitive for ExecutionOutcome {
    type RethPrimitive = reth::providers::ExecutionOutcome;

    fn to_reth(self) -> Self::RethPrimitive {
        let (bundle, receipts, first_block) = self.into_parts();

        reth::providers::ExecutionOutcome {
            bundle,
            receipts: receipts.into_iter().map(ToRethPrimitive::to_reth).collect(),
            first_block,
            requests: vec![],
        }
    }
}
