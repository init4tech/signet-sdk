#!/usr/bin/env bash
set -euo pipefail

# Run Parmigiana live integration tests with consistent defaults.
#
# Optional environment overrides:
# - PARMIGIANA_LIVE_TESTS (default: 1)
# - PARMIGIANA_ETH_PRIV_KEY (required funded signer for tx-confirming live tests)
# - PARMIGIANA_TEST_FILTER (default: ci_)
# - PARMIGIANA_INCLUDE_IGNORED (default: 0)
# - PARMIGIANA_CARGO_PROFILE (optional, example: ci-rust)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

: "${PARMIGIANA_LIVE_TESTS:=1}"
: "${PARMIGIANA_TEST_FILTER:=ci_}"
: "${PARMIGIANA_INCLUDE_IGNORED:=0}"

echo "Running Parmigiana live tests"
echo "  PARMIGIANA_LIVE_TESTS=$PARMIGIANA_LIVE_TESTS"
if [[ -n "$PARMIGIANA_TEST_FILTER" ]]; then
  echo "  PARMIGIANA_TEST_FILTER=$PARMIGIANA_TEST_FILTER"
else
  echo "  PARMIGIANA_TEST_FILTER=<all parmigiana tests>"
fi
echo "  PARMIGIANA_INCLUDE_IGNORED=$PARMIGIANA_INCLUDE_IGNORED"
if [[ -n "${PARMIGIANA_CARGO_PROFILE:-}" ]]; then
  echo "  PARMIGIANA_CARGO_PROFILE=$PARMIGIANA_CARGO_PROFILE"
fi
if [[ -n "${PARMIGIANA_ETH_PRIV_KEY:-}" ]]; then
  echo "  PARMIGIANA_ETH_PRIV_KEY is set"
else
  echo "  PARMIGIANA_ETH_PRIV_KEY is not set"
fi

export PARMIGIANA_LIVE_TESTS

cargo_args=(-p signet-test-utils --test parmigiana)
if [[ -n "${PARMIGIANA_CARGO_PROFILE:-}" ]]; then
  cargo_args+=(--profile "$PARMIGIANA_CARGO_PROFILE")
fi
if [[ -n "$PARMIGIANA_TEST_FILTER" ]]; then
  cargo_args+=("$PARMIGIANA_TEST_FILTER")
fi
cargo_args+=(-- --nocapture --test-threads=1)
if [[ "$PARMIGIANA_INCLUDE_IGNORED" == "1" ]]; then
  cargo_args+=(--ignored)
fi

cargo test "${cargo_args[@]}"
