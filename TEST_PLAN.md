# Parmigiana test plan

The current Parmigiana coverage has two layers:

- Ignored live smoke tests in `crates/test-utils/tests/parmigiana.rs` for RPC reachability.
- A non-ignored `ci_submit_transaction_and_wait_for_confirmation` live test that:
  submits a signed RU transaction through tx-cache, waits for a mined receipt, and prints a
  `PARMIGIANA_TX_ARTIFACT ... tx_hash=...` line on success.

The CI-style live test is gated by `PARMIGIANA_LIVE_TESTS=1`. It uses
`PARMIGIANA_ETH_PRIV_KEY` when provided, otherwise it falls back to the first deterministic
Parmigiana test signer. The test skips instead of failing when the chosen signer does not have
enough RU native balance to pay for the transaction. Live node and tx-cache interactions now go
through `signet-test-utils::parmigiana_context` helper methods rather than ad hoc request code in
the test itself.

1. Fast local compile checks

```bash
cargo test -p signet-orders --no-run
cargo test -p signet-test-utils --test bundle --no-run
cargo test -p signet-test-utils --test parmigiana --no-run
```

2. Run deterministic local tests

```bash
cargo test -p signet-orders -- --nocapture
cargo test -p signet-test-utils --test bundle -- --nocapture
```

3. Run Parmigiana live tests directly

```bash
cargo test -p signet-test-utils --test parmigiana test_chain_ids_https -- --ignored --nocapture
PARMIGIANA_LIVE_TESTS=1 cargo test -p signet-test-utils --test parmigiana ci_submit_transaction_and_wait_for_confirmation -- --nocapture
```

4. Run the default CI-style Parmigiana live test

```bash
./scripts/run_parmigiana_live.sh
```

That script defaults to:

```bash
PARMIGIANA_LIVE_TESTS=1 \
PARMIGIANA_TEST_FILTER=ci_ \
PARMIGIANA_INCLUDE_IGNORED=0 \
cargo test -p signet-test-utils --test parmigiana ci_ -- --nocapture --test-threads=1
```

5. Run with a funded signer to get receipt-confirmed tx hash artifacts

```bash
PARMIGIANA_LIVE_TESTS=1 \
PARMIGIANA_ETH_PRIV_KEY=0x<funded-32-byte-private-key> \
PARMIGIANA_CARGO_PROFILE=ci-rust \
./scripts/run_parmigiana_live.sh
```

6. Run the ignored smoke suite through the helper script

```bash
PARMIGIANA_TEST_FILTER=test_ \
PARMIGIANA_INCLUDE_IGNORED=1 \
./scripts/run_parmigiana_live.sh
```
