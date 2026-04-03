//! [`SignetHeaderV1`] — validated signet header without computed roots.

use super::{check_roots_empty, check_shared_defaults, SignetHeaderError};
use alloy::{
    consensus::{BlockHeader, Header},
    primitives::{Address, BlockNumber, Bloom, Bytes, Sealed, B256, B64, U256},
};
use std::ops::Deref;

/// A validated signet header (V1) wrapping a [`Sealed<Header>`].
///
/// V1 headers have all shared fields at their defaults and both
/// `transactions_root` and `receipts_root` set to [`EMPTY_ROOT_HASH`].
/// The block hash is eagerly cached on construction.
///
/// [`EMPTY_ROOT_HASH`]: alloy::consensus::constants::EMPTY_ROOT_HASH
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignetHeaderV1(Sealed<Header>);

impl SignetHeaderV1 {
    /// Consume the wrapper, returning the inner [`Sealed<Header>`].
    pub fn into_inner(self) -> Sealed<Header> {
        self.0
    }

    /// Get the cached block hash.
    pub const fn hash(&self) -> B256 {
        self.0.hash()
    }
}

impl TryFrom<Header> for SignetHeaderV1 {
    type Error = SignetHeaderError;

    fn try_from(header: Header) -> Result<Self, Self::Error> {
        let mut must_be_default = check_shared_defaults(&header);
        must_be_default.extend(check_roots_empty(&header));

        if must_be_default.is_empty() {
            Ok(Self(Sealed::new(header)))
        } else {
            Err(SignetHeaderError { must_be_default, must_not_be_default: Vec::new() })
        }
    }
}

impl Deref for SignetHeaderV1 {
    type Target = Sealed<Header>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Header> for SignetHeaderV1 {
    fn as_ref(&self) -> &Header {
        &self.0
    }
}

impl BlockHeader for SignetHeaderV1 {
    fn parent_hash(&self) -> B256 {
        self.0.parent_hash()
    }
    fn ommers_hash(&self) -> B256 {
        self.0.ommers_hash()
    }
    fn beneficiary(&self) -> Address {
        self.0.beneficiary()
    }
    fn state_root(&self) -> B256 {
        self.0.state_root()
    }
    fn transactions_root(&self) -> B256 {
        self.0.transactions_root()
    }
    fn receipts_root(&self) -> B256 {
        self.0.receipts_root()
    }
    fn withdrawals_root(&self) -> Option<B256> {
        self.0.withdrawals_root()
    }
    fn logs_bloom(&self) -> Bloom {
        self.0.logs_bloom()
    }
    fn difficulty(&self) -> U256 {
        self.0.difficulty()
    }
    fn number(&self) -> BlockNumber {
        self.0.number()
    }
    fn gas_limit(&self) -> u64 {
        self.0.gas_limit()
    }
    fn gas_used(&self) -> u64 {
        self.0.gas_used()
    }
    fn timestamp(&self) -> u64 {
        self.0.timestamp()
    }
    fn mix_hash(&self) -> Option<B256> {
        self.0.mix_hash()
    }
    fn nonce(&self) -> Option<B64> {
        self.0.nonce()
    }
    fn base_fee_per_gas(&self) -> Option<u64> {
        self.0.base_fee_per_gas()
    }
    fn blob_gas_used(&self) -> Option<u64> {
        self.0.blob_gas_used()
    }
    fn excess_blob_gas(&self) -> Option<u64> {
        self.0.excess_blob_gas()
    }
    fn parent_beacon_block_root(&self) -> Option<B256> {
        self.0.parent_beacon_block_root()
    }
    fn requests_hash(&self) -> Option<B256> {
        self.0.requests_hash()
    }
    fn extra_data(&self) -> &Bytes {
        self.0.extra_data()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_v1_header() -> Header {
        Header::default()
    }

    #[test]
    fn v1_accepts_valid_rootless_header() {
        SignetHeaderV1::try_from(valid_v1_header()).unwrap();
    }

    #[test]
    fn v1_rejects_non_default_transactions_root() {
        let mut header = valid_v1_header();
        header.transactions_root = B256::repeat_byte(1);
        let err = SignetHeaderV1::try_from(header).unwrap_err();
        assert!(err.must_be_default.contains(&"transactions_root"));
    }

    #[test]
    fn v1_rejects_non_default_receipts_root() {
        let mut header = valid_v1_header();
        header.receipts_root = B256::repeat_byte(1);
        let err = SignetHeaderV1::try_from(header).unwrap_err();
        assert!(err.must_be_default.contains(&"receipts_root"));
    }

    #[test]
    fn v1_rejects_non_default_state_root() {
        let mut header = valid_v1_header();
        header.state_root = B256::repeat_byte(1);
        let err = SignetHeaderV1::try_from(header).unwrap_err();
        assert!(err.must_be_default.contains(&"state_root"));
    }

    #[test]
    fn v1_reports_all_violations_at_once() {
        let header = Header {
            state_root: B256::repeat_byte(1),
            transactions_root: B256::repeat_byte(2),
            receipts_root: B256::repeat_byte(3),
            ..Default::default()
        };
        let err = SignetHeaderV1::try_from(header).unwrap_err();
        assert_eq!(err.must_be_default.len(), 3);
        assert!(err.must_be_default.contains(&"state_root"));
        assert!(err.must_be_default.contains(&"transactions_root"));
        assert!(err.must_be_default.contains(&"receipts_root"));
    }

    #[test]
    fn v1_into_inner_returns_sealed() {
        let v1 = SignetHeaderV1::try_from(valid_v1_header()).unwrap();
        let sealed = v1.into_inner();
        let _ = sealed.hash();
    }

    #[test]
    fn v1_hash_is_accessible() {
        let v1 = SignetHeaderV1::try_from(valid_v1_header()).unwrap();
        let _ = v1.hash();
    }

    #[test]
    fn v1_deref_accesses_header_fields() {
        let header = valid_v1_header();
        let v1 = SignetHeaderV1::try_from(header.clone()).unwrap();
        assert_eq!(v1.number(), header.number());
        assert_eq!(v1.timestamp(), header.timestamp());
    }
}
