use crate::sys::{MeteredSysTx, SysBase, SysTx, TransactSysLog};
use alloy::{
    consensus::{TxEip1559, TxType},
    hex,
    primitives::{utils::format_ether, Address, Bytes, Log, TxKind, U256},
};
use signet_extract::ExtractedEvent;
use signet_types::{
    primitives::{Transaction, TransactionSigned},
    MagicSig, MagicSigInfo,
};
use signet_zenith::Transactor;
use trevm::{revm::context::TxEnv, Tx};

/// Shim to impl [`Tx`] for [`Transactor::Transact`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactSysTx {
    /// The rollup chain ID.
    rollup_chain_id: u64,
    /// The recipient address.
    to: Address,
    /// The transaction input data.
    data: Bytes,
    /// The transaction value.
    value: U256,
    /// The gas limit.
    gas: u64,
    /// The max fee per gas.
    max_fee_per_gas: u128,
    /// The nonce of the transaction.
    nonce: Option<u64>,
    /// The magic signature.
    magic_sig: MagicSig,
}

impl TransactSysTx {
    /// Instantiate a new [`TransactSysTx`].
    pub fn new<R>(transact: &ExtractedEvent<'_, R, Transactor::Transact>, aliased: bool) -> Self {
        Self {
            rollup_chain_id: transact.rollup_chain_id(),
            to: transact.event.to,
            data: transact.event.data.clone(),
            value: transact.event.value,
            gas: transact.event.gas.to::<u64>(),
            max_fee_per_gas: transact.event.maxFeePerGas.to::<u128>(),
            nonce: None,
            magic_sig: transact.magic_sig(aliased),
        }
    }

    /// Check if the sender was aliased (i.e. the sender is a smart contract on
    /// the host chain).
    pub fn is_aliased(&self) -> bool {
        match self.magic_sig.ty {
            MagicSigInfo::Transact { aliased, .. } => aliased,
            _ => unreachable!(),
        }
    }

    /// Create a fresh [`TransactionSigned`] with the current nonce.
    fn make_transaction(&self) -> TransactionSigned {
        TransactionSigned::new_unhashed(
            Transaction::Eip1559(TxEip1559 {
                chain_id: self.rollup_chain_id,
                nonce: self.nonce.expect("nonce must be set"),
                gas_limit: self.gas,
                max_fee_per_gas: self.max_fee_per_gas,
                max_priority_fee_per_gas: 0,
                to: self.to.into(),
                value: self.value,
                access_list: Default::default(),
                input: self.data.clone(),
            }),
            self.magic_sig.into(),
        )
    }

    /// Create a [`TransactSysLog`] from the filler.
    fn make_sys_log(&self) -> TransactSysLog {
        TransactSysLog {
            txHash: self.magic_sig.txid,
            logIndex: self.magic_sig.event_idx as u64,
            sender: self.evm_sender(),
            value: self.value,
            gas: U256::from(self.gas),
            maxFeePerGas: U256::from(self.max_fee_per_gas),
        }
    }
}

impl Tx for TransactSysTx {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        let TxEnv {
            tx_type,
            caller,
            gas_limit,
            gas_price,
            kind,
            value,
            data,
            nonce,
            chain_id,
            access_list,
            gas_priority_fee,
            blob_hashes,
            max_fee_per_blob_gas,
            authorization_list,
        } = tx_env;
        *tx_type = TxType::Eip1559 as u8;
        *caller = self.magic_sig.rollup_sender();
        *gas_limit = self.gas;
        *gas_price = self.max_fee_per_gas;
        *kind = self.to.into();
        *value = self.value;
        *data = self.data.clone();
        *nonce = self.nonce.expect("nonce must be set");
        *chain_id = Some(self.rollup_chain_id);
        *access_list = Default::default();
        *gas_priority_fee = Some(0);
        blob_hashes.clear();
        *max_fee_per_blob_gas = 0;
        authorization_list.clear();
    }
}

