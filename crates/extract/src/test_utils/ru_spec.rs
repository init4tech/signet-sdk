use super::{sign_tx_with_key_pair, simple_send};
use alloy::{
    consensus::{
        constants::GWEI_TO_WEI, BlobTransactionSidecar, SidecarBuilder, SimpleCoder, TxEip4844,
        TxEnvelope,
    },
    eips::eip2718::Encodable2718,
    primitives::{keccak256, Address, Bytes, FixedBytes, Log, LogData, Sealable, B256, U256},
    rlp::Encodable,
    signers::{local::PrivateKeySigner, Signature},
};
use reth::{
    primitives::{
        Block, BlockBody, Header, Receipt, RecoveredBlock, SealedBlock, SealedHeader, Transaction,
        TransactionSigned, TxType,
    },
    providers::{Chain, ExecutionOutcome},
};
use reth_exex::ExExNotification;
use signet_types::config::SignetSystemConstants;
use signet_zenith::{
    Passage, RollupOrders, Transactor,
    Zenith::{self},
};
use std::{
    borrow::Borrow,
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

/// A block spec for the Ru chain.
#[derive(Debug, Clone)]
pub struct RuBlockSpec {
    /// The system constants for the block.
    pub constants: SignetSystemConstants,
    /// The transactions in the block.
    pub tx: Vec<Vec<u8>>,
    /// The gas limit for the block.
    pub gas_limit: Option<u64>,
    /// The reward address for the block.
    pub reward_address: Option<Address>,
}

impl RuBlockSpec {
    /// Builder method to set the gas limit.
    pub fn with_gas_limit(mut self, gas_limit: u64) -> Self {
        self.gas_limit = Some(gas_limit);
        self
    }

    /// Builder method to set the reward address.
    pub fn with_reward_address(mut self, reward_address: Address) -> Self {
        self.reward_address = Some(reward_address);
        self
    }

    /// Add a transaction to the block.
    pub fn add_tx(&mut self, tx: &TransactionSigned) {
        self.tx.push(tx.encoded_2718());
    }

    /// Add an alloy transaction to the block
    pub fn add_alloy_tx(&mut self, tx: &TxEnvelope) {
        self.tx.push(tx.encoded_2718());
    }

    /// Add an invalid transaction to the block.
    pub fn add_invalid_tx(&mut self, tx: impl Into<Bytes>) {
        self.tx.push(tx.into().into());
    }

    /// Add a transaction to the block, returning self.
    pub fn tx(mut self, tx: &TransactionSigned) -> Self {
        self.add_tx(tx);
        self
    }

    /// Add an alloy transaction to the block, returning self.
    pub fn alloy_tx(mut self, tx: &TxEnvelope) -> Self {
        self.add_alloy_tx(tx);
        self
    }

    /// Add a simple send to the block, returns the send added.
    pub fn add_simple_send(
        &mut self,
        wallet: &PrivateKeySigner,
        to: Address,
        amount: U256,
        nonce: u64,
    ) -> TransactionSigned {
        let tx = sign_tx_with_key_pair(
            wallet,
            simple_send(to, amount, nonce, self.constants.ru_chain_id()),
        );
        self.add_tx(&tx);
        tx
    }

    /// Convert to a host sidecar.
    pub fn to_sidecar(&self) -> (B256, BlobTransactionSidecar) {
        let mut buf = vec![];
        Vec::<Vec<u8>>::encode(&self.tx, &mut buf);

        let sidecar = SidecarBuilder::<SimpleCoder>::from_slice(&buf).build().unwrap();
        (keccak256(&buf), sidecar)
    }

    /// Convert to a block submitted, along with the sidecar.
    pub fn to_block_submitted(&self) -> (Zenith::BlockSubmitted, BlobTransactionSidecar) {
        let (bdh, sidecar) = self.to_sidecar();

        let block_submitted = Zenith::BlockSubmitted {
            sequencer: Address::repeat_byte(3),
            rollupChainId: U256::from(self.constants.ru_chain_id()),
            gasLimit: U256::from(self.gas_limit.unwrap_or(100_000_000)),
            rewardAddress: self
                .reward_address
                .unwrap_or(signet_types::test_utils::DEFAULT_REWARD_ADDRESS),
            blockDataHash: bdh,
        };

        (block_submitted, sidecar)
    }
}
