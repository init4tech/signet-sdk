mod host_spec;
pub use host_spec::HostBlockSpec;

mod notif_spec;
pub use notif_spec::{NotificationSpec, NotificationWithSidecars};

mod ru_spec;
pub use ru_spec::RuBlockSpec;

use alloy::{
    consensus::{constants::GWEI_TO_WEI, SignableTransaction, TxEip1559},
    primitives::{Address, TxKind, B256, U256},
    signers::{local::PrivateKeySigner, SignerSync},
};
use signet_types::primitives::{Transaction, TransactionSigned};

/// Sign a transaction with a wallet.
pub fn sign_tx_with_key_pair(wallet: &PrivateKeySigner, tx: Transaction) -> TransactionSigned {
    let signature = wallet.sign_hash_sync(&tx.signature_hash()).unwrap();
    TransactionSigned::new_unhashed(tx, signature)
}

/// Make a wallet with a deterministic keypair.
pub fn make_wallet(i: u8) -> PrivateKeySigner {
    PrivateKeySigner::from_bytes(&B256::repeat_byte(i)).unwrap()
}

/// Make a simple send transaction.
pub fn simple_send(to: Address, amount: U256, nonce: u64, ru_chain_id: u64) -> Transaction {
    TxEip1559 {
        nonce,
        gas_limit: 21_000,
        to: TxKind::Call(to),
        value: amount,
        chain_id: ru_chain_id,
        max_fee_per_gas: GWEI_TO_WEI as u128 * 100,
        max_priority_fee_per_gas: GWEI_TO_WEI as u128,
        ..Default::default()
    }
    .into()
}
