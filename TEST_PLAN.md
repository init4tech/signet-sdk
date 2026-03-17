# Parmigiana test plan

The current Parmigiana coverage in this branch is the ignored integration suite in
`crates/test-utils/tests/parmigiana.rs`. These tests are read-only RPC smoke tests:

- `test_chain_ids_wss`
- `test_chain_ids_https`
- `test_wallet_and_balance_wss`
- `test_wallet_and_balance_https`
- `test_all_signers_match_users_wss`
- `test_all_signers_match_users_https`

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
cargo test -p signet-test-utils --test parmigiana test_wallet_and_balance_https -- --ignored --nocapture
cargo test -p signet-test-utils --test parmigiana test_all_signers_match_users_https -- --ignored --nocapture
```

4. Run the full ignored Parmigiana suite

```bash
./scripts/run_parmigiana_live.sh
```

5. CI-equivalent run

```bash
PARMIGIANA_CARGO_PROFILE=ci-rust ./scripts/run_parmigiana_live.sh
```
