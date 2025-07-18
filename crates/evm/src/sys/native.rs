use crate::sys::{MintNativeSysLog, SysAction, SysBase};
use alloy::{
    consensus::{ReceiptEnvelope, TxEip1559, TxReceipt},
    primitives::{utils::format_ether, Address, Log, U256},
};
use signet_extract::ExtractedEvent;
use signet_types::{
    constants::MINTER_ADDRESS,
    primitives::{Transaction, TransactionSigned},
    MagicSig,
};
use signet_zenith::Passage;
use trevm::{
    helpers::Ctx,
    revm::{context::result::EVMError, Database, DatabaseCommit, Inspector},
    Trevm, MIN_TRANSACTION_GAS,
};

const ETH_DECIMALS: u8 = 18;

/// System transaction to mint native tokens.
#[derive(Debug, Clone, Copy)]
pub struct MintNative {
    /// The address that will receive the minted tokens.
    recipient: Address,

    /// The host USD record for the mint.
    decimals: u8,

    /// The amount of native tokens to mint.
    host_amount: U256,

    /// The magic signature for the mint.
    magic_sig: MagicSig,

    /// The nonce of the mint transaction.
    nonce: Option<u64>,
    /// The rollup chain ID.
    rollup_chain_id: u64,
}

impl MintNative {
    /// Create a new [`MintNative`] instance from an [`ExtractedEvent`]
    /// containing a [`Passage::EnterToken`] event.
    pub fn new<R: TxReceipt<Log = Log>>(
        event: &ExtractedEvent<'_, R, Passage::EnterToken>,
        decimals: u8,
    ) -> Self {
        Self {
            recipient: event.event.recipient(),
            decimals,
            host_amount: event.event.amount(),
            magic_sig: event.magic_sig(),
            nonce: None,
            rollup_chain_id: event.rollup_chain_id(),
        }
    }

    /// Create a new [`MintNative`] instance with a nonce.
    pub fn new_with_nonce<R: TxReceipt<Log = Log>>(
        event: &ExtractedEvent<'_, R, Passage::EnterToken>,
        decimals: u8,
        nonce: u64,
    ) -> Self {
        let mut mint = Self::new(event, decimals);
        mint.populate_nonce(nonce);
        mint
    }

    /// Create a new [`Log`] for the [`MintNative`] operation.
    fn make_sys_log(&self) -> MintNativeSysLog {
        MintNativeSysLog {
            txHash: self.magic_sig.txid,
            logIndex: self.magic_sig.event_idx as u64,
            recipient: self.recipient,
            amount: self.mint_amount(),
        }
    }

    /// Convert the [`MintNative`] instance into a [`TransactionSigned`].
    fn make_transaction(&self) -> TransactionSigned {
        TransactionSigned::new_unhashed(
            Transaction::Eip1559(TxEip1559 {
                chain_id: self.rollup_chain_id,
                nonce: self.nonce.expect("must be set"),
                gas_limit: MIN_TRANSACTION_GAS,
                max_fee_per_gas: 0,
                max_priority_fee_per_gas: 0,
                to: self.recipient.into(),
                value: self.mint_amount(),
                access_list: Default::default(),
                input: Default::default(),
            }),
            self.magic_sig.into(),
        )
    }

    /// Get the amount of native tokens to mint, adjusted for the decimals of
    /// the host USD record.
    pub fn mint_amount(&self) -> U256 {
        let decimals = self.decimals;
        adjust_decimals(self.host_amount, decimals, ETH_DECIMALS)
    }
}

impl SysBase for MintNative {
    fn name() -> &'static str {
        "MintNative"
    }

    fn description(&self) -> String {
        format!("Mint {} native tokens to {}", format_ether(self.mint_amount()), self.recipient)
    }

    fn has_nonce(&self) -> bool {
        self.nonce.is_some()
    }

    fn populate_nonce(&mut self, nonce: u64) {
        self.nonce = Some(nonce)
    }

    fn produce_transaction(&self) -> TransactionSigned {
        self.make_transaction()
    }

    fn produce_log(&self) -> Log {
        self.make_sys_log().into()
    }

    fn evm_sender(&self) -> Address {
        MINTER_ADDRESS
    }
}

impl SysAction for MintNative {
    fn apply<Db, Insp, State>(
        &self,
        evm: &mut Trevm<Db, Insp, State>,
    ) -> Result<(), EVMError<Db::Error>>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // Increase the balance of the recipient
        evm.try_increase_balance_unchecked(self.recipient, self.mint_amount())
            .map(drop)
            .map_err(EVMError::Database)
    }

    fn produce_receipt(&self, cumulative_gas_used: u64) -> ReceiptEnvelope {
        ReceiptEnvelope::Eip1559(
            alloy::consensus::Receipt {
                status: true.into(),
                cumulative_gas_used: cumulative_gas_used.saturating_add(MIN_TRANSACTION_GAS),
                logs: vec![self.make_sys_log().into()],
            }
            .with_bloom(),
        )
    }
}

/// Adjust the amount of tokens based on the decimals of the host USD record
/// and the target decimals.
///
/// This is done by either dividing or multiplying the host amount
/// by a power of 10, depending on whether the host decimals are greater than
/// or less than the target decimals.
fn adjust_decimals(amount: U256, decimals: u8, target_decimals: u8) -> U256 {
    if target_decimals == 0 || decimals == 0 {
        // If target decimals is 0, return the host amount unchanged
        return amount;
    }

    if decimals > target_decimals {
        let divisor_exp = decimals - target_decimals;
        let divisor = U256::from(10u64).pow(U256::from(divisor_exp));
        amount / divisor
    } else {
        let multiplier_exp = target_decimals.checked_sub(decimals).unwrap_or_default();
        let multiplier = U256::from(10u64).pow(U256::from(multiplier_exp));
        amount * multiplier
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::uint;

    #[test]
    fn mint_amount_math() {
        uint! {
            assert_eq!(adjust_decimals(10_U256.pow(6_U256), 6, 18), 10_U256.pow(18_U256));
            assert_eq!(adjust_decimals(10_U256.pow(18_U256), 18, 6), 10_U256.pow(6_U256));

            assert_eq!(adjust_decimals(10_U256.pow(6_U256), 6, 12), 10_U256.pow(12_U256));
            assert_eq!(adjust_decimals(10_U256.pow(12_U256), 12, 6), 10_U256.pow(6_U256));

            assert_eq!(adjust_decimals(10_U256.pow(18_U256), 18, 12), 10_U256.pow(12_U256));
            assert_eq!(adjust_decimals(10_U256.pow(12_U256), 12, 18), 10_U256.pow(18_U256));

            assert_eq!(adjust_decimals(10_U256.pow(6_U256), 6, 0), 10_U256.pow(6_U256));

            assert_eq!(adjust_decimals(10_U256.pow(18_U256), 3, 6), 10_U256.pow(21_U256));
            assert_eq!(adjust_decimals(10_U256.pow(21_U256), 6, 3), 10_U256.pow(18_U256));
            assert_eq!(adjust_decimals(10_U256.pow(18_U256), 6, 3), 10_U256.pow(15_U256));
        }
    }
}
