//! Integration tests for the Parmigiana test harness.

use alloy::{network::Ethereum, providers::Provider};
use signet_constants::parmigiana;
use signet_test_utils::parmigiana_context::{
    new_parmigiana_context, ParmigianaContext, RollupTransport,
};

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
