#!/usr/bin/env bash
set -euo pipefail

# Run Parmigiana live integration tests with consistent defaults.
#
# Optional environment overrides:
# - PARMIGIANA_TEST_FILTER (optional single-test filter)
# - PARMIGIANA_INCLUDE_IGNORED (default: 1)
# - PARMIGIANA_CARGO_PROFILE (optional, example: ci-rust)

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

: "${PARMIGIANA_TEST_FILTER:=}"
: "${PARMIGIANA_INCLUDE_IGNORED:=1}"

echo "Running Parmigiana live tests"
if [[ -n "$PARMIGIANA_TEST_FILTER" ]]; then
  echo "  PARMIGIANA_TEST_FILTER=$PARMIGIANA_TEST_FILTER"
else
  echo "  PARMIGIANA_TEST_FILTER=<all parmigiana tests>"
fi
echo "  PARMIGIANA_INCLUDE_IGNORED=$PARMIGIANA_INCLUDE_IGNORED"
if [[ -n "${PARMIGIANA_CARGO_PROFILE:-}" ]]; then
  echo "  PARMIGIANA_CARGO_PROFILE=$PARMIGIANA_CARGO_PROFILE"
fi

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
