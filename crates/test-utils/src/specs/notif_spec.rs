use crate::{chain::Chain, specs::HostBlockSpec};
use alloy::consensus::BlobTransactionSidecar;
use signet_types::primitives::TransactionSigned;
use std::{collections::BTreeMap, sync::Arc};

/// A shim for reth_exex::ExExNotification that allows us to use it in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExExNotification {
    /// Chain got committed without a reorg, and only the new chain is returned.
    Committed {
        /// The new chain after commit.
        new: Arc<Chain>,
    },
    /// Chain got reorged, and both the old and the new chains are returned.
    Reorged {
        /// The old chain before reorg.
        old: Arc<Chain>,
        /// The new chain after reorg.
        new: Arc<Chain>,
    },
    /// Chain got reverted, and only the old chain is returned.
    Reverted {
        /// The old chain before reversion.
        old: Arc<Chain>,
    },
}

impl ExExNotification {
    /// Returns the committed chain, if any.
    pub fn committed_chain(&self) -> Option<&Arc<Chain>> {
        match self {
            ExExNotification::Committed { new } => Some(new),
            ExExNotification::Reorged { new, .. } => Some(new),
            ExExNotification::Reverted { .. } => None,
        }
    }

    /// Returns the reverted chain, if any.
    pub fn reverted_chain(&self) -> Option<&Arc<Chain>> {
        match self {
            ExExNotification::Reorged { old, .. } => Some(old),
            ExExNotification::Reverted { old } => Some(old),
            ExExNotification::Committed { .. } => None,
        }
    }
}

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
                let tx = self.new[0].sealed_block().transactions().last().unwrap().clone();
                sidecars.insert(num, (sidecar, tx));
            }

            // we enumerate to ensure they're in block number order
            for (i, block) in self.new.iter().enumerate().skip(1) {
                block.set_block_number(num + i as u64);

                let execution_outcome = block.execution_outcome();

                // accumualate the sidecar here if necessary
                if let Some(sidecar) = block.sidecar.clone() {
                    let tx = block.sealed_block().transactions().last().unwrap().clone();
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
                notification: ExExNotification::Reorged {
                    old: Arc::new(old_chain),
                    new: Arc::new(new_chain),
                },
                sidecars,
            },
            (Some(old_chain), None) => NotificationWithSidecars {
                notification: ExExNotification::Reverted { old: Arc::new(old_chain) },
                sidecars,
            },
            (None, Some(new_chain)) => NotificationWithSidecars {
                notification: ExExNotification::Committed { new: Arc::new(new_chain) },
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
