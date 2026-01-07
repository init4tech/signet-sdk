use crate::{outcome::SimulatedItem, SimItem};
use alloy::{
    consensus::{transaction::Recovered, SidecarBuilder, SidecarCoder, TxEnvelope},
    primitives::{keccak256, Bytes, B256},
};
use core::fmt;
use signet_bundle::RecoveredBundle;
use signet_zenith::{encode_txns, Alloy2718Coder};
use std::sync::OnceLock;
use tracing::trace;

/// A block that has been built by the simulator.
#[derive(Clone, Default)]
pub struct BuiltBlock {
    /// The host transactions to be included in a resulting bundle.
    pub(crate) host_txns: Vec<Recovered<TxEnvelope>>,

    /// Transactions in the block.
    pub(crate) transactions: Vec<Recovered<TxEnvelope>>,

    /// The block number for the Signet block.
    pub(crate) block_number: u64,

    /// The amount of gas used by the block so far
    pub(crate) gas_used: u64,

    /// The amount of host gas used by the block so far
    pub(crate) host_gas_used: u64,

    // -- Memoization fields --
    /// Memoized raw encoding of the block.
    pub(crate) raw_encoding: OnceLock<Bytes>,
    /// Memoized hash of the block.
    pub(crate) hash: OnceLock<B256>,
}

impl fmt::Debug for BuiltBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuiltBlock")
            .field("transactions", &self.transactions.len())
            .field("host_txns", &self.host_txns.len())
            .field("gas_used", &self.gas_used)
            .field("host_gas_used", &self.host_gas_used)
            .field("block_number", &self.block_number)
            .finish_non_exhaustive()
    }
}

impl BuiltBlock {
    /// Create a new `BuiltBlock`
    pub const fn new(block_number: u64) -> Self {
        Self {
            host_txns: Vec::new(),
            transactions: Vec::new(),
            block_number,
            gas_used: 0,
            host_gas_used: 0,
            raw_encoding: OnceLock::new(),
            hash: OnceLock::new(),
        }
    }

    /// Gets the block number for the block.
    pub const fn block_number(&self) -> u64 {
        self.block_number
    }

    /// Get the amount of gas used by the block.
    pub const fn gas_used(&self) -> u64 {
        self.gas_used
    }

    /// Get the amount of host gas used by the block.
    pub const fn host_gas_used(&self) -> u64 {
        self.host_gas_used
    }

    /// Get the number of transactions in the block.
    pub const fn tx_count(&self) -> usize {
        self.transactions.len()
    }

    /// Check if the block is empty.
    pub const fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// Get the current list of transactions included in this block.
    #[allow(clippy::missing_const_for_fn)] // false positive, const deref
    pub fn transactions(&self) -> &[Recovered<TxEnvelope>] {
        &self.transactions
    }

    /// Get the current list of host transactions included in this block.
    pub const fn host_transactions(&self) -> &[Recovered<TxEnvelope>] {
        self.host_txns.as_slice()
    }

    /// Unseal the block
    pub(crate) fn unseal(&mut self) {
        self.raw_encoding.take();
        self.hash.take();
    }

    /// Seal the block by encoding the transactions and calculating the hash of
    /// the block contents.
    pub(crate) fn seal(&self) {
        self.raw_encoding.get_or_init(|| {
            let iter = self.transactions.iter().map(Recovered::inner);
            encode_txns::<Alloy2718Coder>(iter).into()
        });
        self.hash.get_or_init(|| keccak256(self.raw_encoding.get().unwrap().as_ref()));
    }

    /// Ingest a transaction into the in-progress block.
    pub fn ingest_tx(&mut self, tx: Recovered<TxEnvelope>) {
        trace!(hash = %tx.tx_hash(), "ingesting tx");
        self.unseal();
        self.transactions.push(tx);
    }

    /// Ingest a bundle into the in-progress block.
    /// Ignores Signed Orders for now.
    pub fn ingest_bundle(&mut self, mut bundle: RecoveredBundle) {
        trace!(replacement_uuid = bundle.replacement_uuid(), "adding bundle to block");

        self.unseal();
        // extend the transactions with the decoded transactions.
        // As this builder does not provide bundles landing "top of block", its fine to just extend.
        self.transactions.extend(bundle.drain_txns());
        self.host_txns.extend(bundle.drain_host_txns());
    }

    /// Ingest a simulated item, extending the block.
    pub fn ingest(&mut self, item: SimulatedItem) {
        self.gas_used += item.gas_used;
        self.host_gas_used += item.host_gas_used;

        match item.item {
            SimItem::Bundle(bundle) => self.ingest_bundle(*bundle),
            SimItem::Tx(tx) => self.ingest_tx(*tx),
        }
    }

    /// Encode the in-progress block.
    pub(crate) fn encode_raw(&self) -> &Bytes {
        self.seal();
        self.raw_encoding.get().unwrap()
    }

    /// Calculate the hash of the in-progress block, finishing the block.
    pub fn contents_hash(&self) -> &B256 {
        self.seal();
        self.hash.get().unwrap()
    }

    /// Convert the in-progress block to sign request contents.
    pub fn encode_calldata(&self) -> &Bytes {
        self.encode_raw()
    }

    /// Convert the in-progress block to a blob transaction sidecar.
    pub fn encode_blob<T: SidecarCoder + Default>(&self) -> SidecarBuilder<T> {
        let mut coder = SidecarBuilder::<T>::default();
        coder.ingest(self.encode_raw());
        coder
    }
}
