use super::{sign_tx_with_key_pair, simple_send};
use crate::test_utils::HostBlockSpec;
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

/// A notification spec.
#[derive(Debug, Default)]
pub struct NotificationSpec {
    /// The old blocks.
    pub old: Vec<HostBlockSpec>,
    /// The new blocks.
    pub new: Vec<HostBlockSpec>,
}

impl NotificationSpec {
    /// Make a new notification spec from a single block
    pub fn commit_single_block(block: HostBlockSpec) -> Self {
        Self { old: vec![], new: vec![block] }
    }

    /// Make a new notification spec from a single block
    pub fn revert_single_block(block: HostBlockSpec) -> Self {
        Self { old: vec![block], new: vec![] }
    }

    /// Commit a block to the spec.
    pub fn commit(mut self, block: HostBlockSpec) -> Self {
        self.new.push(block);
        self
    }

    /// Add a block to revert to the spec.
    pub fn revert(mut self, block: HostBlockSpec) -> Self {
        self.old.push(block);
        self
    }

    /// Convert to an exex notification.
    pub fn to_exex_notification(&self) -> NotificationWithSidecars {
        let mut sidecars = BTreeMap::new();

        // we do not accumulate sidecars for the old chain.
        let old_chain = if !self.old.is_empty() {
            let num = self.old[0].block_number();
            let (mut chain, _sidecar) = self.old[0].to_chain();

            // we enumerate to ensure they're in block number order
            for (i, block) in self.old.iter().enumerate().skip(1) {
                block.set_block_number(num + i as u64);
                chain.append_block(block.recovered_block(), block.execution_outcome());
            }
            Some(chain)
        } else {
            None
        };

        let new_chain = if !self.new.is_empty() {
            let num = self.new[0].block_number();
            let (mut chain, sidecar) = self.new[0].to_chain();
            // accumulate sidecar if necessary
            if let Some(sidecar) = sidecar {
                let tx = self.new[0].block().body().transactions().last().unwrap().clone();
                sidecars.insert(num, (sidecar, tx));
            }

            // we enumerate to ensure they're in block number order
            for (i, block) in self.new.iter().enumerate().skip(1) {
                block.set_block_number(num + i as u64);

                let execution_outcome = block.execution_outcome();

                // accumualate the sidecar here if necessary
                if let Some(sidecar) = block.sidecar.clone() {
                    let tx = block.block().body().transactions().last().unwrap().clone();
                    sidecars.insert(block.block_number(), (sidecar, tx));
                }

                chain.append_block(block.recovered_block(), execution_outcome)
            }

            Some(chain)
        } else {
            None
        };

        match (old_chain, new_chain) {
            (Some(old_chain), Some(new_chain)) => NotificationWithSidecars {
                notification: ExExNotification::ChainReorged {
                    old: Arc::new(old_chain),
                    new: Arc::new(new_chain),
                },
                sidecars,
            },
            (Some(old_chain), None) => NotificationWithSidecars {
                notification: ExExNotification::ChainReverted { old: Arc::new(old_chain) },
                sidecars,
            },
            (None, Some(new_chain)) => NotificationWithSidecars {
                notification: ExExNotification::ChainCommitted { new: Arc::new(new_chain) },
                sidecars,
            },
            (None, None) => panic!("missing old and new chains"),
        }
    }
}

/// A notification with sidecars associated with the new chain.
#[derive(Debug, Clone)]
pub struct NotificationWithSidecars {
    /// The notification.
    pub notification: ExExNotification,
    /// Sidecars associated with the new chain.
    pub sidecars: BTreeMap<u64, (BlobTransactionSidecar, TransactionSigned)>,
}

impl NotificationWithSidecars {
    /// Make a new notification from a single block
    pub fn commit_single_block(block: HostBlockSpec) -> Self {
        NotificationSpec::commit_single_block(block).to_exex_notification()
    }

    /// Make a new notification from a single block
    pub fn revert_single_block(block: HostBlockSpec) -> Self {
        NotificationSpec::revert_single_block(block).to_exex_notification()
    }
}
