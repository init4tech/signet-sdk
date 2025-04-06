//! Signet bundle types.
use alloy::{
    consensus::{Transaction, TxEnvelope},
    eips::{eip2718::Encodable2718, BlockNumberOrTag, Decodable2718},
    primitives::{keccak256, Bytes, B256, U256},
    rlp::Buf,
    rpc::types::mev::{EthCallBundle, EthCallBundleResponse, EthCallBundleTransactionResult},
};
use serde::{Deserialize, Serialize};
use signet_types::AggregateFills;
use signet_zenith::AggregateOrders;
use trevm::{
    revm::{context::result::ExecutionResult, Database},
    BundleError,
};

/// Bundle of transactions for `signet_callBundle`.
///
/// The Signet bundle contains the following:
///
/// - A standard [`EthCallBundle`] with the transactions to simulate.
/// - A mapping of assets to users to amounts, which are the host fills to be
///   checked against market orders after simulation.
///
/// This is based on the flashbots `eth_callBundle` bundle. See [their docs].
///
/// [their docs]: https://docs.flashbots.net/flashbots-auction/advanced/rpc-endpoint
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignetCallBundle {
    /// The bundle of transactions to simulate. Same structure as a Flashbots
    /// [`EthCallBundle`] bundle.
    #[serde(flatten)]
    pub bundle: EthCallBundle,
}

impl SignetCallBundle {
    /// Returns the transactions in this bundle.
    #[allow(clippy::missing_const_for_fn)] // false positive
    pub fn txs(&self) -> &[Bytes] {
        &self.bundle.txs
    }

    /// Returns the block number for this bundle.
    pub const fn block_number(&self) -> u64 {
        self.bundle.block_number
    }

    /// Returns the state block number for this bundle.
    pub const fn state_block_number(&self) -> BlockNumberOrTag {
        self.bundle.state_block_number
    }

    /// Returns the timestamp for this bundle.
    pub const fn timestamp(&self) -> Option<u64> {
        self.bundle.timestamp
    }

    /// Returns the gas limit for this bundle.
    pub const fn gas_limit(&self) -> Option<u64> {
        self.bundle.gas_limit
    }

    /// Returns the difficulty for this bundle.
    pub const fn difficulty(&self) -> Option<U256> {
        self.bundle.difficulty
    }

    /// Returns the base fee for this bundle.
    pub const fn base_fee(&self) -> Option<u128> {
        self.bundle.base_fee
    }

    /// Adds an [`Encodable2718`] transaction to the bundle.
    pub fn append_2718_tx(self, tx: impl Encodable2718) -> Self {
        self.append_raw_tx(tx.encoded_2718())
    }

    /// Adds an EIP-2718 envelope to the bundle.
    pub fn append_raw_tx(mut self, tx: impl Into<Bytes>) -> Self {
        self.bundle.txs.push(tx.into());
        self
    }

