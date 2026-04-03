//! Validated signet header newtypes.
//!
//! Signet headers have stricter invariants than standard Ethereum headers.
//! These newtypes enforce those invariants at construction time.

use alloy::{
    consensus::{BlockHeader, Header},
    primitives::{Address, BlockNumber, Bloom, Bytes, Sealed, B256, B64, U256},
};
use std::ops::Deref;

/// Error returned when a [`Header`] violates signet header invariants.
#[derive(Debug, thiserror::Error)]
#[error("invalid signet header: expected default: {must_be_default:?}, expected non-default: {must_not_be_default:?}")]
pub struct SignetHeaderError {
    /// Field names that must be default but were not.
    pub must_be_default: Vec<&'static str>,
    /// Field names that must not be default but were.
    pub must_not_be_default: Vec<&'static str>,
}

/// Check that shared fields equal their defaults.
pub(crate) fn check_shared_defaults(header: &Header) -> Vec<&'static str> {
    let d = Header::default();
    let mut bad = Vec::new();
    if header.ommers_hash() != d.ommers_hash() {
        bad.push("ommers_hash");
    }
    if header.state_root() != d.state_root() {
        bad.push("state_root");
    }
    if header.withdrawals_root() != d.withdrawals_root() {
        bad.push("withdrawals_root");
    }
    if header.blob_gas_used() != d.blob_gas_used() {
        bad.push("blob_gas_used");
    }
    if header.excess_blob_gas() != d.excess_blob_gas() {
        bad.push("excess_blob_gas");
    }
    if header.requests_hash() != d.requests_hash() {
        bad.push("requests_hash");
    }
    if header.extra_data() != d.extra_data() {
        bad.push("extra_data");
    }
    bad
}

/// Check that `transactions_root` and `receipts_root` are `B256::ZERO`.
pub(crate) fn check_roots_default(header: &Header) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if header.transactions_root() != B256::ZERO {
        bad.push("transactions_root");
    }
    if header.receipts_root() != B256::ZERO {
        bad.push("receipts_root");
    }
    bad
}

/// Check that `transactions_root` and `receipts_root` are NOT `B256::ZERO`.
#[allow(dead_code)]
pub(crate) fn check_roots_non_default(header: &Header) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if header.transactions_root() == B256::ZERO {
        bad.push("transactions_root");
    }
    if header.receipts_root() == B256::ZERO {
        bad.push("receipts_root");
    }
    bad
}

/// A validated signet header (V1) with roots required to be default (zero).
///
/// V1 headers have all shared fields at their defaults and both
/// `transactions_root` and `receipts_root` set to `B256::ZERO`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignetHeaderV1(Header);

/// A sealed [`SignetHeaderV1`] with a cached block hash.
pub type SealedSignetHeaderV1 = Sealed<SignetHeaderV1>;

impl SignetHeaderV1 {
    /// Consume the wrapper, returning the inner [`Header`].
    pub fn into_inner(self) -> Header {
        self.0
    }

    /// Consume the wrapper, sealing the inner [`Header`].
    pub fn into_sealed_header(self) -> Sealed<Header> {
        Sealed::new(self.0)
    }

    /// Borrow the inner header and seal the reference.
    pub fn sealed_ref(&self) -> Sealed<&Header> {
        Sealed::new_ref(&self.0)
    }
}

impl TryFrom<Header> for SignetHeaderV1 {
    type Error = SignetHeaderError;

    fn try_from(header: Header) -> Result<Self, Self::Error> {
        let must_be_default = {
            let mut v = check_shared_defaults(&header);
            v.extend(check_roots_default(&header));
            v
        };
        let must_not_be_default = Vec::new();

        if must_be_default.is_empty() && must_not_be_default.is_empty() {
            Ok(Self(header))
        } else {
            Err(SignetHeaderError { must_be_default, must_not_be_default })
        }
    }
}

impl Deref for SignetHeaderV1 {
    type Target = Header;

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

    /// Create a valid V1 header (transaction/receipt roots zeroed, rest default).
    fn valid_v1_header() -> Header {
        Header { transactions_root: B256::ZERO, receipts_root: B256::ZERO, ..Default::default() }
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
    fn v1_into_inner_roundtrips() {
        let header = valid_v1_header();
        let v1 = SignetHeaderV1::try_from(header.clone()).unwrap();
        assert_eq!(v1.into_inner(), header);
    }

    #[test]
    fn v1_deref_accesses_header_fields() {
        let header = valid_v1_header();
        let v1 = SignetHeaderV1::try_from(header.clone()).unwrap();
        assert_eq!(v1.number(), header.number());
        assert_eq!(v1.timestamp(), header.timestamp());
    }
}
