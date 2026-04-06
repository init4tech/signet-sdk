//! Integration tests for the bundle submitter.
//!
//! These tests require network access to the Parmigiana testnet and are
//! marked `#[ignore]`. Run with `cargo t -p signet-test-utils -- --ignored`.

use signet_test_utils::bundle_submitter::{
    new_bundle_submitter, new_bundle_submitter_with_config, BundleSubmitterConfig,
};
use std::time::Duration;

#[tokio::test]
#[ignore = "requires Parmigiana testnet access"]
async fn test_bundle_submitter_single_iteration() {
    signet_test_utils::init_tracing();

    let mut submitter = new_bundle_submitter().await.unwrap();
    let results = submitter.run_iterations(1).await;

    assert_eq!(results.len(), 1);
    // The first result should succeed if the testnet is reachable.
    results[0].as_ref().unwrap();
}

#[tokio::test]
#[ignore = "requires Parmigiana testnet access"]
async fn test_bundle_submitter_with_custom_config() {
    signet_test_utils::init_tracing();

    let config =
        BundleSubmitterConfig::builder().interval(Duration::from_secs(1)).wallet_index(2).build();

    let mut submitter = new_bundle_submitter_with_config(config).await.unwrap();
    let nonce = submitter.refresh_nonce().await.unwrap();
    let block = submitter.current_block().await.unwrap();

    // Nonce and block should be reasonable non-negative values.
    assert!(block > 0, "block number should be positive, got {block}");

    let bundle = submitter.create_test_bundle().await.unwrap();
    assert!(!bundle.txs().is_empty(), "bundle should contain at least one transaction");
    assert_eq!(bundle.host_txs().len(), 0, "bundle should have no host transactions");

    // Verify nonce didn't change just from creating a bundle.
    assert_eq!(nonce, submitter.refresh_nonce().await.unwrap());
}