    /// Adds multiple [`Encodable2718`] transactions to the bundle.
    pub fn extend_2718_txs<I, T>(self, tx: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Encodable2718,
    {
        self.extend_raw_txs(tx.into_iter().map(|tx| tx.encoded_2718()))
    }

    /// Adds multiple calls to the block.
    pub fn extend_raw_txs<I, T>(mut self, txs: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Bytes>,
    {
        self.bundle.txs.extend(txs.into_iter().map(Into::into));
        self
    }

    /// Sets the block number for the bundle.
    pub const fn with_block_number(mut self, block_number: u64) -> Self {
        self.bundle.block_number = block_number;
        self
    }

    /// Sets the state block number for the bundle.
    pub fn with_state_block_number(
        mut self,
        state_block_number: impl Into<BlockNumberOrTag>,
    ) -> Self {
        self.bundle.state_block_number = state_block_number.into();
        self
    }

    /// Sets the timestamp for the bundle.
    pub const fn with_timestamp(mut self, timestamp: u64) -> Self {
        self.bundle.timestamp = Some(timestamp);
        self
    }

    /// Sets the gas limit for the bundle.
    pub const fn with_gas_limit(mut self, gas_limit: u64) -> Self {
        self.bundle.gas_limit = Some(gas_limit);
        self
    }

    /// Sets the difficulty for the bundle.
    pub const fn with_difficulty(mut self, difficulty: U256) -> Self {
        self.bundle.difficulty = Some(difficulty);
        self
    }

    /// Sets the base fee for the bundle.
    pub const fn with_base_fee(mut self, base_fee: u128) -> Self {
        self.bundle.base_fee = Some(base_fee);
        self
    }

    /// Calculate the bundle hash for this bundle.
    ///
    /// The hash is calculated as
    /// `keccak256(tx_hash1 || tx_hash2 || ... || tx_hashn)` where `||` is the
    /// concatenation operator.
    pub fn bundle_hash(&self) -> B256 {
        let mut hasher = alloy::primitives::Keccak256::new();

        // Concatenate the transaction hashes, to then hash them. This is the tx_preimage.
        for tx in self.bundle.txs.iter() {
            // Calculate the tx hash (keccak256(encoded_signed_tx)) and append it to the tx_bytes.
            hasher.update(keccak256(tx).as_slice());
        }
        hasher.finalize()
    }

    /// Decode and validate the transactions in the bundle.
    pub fn decode_and_validate_txs<Db: trevm::revm::Database>(
        &self,
    ) -> Result<Vec<TxEnvelope>, BundleError<Db>> {
        let txs = self
            .txs()
            .iter()
            .map(|tx| TxEnvelope::decode_2718(&mut tx.chunk()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| BundleError::TransactionDecodingError(err))?;

        if txs.iter().any(|tx| tx.is_eip4844()) {
            return Err(BundleError::UnsupportedTransactionType);
        }

        Ok(txs)
    }
}

/// Response for `signet_callBundle`.
///
/// The response contains the following:
/// - The inner [`EthCallBundleResponse`] response.
/// - Aggregate orders produced by the bundle.
/// - Fills produced by the bundle.
///
/// The aggregate orders contains both the net outputs the filler can expect to
/// receive from this bundle and the net inputs the filler must provide to
/// ensure this bundle is valid.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SignetCallBundleResponse {
    #[serde(flatten)]
    inner: EthCallBundleResponse,
    /// Aggregate orders produced by the bundle.
    ///
    /// This mapping will contain all outputs and required inputs, collapsed
    /// into a single entry per asset. For the bundle to be valid, there must
    /// be fills for all the inputs in this mapping. Which is to say, this type
    /// indicates the following:
    ///
    /// - The net outputs the filler can expect to receive from this bundle.
    /// - The net inputs the filler must provide to ensure this bundle is valid.
    pub orders: AggregateOrders,
    /// Fills produced by the bundle. This will contain the net fills produced
    /// by the transaction. These can be deducted from the net inputs required
    /// by the orders to ensure the bundle is valid.
    pub fills: AggregateFills,
}

impl core::ops::Deref for SignetCallBundleResponse {
    type Target = EthCallBundleResponse;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl core::ops::DerefMut for SignetCallBundleResponse {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl AsRef<EthCallBundleResponse> for SignetCallBundleResponse {
    fn as_ref(&self) -> &EthCallBundleResponse {
        &self.inner
    }
}

impl AsMut<EthCallBundleResponse> for SignetCallBundleResponse {
    fn as_mut(&mut self) -> &mut EthCallBundleResponse {
        &mut self.inner
    }
}

impl From<EthCallBundleResponse> for SignetCallBundleResponse {
    fn from(inner: EthCallBundleResponse) -> Self {
        Self { inner, orders: Default::default(), fills: Default::default() }
    }
}

impl From<SignetCallBundleResponse> for EthCallBundleResponse {
    fn from(this: SignetCallBundleResponse) -> Self {
        this.inner
    }
}

impl SignetCallBundleResponse {
    /// Accumulate a transaction result into the response.
    fn accumulate_tx_result(&mut self, tx_result: EthCallBundleTransactionResult) {
        self.inner.total_gas_used += tx_result.gas_used;
        self.inner.gas_fees += tx_result.gas_fees;
        self.inner.results.push(tx_result);
    }

    /// Accumulate the result of transaction execution into the response.
    pub fn accumulate_tx<Db: Database>(
        &mut self,
        tx: &TxEnvelope,
        coinbase_diff: U256,
        base_fee: u64,
        execution_result: ExecutionResult,
    ) -> Result<(), BundleError<Db>> {
        if let TxEnvelope::Eip4844(_) = tx {
            return Err(BundleError::UnsupportedTransactionType);
        }

        // we'll incrementally populate this result.
        let mut result = EthCallBundleTransactionResult::default();

        result.from_address =
            tx.recover_signer().map_err(|e| BundleError::TransactionSenderRecoveryError(e))?;

        // Calculate the gas price and fees
        result.gas_price = U256::from(tx.effective_gas_price(Some(base_fee)));
        result.gas_used = execution_result.gas_used();
        result.gas_fees = result.gas_price * U256::from(result.gas_used);

        // set the return data for the response
        if execution_result.is_success() {
            result.value = Some(execution_result.into_output().unwrap_or_default());
        } else {
            result.revert = Some(execution_result.into_output().unwrap_or_default());
        };

        // Calculate the coinbase diff and the eth sent to coinbase
        result.coinbase_diff = coinbase_diff;
        result.eth_sent_to_coinbase = result.coinbase_diff.saturating_sub(result.gas_fees);

        // Accumulate the result
        self.accumulate_tx_result(result);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy::{
        eips::BlockNumberOrTag,
        primitives::{Address, U256},
        rpc::types::mev::{EthCallBundle, EthCallBundleTransactionResult},
    };

    #[test]
    fn call_bundle_ser_roundtrip() {
        let bundle = SignetCallBundle {
            bundle: EthCallBundle {
                txs: vec![b"tx1".into(), b"tx2".into()],
                block_number: 1,
                state_block_number: BlockNumberOrTag::Number(2),
                timestamp: Some(3),
                gas_limit: Some(4),
                difficulty: Some(alloy::primitives::U256::from(5)),
                base_fee: Some(6),
                transaction_index: Some(7.into()),
                coinbase: Some(Address::repeat_byte(8)),
                timeout: Some(9),
            },
        };

        let serialized = serde_json::to_string(&bundle).unwrap();
        let deserialized: SignetCallBundle = serde_json::from_str(&serialized).unwrap();

        assert_eq!(bundle, deserialized);
    }

    #[test]
    fn call_bundle_resp_ser_roundtrip() {
        let resp: SignetCallBundleResponse = EthCallBundleResponse {
            bundle_hash: B256::repeat_byte(1),
            bundle_gas_price: U256::from(2),
            coinbase_diff: U256::from(3),
            eth_sent_to_coinbase: U256::from(4),
            gas_fees: U256::from(5),
            results: vec![EthCallBundleTransactionResult {
                coinbase_diff: U256::from(6),
                eth_sent_to_coinbase: U256::from(7),
                from_address: Address::repeat_byte(8),
                gas_fees: U256::from(9),
                gas_price: U256::from(10),
                gas_used: 11,
                to_address: Some(Address::repeat_byte(12)),
                tx_hash: B256::repeat_byte(13),
                value: Some(Bytes::from(b"value")),
                revert: Some(Bytes::from(b"revert")),
            }],
            state_block_number: 14,
            total_gas_used: 15,
        }
        .into();

        let serialized = serde_json::to_string(&resp).unwrap();
        let deserialized: SignetCallBundleResponse = serde_json::from_str(&serialized).unwrap();

        assert_eq!(resp, deserialized);
    }
}
