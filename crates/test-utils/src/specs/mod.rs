mod host_spec;
pub use host_spec::HostBlockSpec;

mod notif_spec;
pub use notif_spec::{ExExNotification, NotificationSpec, NotificationWithSidecars};

mod ru_spec;
pub use ru_spec::RuBlockSpec;

use alloy::{
    consensus::{
        constants::GWEI_TO_WEI, SignableTransaction, TxEip1559, TxEnvelope, TypedTransaction,
    },
    eips::Encodable2718,
    primitives::{Address, TxKind, B256, U256},
    rpc::types::mev::EthSendBundle,
    signers::{local::PrivateKeySigner, SignerSync},
    sol_types::SolCall,
};
use signet_bundle::SignetEthBundle;
use signet_types::SignedFill;

/// Sign a transaction with a wallet.
pub fn sign_tx_with_key_pair(wallet: &PrivateKeySigner, tx: TypedTransaction) -> TxEnvelope {
    let signature = wallet.sign_hash_sync(&tx.signature_hash()).unwrap();
    TxEnvelope::new_unhashed(tx, signature)
}

/// Make a wallet with a deterministic keypair.
pub fn make_wallet(i: u8) -> PrivateKeySigner {
    PrivateKeySigner::from_bytes(&B256::repeat_byte(i)).unwrap()
}

/// Make a simple send transaction.
pub fn simple_send(to: Address, amount: U256, nonce: u64, ru_chain_id: u64) -> TypedTransaction {
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

/// Make a simple contract call transaction.
pub fn simple_call<T>(
    to: Address,
    input: &T,
    value: U256,
    nonce: u64,
    ru_chain_id: u64,
) -> TypedTransaction
where
    T: SolCall,
{
    TxEip1559 {
        nonce,
        gas_limit: 2_100_000,
        to: TxKind::Call(to),
        value,
        chain_id: ru_chain_id,
        max_fee_per_gas: GWEI_TO_WEI as u128 * 100,
        max_priority_fee_per_gas: GWEI_TO_WEI as u128,
        input: input.abi_encode().into(),
        ..Default::default()
    }
    .into()
}

/// Create a simple bundle from a list of transactions.
pub fn simple_bundle<'a>(
    txs: impl IntoIterator<Item = &'a TxEnvelope>,
    host_fills: Option<SignedFill>,
    block_number: u64,
) -> SignetEthBundle {
    let txs = txs.into_iter().map(|tx| tx.encoded_2718().into()).collect();

    SignetEthBundle {
        bundle: EthSendBundle {
            txs,
            block_number,
            min_timestamp: None,
            max_timestamp: None,
            reverting_tx_hashes: vec![],
            replacement_uuid: None,
            dropping_tx_hashes: vec![],
            refund_percent: None,
            refund_recipient: None,
            refund_tx_hashes: vec![],
            extra_fields: Default::default(),
        },
        host_fills,
        host_txs: vec![],
    }
}
