//! Proptests covering `signet-types` deserialization paths that consume
//! untrusted JSON.
//!
//! Regression coverage for ENG-2288 (`SignedOrder::order_hash` panic on
//! malformed signatures) and a forward-looking guarantee that no
//! `serde::Deserialize` impl on a SDK-exposed type panics on arbitrary
//! input — every malformed payload must surface as a `serde_json::Error`,
//! never an unwinding panic.
use alloy::primitives::Bytes;
use proptest::prelude::*;
use serde::Deserialize;
use signet_types::SignedOrder;
use signet_zenith::serde_helpers::{deserialize_non_empty_vec, deserialize_signature_bytes};

#[derive(Debug, Deserialize)]
struct SigWrap {
    #[serde(deserialize_with = "deserialize_signature_bytes")]
    signature: Bytes,
}

#[derive(Debug, Deserialize)]
struct VecWrap {
    #[serde(deserialize_with = "deserialize_non_empty_vec")]
    items: Vec<u64>,
}

fn hex_string(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("0x");
    for b in bytes {
        use std::fmt::Write;
        write!(&mut s, "{b:02x}").unwrap();
    }
    s
}

fn signed_order_json(sig_bytes: &[u8], permitted_count: usize, outputs_count: usize) -> String {
    let permitted: Vec<String> = (0..permitted_count)
        .map(|i| format!(r#"{{"token":"0x{:040x}","amount":"0x{i:x}"}}"#, i as u128))
        .collect();
    let outputs: Vec<String> = (0..outputs_count)
        .map(|i| {
            format!(
                r#"{{"token":"0x{:040x}","amount":"0x{i:x}","recipient":"0x{:040x}","chainId":{i}}}"#,
                i as u128, i as u128
            )
        })
        .collect();
    format!(
        r#"{{
            "permit": {{
                "permitted": [{}],
                "nonce": "0x0",
                "deadline": "0xffffffffffffffff"
            }},
            "owner": "0x0000000000000000000000000000000000000000",
            "signature": "{}",
            "outputs": [{}]
        }}"#,
        permitted.join(","),
        hex_string(sig_bytes),
        outputs.join(",")
    )
}

proptest! {
    /// `deserialize_signature_bytes` accepts iff the decoded byte string
    /// is exactly 65 bytes.
    #[test]
    fn signature_helper_accepts_only_65_bytes(bytes in prop::collection::vec(any::<u8>(), 0..200)) {
        let json = format!(r#"{{"signature":"{}"}}"#, hex_string(&bytes));
        match serde_json::from_str::<SigWrap>(&json) {
            Ok(w) => prop_assert_eq!(w.signature.len(), 65),
            Err(_) => prop_assert_ne!(bytes.len(), 65),
        }
    }

    /// `deserialize_non_empty_vec` accepts iff the decoded vector is
    /// non-empty.
    #[test]
    fn non_empty_vec_helper_rejects_empty(items in prop::collection::vec(any::<u64>(), 0..16)) {
        let json = format!(
            r#"{{"items":[{}]}}"#,
            items.iter().map(u64::to_string).collect::<Vec<_>>().join(",")
        );
        match serde_json::from_str::<VecWrap>(&json) {
            Ok(w) => prop_assert!(!w.items.is_empty()),
            Err(_) => prop_assert!(items.is_empty()),
        }
    }

    /// `SignedOrder` deserialization is total: any combination of
    /// signature length, permitted count, and outputs count either
    /// produces an `Ok` value satisfying all three structural
    /// invariants, or an `Err`. It never panics.
    #[test]
    fn signed_order_deserialize_total(
        sig_bytes in prop::collection::vec(any::<u8>(), 0..130),
        permitted_count in 0usize..6,
        outputs_count in 0usize..6,
    ) {
        let json = signed_order_json(&sig_bytes, permitted_count, outputs_count);
        match serde_json::from_str::<SignedOrder>(&json) {
            Ok(order) => {
                prop_assert_eq!(sig_bytes.len(), 65);
                prop_assert!(permitted_count > 0);
                prop_assert!(outputs_count > 0);
                prop_assert_eq!(order.permit().signature.len(), 65);
                prop_assert!(!order.permit().permit.permitted.is_empty());
                prop_assert!(!order.outputs().is_empty());
                // order_hash() must not panic on any value the
                // Deserialize impl admits.
                let _ = order.order_hash();
            }
            Err(_) => {
                prop_assert!(
                    sig_bytes.len() != 65 || permitted_count == 0 || outputs_count == 0
                );
            }
        }
    }

    /// Well-formed `SignedOrder` JSON survives a serde round-trip with a
    /// stable `order_hash`.
    #[test]
    fn signed_order_roundtrip(
        permitted_count in 1usize..6,
        outputs_count in 1usize..6,
    ) {
        let sig = vec![0u8; 65];
        let json = signed_order_json(&sig, permitted_count, outputs_count);
        let order: SignedOrder = serde_json::from_str(&json).unwrap();
        let hash = *order.order_hash();
        let reserialized = serde_json::to_string(&order).unwrap();
        let decoded: SignedOrder = serde_json::from_str(&reserialized).unwrap();
        prop_assert_eq!(decoded.order_hash(), &hash);
    }
}

// Sanity check that arbitrary garbage JSON doesn't panic the
// deserializer — complements the structured proptests above by covering
// completely malformed shapes.
proptest! {
    #[test]
    fn arbitrary_string_never_panics(s in ".{0,256}") {
        let _ = serde_json::from_str::<SignedOrder>(&s);
    }
}
