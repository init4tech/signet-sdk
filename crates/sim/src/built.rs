use alloy::{
    consensus::{SidecarBuilder, SidecarCoder, TxEnvelope},
    eips::Decodable2718,
    primitives::{keccak256, Bytes, B256},
    rlp::Buf,
};
use core::fmt;
use signet_bundle::SignetEthBundle;
use signet_zenith::{encode_txns, Alloy2718Coder, SignedFill};
use std::sync::OnceLock;
use tracing::{error, trace};

use crate::{outcome::SimulatedItem, SimItem};

/// A block that has been built by the simulator.
#[derive(Clone, Default)]
pub struct BuiltBlock {
    /// The host fill actions.
    pub(crate) host_fills: Vec<SignedFill>,
    /// Transactions in the block.
    pub(crate) transactions: Vec<TxEnvelope>,

    /// The amount of gas used by the block so far
    pub(crate) gas_used: u64,

    /// Memoized raw encoding of the block.
    pub(crate) raw_encoding: OnceLock<Bytes>,
    /// Memoized hash of the block.
    pub(crate) hash: OnceLock<B256>,
}

impl fmt::Debug for BuiltBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuiltBlock")
            .field("host_fills", &self.host_fills.len())
            .field("transactions", &self.transactions.len())
            .field("gas_used", &self.gas_used)
            .finish()
    }
}

impl BuiltBlock {
    /// Create a new `BuiltBlock`
    pub const fn new() -> Self {
        Self {
            host_fills: Vec::new(),
            transactions: Vec::new(),
            gas_used: 0,
            raw_encoding: OnceLock::new(),
            hash: OnceLock::new(),
        }
    }

    /// Get the amount of gas used by the block.
    pub const fn gas_used(&self) -> u64 {
        self.gas_used
    }

    /// Get the number of transactions in the block.
    pub fn tx_count(&self) -> usize {
        self.transactions.len()
    }

    /// Check if the block is empty.
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// Get the current list of transactions included in this block.
    #[allow(clippy::missing_const_for_fn)] // false positive, const deref
    pub fn transactions(&self) -> &[TxEnvelope] {
        &self.transactions
    }

    /// Unseal the block
    pub(crate) fn unseal(&mut self) {
        self.raw_encoding.take();
        self.hash.take();
    }

    /// Seal the block by encoding the transactions and calculating the hash of
    /// the block contents.
    pub(crate) fn seal(&self) {
        self.raw_encoding.get_or_init(|| encode_txns::<Alloy2718Coder>(&self.transactions).into());
        self.hash.get_or_init(|| keccak256(self.raw_encoding.get().unwrap().as_ref()));
    }

    /// Ingest a transaction into the in-progress block.
    pub fn ingest_tx(&mut self, tx: TxEnvelope) {
        trace!(hash = %tx.tx_hash(), "ingesting tx");
        self.unseal();
        self.transactions.push(tx);
    }

    /// Ingest a bundle into the in-progress block.
    /// Ignores Signed Orders for now.
    pub fn ingest_bundle(&mut self, bundle: SignetEthBundle) {
        trace!(replacement_uuid = bundle.replacement_uuid(), "adding bundle to block");

        let txs = bundle
            .bundle
            .txs
            .into_iter()
            .map(|tx| TxEnvelope::decode_2718(&mut tx.chunk()))
            .collect::<Result<Vec<_>, _>>();

        if let Ok(txs) = txs {
            self.unseal();
            // extend the transactions with the decoded transactions.
            // As this builder does not provide bundles landing "top of block", its fine to just extend.
            self.transactions.extend(txs);

            if let Some(host_fills) = bundle.host_fills {
                self.host_fills.push(host_fills);
            }
        } else {
            error!("failed to decode bundle. dropping");
        }
    }

    /// Ingest a simulated item, extending the block.
    pub fn ingest(&mut self, item: SimulatedItem) {
        self.gas_used += item.gas_used;

        match item.item {
            SimItem::Bundle(bundle) => self.ingest_bundle(bundle),
            SimItem::Tx(tx) => self.ingest_tx(tx),
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
