//! Signet bundle types.
use alloy::{
    consensus::{Transaction, TxEnvelope},
    eips::{eip2718::Encodable2718, BlockNumberOrTag, Decodable2718},
    primitives::{keccak256, Address, Bytes, B256, U256},
    rlp::Buf,
    rpc::types::mev::{EthCallBundle, EthCallBundleResponse, EthCallBundleTransactionResult},
};
use serde::{Deserialize, Serialize};
use signet_types::MarketContext;
use std::collections::BTreeMap;
use trevm::{
    revm::{primitives::ExecutionResult, Database},
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
    /// Host fills to be applied to the bundle for simulation. The mapping corresponds
    /// to asset => user => amount.
    pub host_fills: BTreeMap<Address, BTreeMap<Address, U256>>,
}

impl SignetCallBundle {
    /// Returns the host fills for this bundle.
    pub const fn host_fills(&self) -> &BTreeMap<Address, BTreeMap<Address, U256>> {
        &self.host_fills
    }

    /// Returns the transactions in this bundle.
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

    /// Create a market context from the fills in this bundle.
    pub fn build_context(&self, host_chain_id: u64) -> MarketContext {
        let mut context = MarketContext::default();
        self.host_fills.iter().for_each(|(asset, fills)| {
            fills.iter().for_each(|(recipient, amount)| {
                context.add_raw_fill(host_chain_id, *asset, *recipient, *amount)
            })
        });
        context
    }

    /// Creates a new bundle from the given [`Encodable2718`] transactions.
    pub fn from_2718_and_host_fills<I, T>(
        txs: I,
        host_fills: BTreeMap<Address, BTreeMap<Address, U256>>,
    ) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Encodable2718,
    {
        Self::from_raw_txs_and_host_fills(txs.into_iter().map(|tx| tx.encoded_2718()), host_fills)
    }

    /// Creates a new bundle with the given transactions and host fills.
    pub fn from_raw_txs_and_host_fills<I, T>(
        txs: I,
        host_fills: BTreeMap<Address, BTreeMap<Address, U256>>,
    ) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Bytes>,
    {
        Self {
            bundle: EthCallBundle {
                txs: txs.into_iter().map(Into::into).collect(),
                ..Default::default()
            },
            host_fills,
        }
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

    /// Make a bundle hash from the given deserialized transaction array and host fills from this bundle.
    /// The hash is calculated as keccak256(tx_preimage + host_preimage).
    /// The tx_preimage is calculated as `keccak(tx_hash1 + tx_hash2 + ... + tx_hashn)`.
    /// The host_preimage is calculated as
    /// `keccak(NUM_OF_ASSETS_LE + asset1 + NUM_OF_FILLS_LE + asset1_user1 + user1_amount2 + ... + asset1_usern + asset1_amountn + ...)`.
    /// For the number of users/fills and amounts in the host_preimage, the amounts are serialized as little-endian U256 slice.
    pub fn bundle_hash(&self) -> B256 {
        let mut hasher = alloy::primitives::Keccak256::new();

        // Concatenate the transaction hashes, to then hash them. This is the tx_preimage.
        for tx in self.bundle.txs.iter() {
            // Calculate the tx hash (keccak256(encoded_signed_tx)) and append it to the tx_bytes.
            hasher.update(keccak256(tx).as_slice());
        }
        let tx_preimage = hasher.finalize();

        // Now, let's build the host_preimage. We do it in steps:
        // 1. Prefix the number of assets, encoded as a little-endian U256 slice.
        // 2. For each asset:
        // 3. Concatenate the asset address.
        // 4. Prefix the number of fills.
        // 5. For each fill, concatenate the user and amount, the latter encoded as a little-endian U256 slice.
        let mut hasher = alloy::primitives::Keccak256::new();

        // Prefix the list of users with the number of assets.
        hasher.update(U256::from(self.host_fills.len()).as_le_slice());

        for (asset, fills) in self.host_fills.iter() {
            // Concatenate the asset address.
            hasher.update(asset.as_slice());

            // Prefix the list of fills with the number of fills
            hasher.update(U256::from(fills.len()).as_le_slice());

            for (user, amount) in fills.iter() {
                // Concatenate the user address and amount for each fill.
                hasher.update(user.as_slice());
                hasher.update(amount.as_le_slice());
            }
        }

        // Hash the host pre-image.
        let host_preimage = hasher.finalize();

        let mut pre_image = alloy::primitives::Keccak256::new();
        pre_image.update(tx_preimage.as_slice());
        pre_image.update(host_preimage.as_slice());

        // Hash both tx and host hashes to get the final bundle hash.
        pre_image.finalize()
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
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SignetCallBundleResponse {
    #[serde(flatten)]
    inner: EthCallBundleResponse,
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
        Self { inner }
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
        base_fee: U256,
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
        result.gas_price = U256::from(tx.effective_gas_price(Some(base_fee.saturating_to())));
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
            host_fills: [(
                Address::repeat_byte(10),
                vec![(Address::repeat_byte(11), U256::from(12))].into_iter().collect(),
            )]
            .into_iter()
            .collect(),
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
