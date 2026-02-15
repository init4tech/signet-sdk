//! Typed Ethereum transaction receipt.

use alloy::consensus::{Receipt as AlloyReceipt, TxType};

/// Typed ethereum transaction receipt.
///
/// Receipt containing the result of transaction execution, paired with the
/// transaction type discriminant.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Receipt {
    /// Receipt type.
    pub tx_type: TxType,
    /// The actual receipt data.
    pub inner: AlloyReceipt,
}
