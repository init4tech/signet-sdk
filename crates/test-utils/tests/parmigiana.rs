//! Integration tests for the Parmigiana test harness.

use std::{env, time::Duration};

use alloy::{
    consensus::{Transaction, TypedTransaction},
    network::Ethereum,
    primitives::{Address, U256},
    providers::Provider,
};
use signet_constants::parmigiana;
use signet_test_utils::{
    parmigiana_context::{new_parmigiana_context, ParmigianaContext, RollupTransport},
    specs::{sign_tx_with_key_pair, simple_call, simple_send},
};
use signet_zenith::RollupPassage::exitCall;

const DEFAULT_MIN_RU_NATIVE_BALANCE: u64 = 1_000_000_000_000_000;
const LIVE_TX_TEST_NAME: &str = "ci_submit_transaction_and_wait_for_confirmation";
const LIVE_EXIT_BUNDLE_TEST_NAME: &str = "ci_submit_exit_bundle_and_wait_for_confirmation";
const LIVE_TEST_TRANSFER_AMOUNT: u64 = 1;

async fn check_chain_ids<H, R>(ctx: &ParmigianaContext<H, R>)
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    let host_chain_id = ctx.host_chain_id().await.unwrap();
    assert_eq!(host_chain_id, parmigiana::HOST_CHAIN_ID);

    let ru_chain_id = ctx.ru_chain_id().await.unwrap();
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

fn min_ru_native_balance(tx: &TypedTransaction) -> U256 {
    let configured_min =
        U256::from(env_u64("PARMIGIANA_MIN_RU_NATIVE_BALANCE", DEFAULT_MIN_RU_NATIVE_BALANCE));
    let tx_max_cost = U256::from(tx.gas_limit()) * U256::from(tx.max_fee_per_gas()) + tx.value();
    if configured_min > tx_max_cost {
        configured_min
    } else {
        tx_max_cost
    }
}

fn should_run_live(test_name: &str) -> bool {
    if live_tests_enabled() {
        return true;
    }
    eprintln!("skipping {test_name}: set PARMIGIANA_LIVE_TESTS=1 to enable live Parmigiana tests");
    false
}

async fn assert_sufficient_native_balance<H, R>(
    ctx: &ParmigianaContext<H, R>,
    owner: Address,
    minimum: U256,
    chain_label: &str,
    test_name: &str,
) where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    let balance = ctx.ru_native_balance_of(owner).await.unwrap();
    assert!(
        balance >= minimum,
        "{test_name} requires at least {minimum} {chain_label} native balance to cover max tx cost, found {balance}"
    );
}

fn ci_transfer(to: Address, nonce: u64, ru_chain_id: u64) -> TypedTransaction {
    simple_send(to, U256::from(LIVE_TEST_TRANSFER_AMOUNT), nonce, ru_chain_id)
}

fn ci_exit(host_recipient: Address, nonce: u64, ru_chain_id: u64) -> TypedTransaction {
    simple_call(
        parmigiana::RU_PASSAGE,
        &exitCall { hostRecipient: host_recipient },
        U256::from(LIVE_TEST_TRANSFER_AMOUNT),
        nonce,
        ru_chain_id,
    )
}

async fn run_submit_transaction_and_wait_for_confirmation() {
    let ctx = new_parmigiana_context(RollupTransport::Https).await.unwrap();
    let signer = ctx.primary_signer();
    let signer_addr = signer.address();
    let ru_chain_id = ctx.ru_chain_id().await.unwrap();
    let preview_tx = ci_transfer(ctx.users[2], 0, ru_chain_id);
    let min_balance = min_ru_native_balance(&preview_tx);
    assert_sufficient_native_balance(&ctx, signer_addr, min_balance, "RU", LIVE_TX_TEST_NAME).await;

    let nonce = ctx.ru_transaction_count(signer_addr).await.unwrap();
    let tx = ci_transfer(ctx.user(2), nonce, ru_chain_id);
    let envelope = sign_tx_with_key_pair(signer, tx);
    let tx_hash = *envelope.hash();

    let response = ctx.forward_rollup_transaction(envelope).await.unwrap();
    assert_eq!(response.tx_hash, tx_hash, "tx-cache returned unexpected tx hash");
    ctx.wait_for_transaction_in_cache(tx_hash, order_cache_timeout()).await.unwrap();
    let confirmed =
        ctx.wait_for_successful_ru_receipt(tx_hash, ru_receipt_timeout()).await.unwrap();
    println!(
        "PARMIGIANA_TX_ARTIFACT test={LIVE_TX_TEST_NAME} chain=rollup tx_hash={:#x} block_number={} block_hash={:#x} transaction_index={}",
        confirmed.tx_hash,
        confirmed.block_number,
        confirmed.block_hash,
        confirmed.transaction_index
    );
}

#[tokio::test]
async fn ci_submit_transaction_and_wait_for_confirmation() {
    if !should_run_live(LIVE_TX_TEST_NAME) {
        return;
    }
    run_submit_transaction_and_wait_for_confirmation().await;
}

async fn run_submit_exit_bundle_and_wait_for_confirmation() {
    let ctx = new_parmigiana_context(RollupTransport::Https).await.unwrap();
    let signer = ctx.primary_signer();
    let signer_addr = signer.address();
    let host_recipient = ctx.user(2);
    let ru_chain_id = ctx.ru_chain_id().await.unwrap();
    let preview_tx = ci_exit(host_recipient, 0, ru_chain_id);
    let min_balance = min_ru_native_balance(&preview_tx);
    assert_sufficient_native_balance(
        &ctx,
        signer_addr,
        min_balance,
        "RU",
        LIVE_EXIT_BUNDLE_TEST_NAME,
    )
    .await;

    let nonce = ctx.ru_transaction_count(signer_addr).await.unwrap();
    let tx = ci_exit(host_recipient, nonce, ru_chain_id);
    let envelope = sign_tx_with_key_pair(signer, tx);
    let tx_hash = *envelope.hash();

    let bundle = ctx.rollup_bundle_for_next_ru_block(vec![envelope]).await.unwrap();
    let response = ctx.forward_bundle(bundle).await.unwrap();

    let confirmed =
        ctx.wait_for_successful_ru_receipt(tx_hash, ru_receipt_timeout()).await.unwrap();
    println!(
        "PARMIGIANA_BUNDLE_ARTIFACT test={LIVE_EXIT_BUNDLE_TEST_NAME} bundle_id={} chain=rollup tx_hash={:#x} block_number={} block_hash={:#x} transaction_index={}",
        response.id,
        confirmed.tx_hash,
        confirmed.block_number,
        confirmed.block_hash,
        confirmed.transaction_index
    );
}

#[tokio::test]
async fn ci_submit_exit_bundle_and_wait_for_confirmation() {
    if !should_run_live(LIVE_EXIT_BUNDLE_TEST_NAME) {
        return;
    }
    run_submit_exit_bundle_and_wait_for_confirmation().await;
}
