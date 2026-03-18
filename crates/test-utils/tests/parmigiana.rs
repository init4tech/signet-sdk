//! Integration tests for the Parmigiana test harness.

use std::{env, time::Duration};

use alloy::{
    network::Ethereum,
    primitives::{Address, U256},
    providers::Provider,
    signers::{k256::ecdsa::SigningKey, local::PrivateKeySigner},
};
use signet_constants::parmigiana;
use signet_test_utils::{
    parmigiana_context::{new_parmigiana_context, ParmigianaContext, RollupTransport},
    specs::{sign_tx_with_key_pair, simple_send},
};

const MIN_RU_NATIVE_BALANCE: u64 = 1_000_000_000_000_000;

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

async fn skip_if_insufficient_native_balance<H, R>(
    ctx: &ParmigianaContext<H, R>,
    owner: Address,
    minimum: U256,
    chain_label: &str,
    test_name: &str,
) -> bool
where
    H: Provider<Ethereum>,
    R: Provider<Ethereum>,
{
    let balance = ctx.ru_native_balance_of(owner).await.unwrap();
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
        &ctx,
        signer_addr,
        min_ru_native_balance(),
        "RU",
        "ci_submit_transaction_and_wait_for_confirmation",
    )
    .await
    {
        return;
    }

    let nonce = ctx.ru_transaction_count(signer_addr).await.unwrap();
    let tx = simple_send(ctx.users[2], U256::from(1u64), nonce, ctx.ru_chain_id().await.unwrap());
    let envelope = sign_tx_with_key_pair(&signer, tx);
    let tx_hash = *envelope.hash();

    let response = ctx.forward_rollup_transaction(envelope).await.unwrap();
    assert_eq!(response.tx_hash, tx_hash, "tx-cache returned unexpected tx hash");
    ctx.wait_for_transaction_in_cache(tx_hash, order_cache_timeout()).await.unwrap();
    let confirmed =
        ctx.wait_for_successful_ru_receipt(tx_hash, ru_receipt_timeout()).await.unwrap();
    println!(
        "PARMIGIANA_TX_ARTIFACT test=ci_submit_transaction_and_wait_for_confirmation chain=rollup tx_hash={:#x} block_number={} block_hash={:#x} transaction_index={}",
        confirmed.tx_hash,
        confirmed.block_number,
        confirmed.block_hash,
        confirmed.transaction_index
    );
}

#[tokio::test]
async fn ci_submit_transaction_and_wait_for_confirmation() {
    if !should_run_live("ci_submit_transaction_and_wait_for_confirmation") {
        return;
    }
    run_submit_transaction_and_wait_for_confirmation().await;
}
