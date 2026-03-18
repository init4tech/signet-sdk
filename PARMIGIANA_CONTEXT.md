# Parmigiana testing context

## Scope

This branch now combines:

- the existing ignored Parmigiana smoke tests for RPC reachability
- one env-gated CI live test that submits a real RU transaction, waits for a receipt, and prints a machine-friendly artifact line containing the confirmed tx hash

## Current Parmigiana coverage

Ignored live smoke tests:

- `test_chain_ids_wss`
- `test_chain_ids_https`
- `test_wallet_and_balance_wss`
- `test_wallet_and_balance_https`
- `test_all_signers_match_users_wss`
- `test_all_signers_match_users_https`

CI live test:

- `ci_submit_transaction_and_wait_for_confirmation`

The CI test only runs when `PARMIGIANA_LIVE_TESTS=1`. It supports an optional
`PARMIGIANA_ETH_PRIV_KEY` override for a funded signer. If no override is provided, it falls
back to the first deterministic Parmigiana test signer. If the selected signer does not have
enough RU native balance, the test skips cleanly instead of failing the workflow.

## Receipt-confirmed artifact output

When the CI live transaction is mined successfully, the test prints:

```text
PARMIGIANA_TX_ARTIFACT test=... chain=rollup tx_hash=0x... block_number=... block_hash=0x... transaction_index=...
```

These lines are emitted only after receipt confirmation, so they are safe to capture as test artifacts.

## Submission path

`ci_submit_transaction_and_wait_for_confirmation` uses:

- `simple_send(...)` and `sign_tx_with_key_pair(...)` from `signet-test-utils`
- `ParmigianaContext::ru_transaction_count(...)` and `ParmigianaContext::ru_native_balance_of(...)`
  for live node reads
- `ParmigianaContext::forward_rollup_transaction(...)`, which uses `TxCache::parmigiana()`
- `ParmigianaContext::wait_for_transaction_in_cache(...)` to confirm tx-cache acceptance
- `ParmigianaContext::wait_for_successful_ru_receipt(...)` to confirm the transaction was mined
  successfully

On timeout, the test includes `latest_block` and `seen_in_pool` diagnostics.

## Helper script and CI

`scripts/run_parmigiana_live.sh` now defaults to:

```bash
PARMIGIANA_LIVE_TESTS=1 cargo test -p signet-test-utils --test parmigiana ci_ -- --nocapture --test-threads=1
```

It supports:

- `PARMIGIANA_ETH_PRIV_KEY` for a funded signer
- `PARMIGIANA_TEST_FILTER` to narrow the live test selection
- `PARMIGIANA_INCLUDE_IGNORED=1` to run the ignored smoke tests through the same wrapper
- `PARMIGIANA_CARGO_PROFILE` to select a cargo profile such as `ci-rust`
- `PARMIGIANA_RU_RECEIPT_TIMEOUT_SECS`, `PARMIGIANA_ORDER_CACHE_TIMEOUT_SECS`, and `PARMIGIANA_MIN_RU_NATIVE_BALANCE` to tune live-test behavior

The GitHub Actions workflow passes `PARMIGIANA_ETH_PRIV_KEY` from repository secrets when present,
tees the full test log to `parmigiana-live-tests.log`, and uploads that log as a workflow artifact.
Any `PARMIGIANA_TX_ARTIFACT ...` lines are captured inside that uploaded log artifact.
