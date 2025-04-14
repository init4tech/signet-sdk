use alloy::{
    consensus::{SidecarBuilder, SidecarCoder, TxEnvelope},
    eips::Decodable2718,
    primitives::{keccak256, Bytes, B256},
    rlp::Buf,
};
use signet_bundle::SignetEthBundle;
use signet_zenith::{encode_txns, Alloy2718Coder, SignedOrder};
use std::sync::OnceLock;
use tracing::{error, trace};

/// A block that has been built by the simulator.
#[derive(Debug, Clone, Default)]
pub struct BuiltBlock {
    /// The host fill actions.
    pub(crate) host_fills: Vec<SignedOrder>,
    /// Transactions in the block.
    pub(crate) transactions: Vec<TxEnvelope>,

    /// Memoized raw encoding of the block.
    pub(crate) raw_encoding: OnceLock<Bytes>,
    /// Memoized hash of the block.
    pub(crate) hash: OnceLock<B256>,
}

impl BuiltBlock {
    /// Create a new `BuiltBlock`
    pub const fn new() -> Self {
        Self {
            host_fills: Vec::new(),
            transactions: Vec::new(),
            raw_encoding: OnceLock::new(),
            hash: OnceLock::new(),
        }
    }

    /// Get the number of transactions in the block.
    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    /// Check if the block is empty.
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// Returns the current list of transactions included in this block
    pub fn transactions(&self) -> Vec<TxEnvelope> {
        self.transactions.clone()
    }

    /// Unseal the block
    pub(crate) fn unseal(&mut self) {
        self.raw_encoding.take();
        self.hash.take();
    }

    /// Seal the block by encoding the transactions and calculating the contentshash.
    pub(crate) fn seal(&self) {
        self.raw_encoding.get_or_init(|| encode_txns::<Alloy2718Coder>(&self.transactions).into());
        self.hash.get_or_init(|| keccak256(self.raw_encoding.get().unwrap().as_ref()));
    }

    /// Ingest a transaction into the in-progress block. Fails
    pub fn ingest_tx(&mut self, tx: &TxEnvelope) {
        trace!(hash = %tx.tx_hash(), "ingesting tx");
        self.unseal();
        self.transactions.push(tx.clone());
    }

    /// Remove a transaction from the in-progress block.
    pub fn remove_tx(&mut self, tx: &TxEnvelope) {
        trace!(hash = %tx.tx_hash(), "removing tx");
        self.unseal();
        self.transactions.retain(|t| t.tx_hash() != tx.tx_hash());
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

    /// Encode the in-progress block
    pub(crate) fn encode_raw(&self) -> &Bytes {
        self.seal();
        self.raw_encoding.get().unwrap()
    }

    /// Calculate the hash of the in-progress block, finishing the block.
    pub fn contents_hash(&self) -> B256 {
        self.seal();
        *self.hash.get().unwrap()
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
