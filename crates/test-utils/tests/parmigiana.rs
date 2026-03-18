//! Integration tests for the Parmigiana test harness.

use std::{env, time::Duration};

use alloy::{
    network::{Ethereum, ReceiptResponse},
    primitives::{Address, TxHash, U256},
    providers::Provider,
    signers::{k256::ecdsa::SigningKey, local::PrivateKeySigner},
};
use signet_constants::parmigiana;
use signet_test_utils::{
    parmigiana_context::{new_parmigiana_context, ParmigianaContext, RollupTransport},
    specs::{sign_tx_with_key_pair, simple_send},
};
use signet_tx_cache::{types::TransactionResponse, TxCache};
use tokio::time::{sleep, Instant};

const MIN_RU_NATIVE_BALANCE: u64 = 1_000_000_000_000_000;

async fn check_chain_ids<H, R>(ctx: &ParmigianaContext<H, R>)
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    let host_chain_id = ctx.host_provider.get_chain_id().await.unwrap();
    assert_eq!(host_chain_id, parmigiana::HOST_CHAIN_ID);

    let ru_chain_id = ctx.ru_provider.get_chain_id().await.unwrap();
    assert_eq!(ru_chain_id, parmigiana::RU_CHAIN_ID);
}

async fn check_wallet_and_balance<H, R>(ctx: &ParmigianaContext<H, R>)
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    assert_eq!(ctx.primary_signer().address(), ctx.primary_user());

    let _wallet = ctx.wallet(0);

    // Smoke-test that balance queries work (don't assert a specific value).
    let _host_bal = ctx.host_balance(0).await.unwrap();
    let _ru_bal = ctx.ru_balance(0).await.unwrap();
}

fn check_all_signers_match_users<H, R>(ctx: &ParmigianaContext<H, R>)
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    for (signer, user) in ctx.signers.iter().zip(ctx.users.iter()) {
        assert_eq!(signer.address(), *user);
    }
}

#[tokio::test]
#[ignore = "requires Parmigiana testnet access"]
async fn test_chain_ids_wss() {
    let ctx = new_parmigiana_context(RollupTransport::Wss).await.unwrap();
    check_chain_ids(&ctx).await;
}

#[tokio::test]
#[ignore = "requires Parmigiana testnet access"]
async fn test_chain_ids_https() {
    let ctx = new_parmigiana_context(RollupTransport::Https).await.unwrap();
    check_chain_ids(&ctx).await;
}

#[tokio::test]
#[ignore = "requires Parmigiana testnet access"]
async fn test_wallet_and_balance_wss() {
    let ctx = new_parmigiana_context(RollupTransport::Wss).await.unwrap();
    check_wallet_and_balance(&ctx).await;
}

#[tokio::test]
#[ignore = "requires Parmigiana testnet access"]
async fn test_wallet_and_balance_https() {
    let ctx = new_parmigiana_context(RollupTransport::Https).await.unwrap();
    check_wallet_and_balance(&ctx).await;
}

#[tokio::test]
#[ignore = "requires Parmigiana testnet access"]
async fn test_all_signers_match_users_wss() {
    let ctx = new_parmigiana_context(RollupTransport::Wss).await.unwrap();
    check_all_signers_match_users(&ctx);
}

#[tokio::test]
#[ignore = "requires Parmigiana testnet access"]
async fn test_all_signers_match_users_https() {
    let ctx = new_parmigiana_context(RollupTransport::Https).await.unwrap();
    check_all_signers_match_users(&ctx);
}

async fn wait_for_receipt<P>(provider: &P, tx_hash: TxHash, timeout: Duration)
where
    P: Provider<Ethereum>,
{
    let started = Instant::now();
    loop {
        let receipt = provider.get_transaction_receipt(tx_hash).await.unwrap();
        if receipt.is_some() {
            return;
        }

        let seen_in_pool = provider.get_transaction_by_hash(tx_hash).await.ok().flatten().is_some();
        if started.elapsed() > timeout {
            let latest_block = provider.get_block_number().await.unwrap_or_default();
            panic!(
                "timed out waiting for receipt for tx {tx_hash:#x}; latest_block={latest_block}; seen_in_pool={seen_in_pool}"
            );
        }
        sleep(Duration::from_secs(2)).await;
    }
}

async fn wait_for_successful_receipt<P>(
    provider: &P,
    tx_hash: TxHash,
    timeout: Duration,
    test_name: &str,
) where
    P: Provider<Ethereum>,
{
    wait_for_receipt(provider, tx_hash, timeout).await;
    let receipt = provider
        .get_transaction_receipt(tx_hash)
        .await
        .unwrap()
        .expect("receipt should exist after wait");
    assert!(receipt.status(), "tx {tx_hash:#x} should succeed");

    let block_number = receipt.block_number().expect("receipt missing block number");
    let block_hash = receipt.block_hash().expect("receipt missing block hash");
    let transaction_index = receipt.transaction_index().expect("receipt missing tx index");
    println!(
        "PARMIGIANA_TX_ARTIFACT test={test_name} chain=rollup tx_hash={tx_hash:#x} block_number={block_number} block_hash={block_hash:#x} transaction_index={transaction_index}"
    );
}

