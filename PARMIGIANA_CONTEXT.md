# Parmigiana testing context

## Scope

This branch currently adds Parmigiana test collateral around the existing ignored
integration suite in `crates/test-utils/tests/parmigiana.rs`.

That suite is read-only. It does not submit transactions, bundles, or orders. It
only checks that the published Parmigiana RPC endpoints are reachable and that the
deterministic test identities in `signet-test-utils` line up with the expected
addresses.

## Current Parmigiana coverage

The integration binary defines six ignored tests:

- `test_chain_ids_wss`
- `test_chain_ids_https`
- `test_wallet_and_balance_wss`
- `test_wallet_and_balance_https`
- `test_all_signers_match_users_wss`
- `test_all_signers_match_users_https`

All six require live network access, so they are marked `#[ignore = "requires Parmigiana testnet access"]`.

## Harness behavior

`crates/test-utils/src/parmigiana_context.rs` builds a `ParmigianaContext` with:

- host RPC: `https://host-rpc.parmigiana.signet.sh`
- rollup HTTP RPC: `https://rpc.parmigiana.signet.sh`
- rollup WebSocket RPC: `wss://rpc.parmigiana.signet.sh`
- deterministic signers and addresses from `signet-test-utils`

The harness validates the expected host and rollup chain IDs during setup and exposes
helpers for fetching balances on both chains.

## Helper script and CI

`scripts/run_parmigiana_live.sh` is a thin wrapper around:

```bash
cargo test -p signet-test-utils --test parmigiana -- --ignored --nocapture --test-threads=1
```

It supports two optional knobs:

- `PARMIGIANA_TEST_FILTER` to run a single ignored test
- `PARMIGIANA_CARGO_PROFILE` to select a cargo profile such as `ci-rust`

The GitHub Actions workflow uses:

```bash
PARMIGIANA_CARGO_PROFILE=ci-rust ./scripts/run_parmigiana_live.sh
```

## Validated commands on this branch

These commands were checked locally while preparing the branch:

```bash
bash -n scripts/run_parmigiana_live.sh
cargo test -p signet-orders -- --nocapture
cargo test -p signet-test-utils --test bundle -- --nocapture
cargo test -p signet-test-utils --test parmigiana -- --list
```

## Next useful extensions

If this branch is meant to evolve beyond smoke coverage, the next step is to add
new Parmigiana integration tests rather than extending the runbook further. The
current suite does not yet cover transaction submission or bundle/order execution.