impl SysBase for TransactSysTx {
    fn name() -> &'static str {
        "TransactSysTx"
    }

    fn description(&self) -> String {
        let is_aliased = if self.is_aliased() { " (aliased)" } else { "" };

        format!(
            "Transact from {}{is_aliased} to {} with value {} and {} bytes of input data: `0x{}{}`",
            self.magic_sig.rollup_sender(),
            self.to,
            format_ether(self.value),
            self.data.len(),
            self.data.chunks(4).next().map(hex::encode).unwrap_or_default(),
            if self.data.len() > 4 { "..." } else { "" },
        )
    }

    fn has_nonce(&self) -> bool {
        self.nonce.is_some()
    }

    fn populate_nonce(&mut self, nonce: u64) {
        self.nonce = Some(nonce);
    }

    fn produce_transaction(&self) -> TransactionSigned {
        self.make_transaction()
    }

    fn produce_log(&self) -> Log {
        self.make_sys_log().into()
    }

    fn evm_sender(&self) -> Address {
        self.magic_sig.rollup_sender()
    }
}

impl SysTx for TransactSysTx {
    fn callee(&self) -> TxKind {
        self.to.into()
    }

    fn input(&self) -> Bytes {
        self.data.clone()
    }

    fn value(&self) -> U256 {
        self.value
    }
}

impl MeteredSysTx for TransactSysTx {
    fn gas_limit(&self) -> u128 {
        self.gas as u128
    }

    fn max_fee_per_gas(&self) -> u128 {
        self.max_fee_per_gas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::B256;

    /// Verifies that `produce_transaction()` returns a transaction with the
    /// correct nonce and that changing the nonce produces a different hash.
    #[test]
    fn produce_transaction_hash_changes_with_nonce() {
        let magic_sig = MagicSig {
            ty: MagicSigInfo::Transact { sender: Address::repeat_byte(0x11), aliased: false },
            txid: B256::repeat_byte(0xaa),
            event_idx: 0,
        };

        let mut tx = TransactSysTx {
            rollup_chain_id: 1,
            to: Address::repeat_byte(0x42),
            data: Bytes::from_static(&[1, 2, 3, 4]),
            value: U256::from(1000),
            gas: 100_000,
            max_fee_per_gas: 1_000_000_000,
            nonce: None,
            magic_sig,
        };

        // Set nonce=0 and produce transaction
        tx.populate_nonce(0);
        let tx_nonce_0 = tx.produce_transaction();
        let hash_nonce_0 = *tx_nonce_0.hash();

        // Set nonce=1 and produce transaction
        tx.populate_nonce(1);
        let tx_nonce_1 = tx.produce_transaction();
        let hash_nonce_1 = *tx_nonce_1.hash();

        // Hashes must be different
        assert_ne!(
            hash_nonce_0, hash_nonce_1,
            "transaction hashes should differ when nonce changes"
        );

        // Set nonce=2 and produce transaction
        tx.populate_nonce(2);
        let tx_nonce_2 = tx.produce_transaction();
        let hash_nonce_2 = *tx_nonce_2.hash();

        // All hashes must be unique
        assert_ne!(hash_nonce_0, hash_nonce_2);
        assert_ne!(hash_nonce_1, hash_nonce_2);
    }

    /// Verifies that two TransactSysTx with identical content but different
    /// nonces produce transactions with different hashes.
    #[test]
    fn identical_tx_content_different_nonces_have_different_hashes() {
        let magic_sig = MagicSig {
            ty: MagicSigInfo::Transact { sender: Address::repeat_byte(0x11), aliased: false },
            txid: B256::repeat_byte(0xaa),
            event_idx: 0,
        };

        let mut tx1 = TransactSysTx {
            rollup_chain_id: 1,
            to: Address::repeat_byte(0x42),
            data: Bytes::from_static(&[1, 2, 3, 4]),
            value: U256::from(1000),
            gas: 100_000,
            max_fee_per_gas: 1_000_000_000,
            nonce: None,
            magic_sig,
        };

        let mut tx2 = tx1.clone();

        tx1.populate_nonce(5);
        tx2.populate_nonce(6);

        let hash1 = *tx1.produce_transaction().hash();
        let hash2 = *tx2.produce_transaction().hash();

        assert_ne!(hash1, hash2, "two txs with different nonces should have different hashes");
    }
}
