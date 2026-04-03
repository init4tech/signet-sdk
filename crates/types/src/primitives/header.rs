//! Validated signet header newtypes.
//!
//! Signet headers have stricter invariants than standard Ethereum headers.
//! These newtypes enforce those invariants at construction time and eagerly
//! cache the block hash.

use alloy::{
    consensus::{constants::EMPTY_ROOT_HASH, BlockHeader, Header},
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

/// Check that `transactions_root` and `receipts_root` are `EMPTY_ROOT_HASH`.
pub(crate) fn check_roots_empty(header: &Header) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if header.transactions_root() != EMPTY_ROOT_HASH {
        bad.push("transactions_root");
    }
    if header.receipts_root() != EMPTY_ROOT_HASH {
        bad.push("receipts_root");
    }
    bad
}

/// Check that `transactions_root` and `receipts_root` are NOT `EMPTY_ROOT_HASH`.
#[allow(dead_code)]
pub(crate) fn check_roots_non_empty(header: &Header) -> Vec<&'static str> {
    let mut bad = Vec::new();
    if header.transactions_root() == EMPTY_ROOT_HASH {
        bad.push("transactions_root");
    }
    if header.receipts_root() == EMPTY_ROOT_HASH {
        bad.push("receipts_root");
    }
    bad
}

/// A validated signet header (V1) wrapping a [`Sealed<Header>`].
///
/// V1 headers have all shared fields at their defaults and both
/// `transactions_root` and `receipts_root` set to `B256::ZERO`.
/// The block hash is eagerly cached on construction.
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

/// A validated signet header (V2) wrapping a [`Sealed<Header>`].
///
/// V2 headers have all shared fields at their defaults but both
/// `transactions_root` and `receipts_root` set to non-zero values.
/// The block hash is eagerly cached on construction.
///
/// **Unstable** — this type is experimental and not yet used in production.
#[cfg(feature = "experimental")]
#[deprecated(note = "SignetHeaderV2 is unstable and not yet used in production")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignetHeaderV2(Sealed<Header>);

#[cfg(feature = "experimental")]
#[allow(deprecated)]
impl SignetHeaderV2 {
    /// Consume the wrapper, returning the inner [`Sealed<Header>`].
    pub fn into_inner(self) -> Sealed<Header> {
        self.0
    }

    /// Get the cached block hash.
    pub const fn hash(&self) -> B256 {
        self.0.hash()
    }
}

#[cfg(feature = "experimental")]
#[allow(deprecated)]
impl TryFrom<Header> for SignetHeaderV2 {
    type Error = SignetHeaderError;

    fn try_from(header: Header) -> Result<Self, Self::Error> {
        let must_be_default = check_shared_defaults(&header);
        let must_not_be_default = check_roots_non_empty(&header);

        if must_be_default.is_empty() && must_not_be_default.is_empty() {
            Ok(Self(Sealed::new(header)))
        } else {
            Err(SignetHeaderError { must_be_default, must_not_be_default })
        }
    }
}

#[cfg(feature = "experimental")]
#[allow(deprecated)]
impl Deref for SignetHeaderV2 {
    type Target = Sealed<Header>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(feature = "experimental")]
#[allow(deprecated)]
impl AsRef<Header> for SignetHeaderV2 {
    fn as_ref(&self) -> &Header {
        &self.0
    }
}

#[cfg(feature = "experimental")]
#[allow(deprecated)]
impl BlockHeader for SignetHeaderV2 {
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

    /// Create a valid V1 header (roots are EMPTY_ROOT_HASH from default).
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
        let header = valid_v1_header();
        let v1 = SignetHeaderV1::try_from(header).unwrap();
        let sealed = v1.into_inner();
        // Sealed<Header> has a cached hash
        let _ = sealed.hash();
    }

    #[test]
    fn v1_hash_is_accessible() {
        let v1 = SignetHeaderV1::try_from(valid_v1_header()).unwrap();
        // hash() should not panic
        let _ = v1.hash();
    }

    #[test]
    fn v1_deref_accesses_header_fields() {
        let header = valid_v1_header();
        let v1 = SignetHeaderV1::try_from(header.clone()).unwrap();
        assert_eq!(v1.number(), header.number());
        assert_eq!(v1.timestamp(), header.timestamp());
    }

    #[cfg(feature = "experimental")]
    mod v2_tests {
        use super::*;

        fn valid_v2_header() -> Header {
            Header {
                transactions_root: B256::repeat_byte(0x01),
                receipts_root: B256::repeat_byte(0x02),
                ..Default::default()
            }
        }

        #[allow(deprecated)]
        #[test]
        fn v2_accepts_valid_rooted_header() {
            SignetHeaderV2::try_from(valid_v2_header()).unwrap();
        }

        #[allow(deprecated)]
        #[test]
        fn v2_rejects_empty_root_transactions_root() {
            let header = Header {
                // transactions_root left as EMPTY_ROOT_HASH from default
                receipts_root: B256::repeat_byte(0x01),
                ..Default::default()
            };
            let err = SignetHeaderV2::try_from(header).unwrap_err();
            assert!(err.must_not_be_default.contains(&"transactions_root"));
        }

        #[allow(deprecated)]
        #[test]
        fn v2_rejects_empty_root_receipts_root() {
            let header = Header {
                transactions_root: B256::repeat_byte(0x01),
                // receipts_root left as EMPTY_ROOT_HASH from default
                ..Default::default()
            };
            let err = SignetHeaderV2::try_from(header).unwrap_err();
            assert!(err.must_not_be_default.contains(&"receipts_root"));
        }

        #[allow(deprecated)]
        #[test]
        fn v2_rejects_non_default_shared_field() {
            let header = Header {
                transactions_root: B256::repeat_byte(0x01),
                receipts_root: B256::repeat_byte(0x02),
                state_root: B256::repeat_byte(0x03),
                ..Default::default()
            };
            let err = SignetHeaderV2::try_from(header).unwrap_err();
            assert!(err.must_be_default.contains(&"state_root"));
        }

        #[allow(deprecated)]
        #[test]
        fn v2_reports_both_violation_kinds() {
            // state_root is non-default, roots are EMPTY_ROOT_HASH (default)
            let header = Header { state_root: B256::repeat_byte(0x01), ..Default::default() };
            let err = SignetHeaderV2::try_from(header).unwrap_err();
            assert!(!err.must_be_default.is_empty());
            assert!(!err.must_not_be_default.is_empty());
        }

        #[allow(deprecated)]
        #[test]
        fn v2_into_inner_returns_sealed() {
            let v2 = SignetHeaderV2::try_from(valid_v2_header()).unwrap();
            let sealed = v2.into_inner();
            let _ = sealed.hash();
        }
    }
}
