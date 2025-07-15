use crate::sys::{MeteredSysTx, SysBase, SysTx, TransactSysLog};
use alloy::{
    consensus::{EthereumTxEnvelope, Transaction},
    hex,
    primitives::{utils::format_ether, Address, Bytes, Log, TxKind, U256},
};
use core::fmt;
use signet_extract::ExtractedEvent;
use signet_types::{primitives::TransactionSigned, MagicSig};
use signet_zenith::Transactor;
use trevm::{revm::context::TxEnv, Tx};

/// Shim to impl [`Tx`] for [`Transactor::Transact`].
#[derive(PartialEq, Eq)]
pub struct TransactSysTx {
    tx: TransactionSigned,

    /// The nonce of the transaction.
    nonce: Option<u64>,

    /// The magic sig. Memoized here to make it a little simpler to
    /// access. Also available on the [`MagicSig`] in the transaction above.
    magic_sig: MagicSig,
}

impl<'a, R> From<&ExtractedEvent<'a, R, Transactor::Transact>> for TransactSysTx {
    fn from(transact: &ExtractedEvent<'a, R, Transactor::Transact>) -> Self {
        Self::new(transact)
    }
}

impl TransactSysTx {
    /// Instantiate a new [`TransactFiller`].
    pub fn new<R>(transact: &ExtractedEvent<'_, R, Transactor::Transact>) -> Self {
        let magic_sig = transact.magic_sig();
        let tx = transact.make_transaction(0);
        Self { tx, nonce: None, magic_sig }
    }

    /// Create a [`TransactSysLog`] from the filler.
    fn make_sys_log(&self) -> TransactSysLog {
        TransactSysLog {
            txHash: self.magic_sig.txid,
            logIndex: self.magic_sig.event_idx as u64,
            sender: self.evm_sender(),
            value: self.tx.value(),
            gas: U256::from(self.tx.gas_limit()),
            maxFeePerGas: U256::from(self.tx.max_fee_per_gas()),
        }
    }
}

// NB: manual impl because of incorrect auto-derive bound on `R: Debug`
impl fmt::Debug for TransactSysTx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactFiller")
            .field("transact", &self.tx)
            .field("magic_sig", &self.magic_sig)
            .finish()
    }
}

// NB: manual impl because of incorrect auto-derive bound on `R: Clone`
impl Clone for TransactSysTx {
    fn clone(&self) -> Self {
        Self { tx: self.tx.clone(), nonce: self.nonce, magic_sig: self.magic_sig }
    }
}

impl Tx for TransactSysTx {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        self.tx.as_eip1559().unwrap().fill_tx_env(tx_env);
        tx_env.caller = self.magic_sig.sender();
    }
}

impl SysBase for TransactSysTx {
    fn name() -> &'static str {
        "TransactSysTx"
    }

    fn description(&self) -> String {
        format!(
            "Transact from {} to {} with value {} and {} bytes of input data: `0x{}{}`",
            self.magic_sig.sender(),
            self.tx.to().expect("creates not allowed"),
            format_ether(self.tx.value()),
            self.tx.input().len(),
            self.tx.input().chunks(4).next().map(hex::encode).unwrap_or_default(),
            if self.tx.input().len() > 4 { "..." } else { "" },
        )
    }

    fn has_nonce(&self) -> bool {
        self.nonce.is_some()
    }

    fn populate_nonce(&mut self, nonce: u64) {
        // NB: we have to set the nonce on the tx as well.
        let EthereumTxEnvelope::Eip1559(signed) = &mut self.tx else {
            unreachable!("new sets this to 1559");
        };
        signed.tx_mut().nonce = nonce;
        self.nonce = Some(nonce);
    }

    fn produce_transaction(&self) -> TransactionSigned {
        self.tx.clone()
    }

    fn produce_log(&self) -> Log {
        self.make_sys_log().into()
    }

    fn evm_sender(&self) -> Address {
        self.magic_sig.sender()
    }
}

impl SysTx for TransactSysTx {
    fn callee(&self) -> TxKind {
        self.tx.kind()
    }

    fn input(&self) -> Bytes {
        self.tx.input().clone()
    }

    fn value(&self) -> U256 {
        self.tx.value()
    }
}

impl MeteredSysTx for TransactSysTx {
    fn gas_limit(&self) -> u128 {
        self.tx.gas_limit() as u128
    }

    fn max_fee_per_gas(&self) -> u128 {
        self.tx.max_fee_per_gas()
    }
}
