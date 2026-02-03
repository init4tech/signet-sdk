# Signet SDK

## Commands

- `cargo +nightly fmt` - format
- `cargo clippy -p <crate> --all-features --all-targets` - lint with features
- `cargo clippy -p <crate> --no-default-features --all-targets` - lint without
- `cargo t -p <crate>` - test specific crate

Pre-commit: clippy (both feature sets) + fmt. Never use `cargo check/build`.

## Style

- Functional combinators over imperative control flow
- `let else` for early returns, avoid nesting
- No glob imports; group imports from same crate
- Private by default, `pub(crate)` for internal, `pub` for API only
- `thiserror` for library errors, `eyre` for apps, never `anyhow`
- Builders for structs with >4 fields or multiple same-type fields
- Tests: fail fast with `unwrap()`, never return `Result`

## Crate Notes

### signet-types

Test vectors for external SDK verification exist in:
- `crates/types/src/signing/mod.rs` - EIP-712 signing hash vectors
- `crates/types/src/signing/order.rs` - order serialization vectors

These are `#[ignore]`d. Run with `--ignored` flag to generate JSON output.
