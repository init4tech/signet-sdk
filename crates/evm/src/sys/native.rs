use crate::{sys::MintNativeSysLog, EvmNeedsTx, RunTxResult, SignetDriver};
use alloy::{
    consensus::{ReceiptEnvelope, TxEip1559, TxReceipt},
    primitives::{Address, Log, U256},
};
use signet_extract::{Extractable, ExtractedEvent};
use signet_types::{
    constants::MINTER_ADDRESS,
    primitives::{Transaction, TransactionSigned},
    MagicSig,
};
use signet_zenith::Passage;
use trevm::{
    helpers::Ctx,
    revm::{context::result::EVMError, Database, DatabaseCommit, Inspector},
    trevm_try, MIN_TRANSACTION_GAS,
};

/// System transaction to mint native tokens.
#[derive(Debug, Clone, Copy)]
pub struct MintNative {
    /// The address that will receive the minted tokens.
    recipient: Address,
    /// The amount of native tokens to mint.
    amount: U256,

    /// The magic signature for the mint.
    magic_sig: MagicSig,

    /// The nonce of the mint transaction.
    nonce: u64,
    /// The rollup chain ID.
    rollup_chain_id: u64,
}

impl MintNative {
    /// Create a new [`MintNative`] instance from an [`ExtractedEvent`]
    /// containing a [`Passage::EnterToken`] event.
    pub fn new<R: TxReceipt<Log = Log>>(
        nonce: u64,
        event: &ExtractedEvent<'_, R, Passage::EnterToken>,
    ) -> Self {
        Self {
            recipient: event.event.recipient(),
            amount: event.event.amount(),
            magic_sig: event.magic_sig(),
            nonce,
            rollup_chain_id: event.rollup_chain_id(),
        }
    }

    /// Create a new [`Log`] for the [`MintNative`] operation.
    pub const fn to_log(&self) -> MintNativeSysLog {
        MintNativeSysLog {
            txHash: self.magic_sig.txid,
            logIndex: self.magic_sig.event_idx as u64,
            recipient: self.recipient,
            amount: self.amount,
        }
    }

    /// Convert the [`MintNative`] instance into a [`TransactionSigned`].
    pub fn to_transaction(&self) -> TransactionSigned {
        TransactionSigned::new_unhashed(
            Transaction::Eip1559(TxEip1559 {
                chain_id: self.rollup_chain_id,
                nonce: self.nonce,
                gas_limit: MIN_TRANSACTION_GAS,
                max_fee_per_gas: 0,
                max_priority_fee_per_gas: 0,
                to: self.recipient.into(),
                value: self.amount,
                access_list: Default::default(),
                input: Default::default(),
            }),
            self.magic_sig.into(),
        )
    }
}

impl<'a, 'b, C> SignetDriver<'a, 'b, C>
where
    C: Extractable,
{
    fn mint_native_receipt(&self, mint: &MintNative) -> ReceiptEnvelope {
        let cumulative_gas_used = self.cumulative_gas_used().saturating_add(MIN_TRANSACTION_GAS);

        ReceiptEnvelope::Eip1559(
            alloy::consensus::Receipt {
                status: true.into(),
                cumulative_gas_used,
                logs: vec![mint.to_log().into()],
            }
            .with_bloom(),
        )
    }

    pub(crate) fn mint_native<Db, Insp>(
        &mut self,
        mut trevm: EvmNeedsTx<Db, Insp>,
        mint: &MintNative,
    ) -> RunTxResult<Self, Db, Insp>
    where
        Db: Database + DatabaseCommit,
        Insp: Inspector<Ctx<Db>>,
    {
        // Increase the balance
        trevm_try!(
            trevm
                .try_increase_balance_unchecked(mint.recipient, mint.amount)
                .map_err(EVMError::Database),
            trevm
        );

        // push receipt and transaction to the block
        self.processed.push(mint.to_transaction());
        self.output.push_result(self.mint_native_receipt(mint), MINTER_ADDRESS);

        Ok(trevm)
    }
}
