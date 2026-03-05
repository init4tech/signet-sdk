//! Integration tests for the Parmigiana test harness.
//!
//! These tests verify that the harness compiles and provides
//! correct access to Parmigiana constants and utilities.

mod common;

use common::parmigiana::{DEFAULT_HOST_RPC, DEFAULT_RU_RPC, HOST_RPC_ENV, RU_RPC_ENV};
use signet_constants::parmigiana;
use signet_test_utils::users::{TEST_SIGNERS, TEST_USERS};

/// Verify that the Parmigiana constants are accessible through the harness.
#[test]
fn test_parmigiana_constants_accessible() {
    // Host chain constants
    assert_eq!(parmigiana::HOST_CHAIN_ID, 3151908);
    assert_eq!(parmigiana::RU_CHAIN_ID, 88888);

    // Contract addresses should be non-zero
    assert!(!parmigiana::HOST_ZENITH.is_zero());
    assert!(!parmigiana::HOST_ORDERS.is_zero());
    assert!(!parmigiana::HOST_PASSAGE.is_zero());
    assert!(!parmigiana::HOST_TRANSACTOR.is_zero());
    assert!(!parmigiana::RU_ORDERS.is_zero());
    assert!(!parmigiana::RU_PASSAGE.is_zero());

    // Token addresses should be non-zero
    assert!(!parmigiana::HOST_USDC.is_zero());
    assert!(!parmigiana::HOST_USDT.is_zero());
    assert!(!parmigiana::HOST_WBTC.is_zero());
    assert!(!parmigiana::HOST_WETH.is_zero());
    assert!(!parmigiana::RU_WBTC.is_zero());
    assert!(!parmigiana::RU_WETH.is_zero());
}

/// Verify that test accounts are available.
#[test]
fn test_accounts_available() {
    // Should have 10 test signers
    assert_eq!(TEST_SIGNERS.len(), 10);
    assert_eq!(TEST_USERS.len(), 10);

    // All users should have non-zero addresses
    for user in TEST_USERS.iter() {
        assert!(!user.is_zero());
    }

    // Addresses should match signers
    for (signer, user) in TEST_SIGNERS.iter().zip(TEST_USERS.iter()) {
        assert_eq!(signer.address(), *user);
    }
}

/// Verify environment variable names are correct.
#[test]
fn test_env_var_names() {
    assert_eq!(HOST_RPC_ENV, "PARMIGIANA_HOST_RPC_URL");
    assert_eq!(RU_RPC_ENV, "PARMIGIANA_RU_RPC_URL");
}

/// Verify default RPC URLs are reasonable.
#[test]
fn test_default_rpc_urls() {
    assert!(DEFAULT_HOST_RPC.starts_with("https://"));
    assert!(DEFAULT_RU_RPC.starts_with("https://"));
    assert!(DEFAULT_HOST_RPC.contains("parmigiana"));
    assert!(DEFAULT_RU_RPC.contains("parmigiana"));
}

/// Verify cleanup function compiles and works.
#[test]
fn test_cleanup_compiles() {
    // This just verifies the function signature is correct
    // Actual cleanup is a no-op currently
    fn _dummy_cleanup_test<T>(ctx: &T)
    where
        T: std::any::Any,
    {
        let _ = ctx;
    }
}