async fn tx_exists_in_cache(tx_cache: &TxCache, tx_hash: TxHash) -> bool {
    let mut cursor = None;
    loop {
        let page = tx_cache
            .get_transactions(cursor.clone())
            .await
            .expect("tx-cache transaction query should succeed");
        if page.transactions.iter().any(|tx| *tx.hash() == tx_hash) {
            return true;
        }
        let Some(next_cursor) = page.next_cursor().cloned() else {
            return false;
        };
        cursor = Some(next_cursor);
    }
}

async fn wait_for_tx_in_cache(tx_cache: &TxCache, tx_hash: TxHash, timeout: Duration) {
    let started = Instant::now();
    loop {
        if tx_exists_in_cache(tx_cache, tx_hash).await {
            return;
        }
        assert!(
            started.elapsed() <= timeout,
            "timed out waiting for tx {tx_hash:#x} to appear in tx-cache"
        );
        sleep(Duration::from_secs(2)).await;
    }
}

fn live_tests_enabled() -> bool {
    env::var("PARMIGIANA_LIVE_TESTS")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name).ok().and_then(|value| value.parse::<u64>().ok()).unwrap_or(default)
}

fn ru_receipt_timeout() -> Duration {
    Duration::from_secs(env_u64("PARMIGIANA_RU_RECEIPT_TIMEOUT_SECS", 240))
}

fn order_cache_timeout() -> Duration {
    Duration::from_secs(env_u64("PARMIGIANA_ORDER_CACHE_TIMEOUT_SECS", 60))
}

fn min_ru_native_balance() -> U256 {
    U256::from(env_u64("PARMIGIANA_MIN_RU_NATIVE_BALANCE", MIN_RU_NATIVE_BALANCE))
}

fn should_run_live(test_name: &str) -> bool {
    if live_tests_enabled() {
        return true;
    }
    eprintln!("skipping {test_name}: set PARMIGIANA_LIVE_TESTS=1 to enable live Parmigiana tests");
    false
}

fn signer_from_env() -> Option<PrivateKeySigner> {
    let key = env::var("PARMIGIANA_ETH_PRIV_KEY").ok()?;
    let key = key.trim().trim_start_matches("0x");
    let bytes = alloy::hex::decode(key).expect("PARMIGIANA_ETH_PRIV_KEY must be valid hex");
    let bytes: [u8; 32] =
        bytes.try_into().expect("PARMIGIANA_ETH_PRIV_KEY must be exactly 32 bytes");
    Some(PrivateKeySigner::from(
        SigningKey::from_slice(&bytes)
            .expect("PARMIGIANA_ETH_PRIV_KEY must be a valid secp256k1 key"),
    ))
}

fn signer_for_test<H, R>(ctx: &ParmigianaContext<H, R>) -> (PrivateKeySigner, Address)
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    if let Some(signer) = signer_from_env() {
        let address = signer.address();
        return (signer, address);
    }
    (ctx.signers[0].clone(), ctx.users[0])
}

async fn forward_raw_transaction_with_debug(
    tx_cache: &TxCache,
    tx: &alloy::consensus::TxEnvelope,
    label: &str,
) -> TransactionResponse {
    let url = tx_cache.url().join("transactions").unwrap();
    let response = tx_cache.client().post(url).json(tx).send().await.unwrap();
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    assert!(status.is_success(), "tx-cache rejected {label} with status {status}: {body}");
    serde_json::from_str(&body).expect("tx-cache should return valid transaction JSON")
}

async fn skip_if_insufficient_native_balance<P>(
    provider: &P,
    owner: Address,
    minimum: U256,
    chain_label: &str,
    test_name: &str,
) -> bool
where
    P: Provider<Ethereum>,
{
    let balance = provider.get_balance(owner).await.unwrap();
    if balance >= minimum {
        return false;
    }

    eprintln!(
        "skipping {test_name}: {chain_label} native balance {balance} is below required minimum {minimum}"
    );
    true
}

async fn run_submit_transaction_and_wait_for_confirmation() {
    let ctx = new_parmigiana_context(RollupTransport::Https).await.unwrap();
    let (signer, signer_addr) = signer_for_test(&ctx);
    if skip_if_insufficient_native_balance(
        &ctx.ru_provider,
        signer_addr,
        min_ru_native_balance(),
        "RU",
        "ci_submit_transaction_and_wait_for_confirmation",
    )
    .await
    {
        return;
    }

    let tx_cache = TxCache::parmigiana();
    let nonce = ctx.ru_provider.get_transaction_count(signer_addr).await.unwrap();
    let tx = simple_send(ctx.users[2], U256::from(1u64), nonce, parmigiana::RU_CHAIN_ID);
    let envelope = sign_tx_with_key_pair(&signer, tx);
    let tx_hash = *envelope.hash();

    let response = forward_raw_transaction_with_debug(
        &tx_cache,
        &envelope,
        "ci_submit_transaction_and_wait_for_confirmation",
    )
    .await;
    assert_eq!(response.tx_hash, tx_hash, "tx-cache returned unexpected tx hash");
    wait_for_tx_in_cache(&tx_cache, tx_hash, order_cache_timeout()).await;
    wait_for_successful_receipt(
        &ctx.ru_provider,
        tx_hash,
        ru_receipt_timeout(),
        "ci_submit_transaction_and_wait_for_confirmation",
    )
    .await;
}

#[tokio::test]
async fn ci_submit_transaction_and_wait_for_confirmation() {
    if !should_run_live("ci_submit_transaction_and_wait_for_confirmation") {
        return;
    }
    run_submit_transaction_and_wait_for_confirmation().await;
}
