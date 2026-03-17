# Testing Guide for Signet SDK Contributors

This guide covers testing patterns, conventions, and utilities for contributing to the Signet SDK codebase.

## Table of Contents

1. [Test ID Naming Conventions](#test-id-naming-conventions)
2. [Running Tests](#running-tests)
3. [Parmigiana Test Setup](#parmigiana-test-setup)
4. [Mock Provider Usage](#mock-provider-usage)
5. [Adding Test Fixtures](#adding-test-fixtures)

---

## Test ID Naming Conventions

Signet uses a structured test ID system to categorize and identify tests across the codebase. Tests should use these prefixes in their function names and documentation.

> **Note:** This is an aspirational convention for new tests. Only 7 of 124 existing tests currently use TC-* naming — adoption is not expected retroactively.

### Prefix Reference

| Prefix | Domain | Description |
|--------|--------|-------------|
| `TC-EXT-*` | Extraction | Tests for `signet-extract` — extracting events from host chain blocks |
| `TC-ORD-*` | Orders | Tests for `signet-orders` — order handling, filling, and submission |
| `TC-RPC-*` | RPC | Tests for RPC interactions and provider behavior |
| `TC-BLD-*` | Builder | Tests for `signet-sim` — block building and simulation |
| `TC-712-*` | EIP-712 | Tests for EIP-712 signing and typed data verification |

### Example Test with ID

```rust
/// TC-EXT-001: Extract enters from a host block containing only native enters.
///
/// Verifies that the extractor correctly identifies Enter events, preserving
/// recipient addresses and amounts, with no spurious enter_tokens or transacts.
#[test]
fn tc_ext_001_extract_enters_only() {
    let hbs = HostBlockSpec::test()
        .with_block_number(TEST_SYS.host_deploy_height() + 1)
        .enter(TEST_USERS[0], (GWEI_TO_WEI * 5) as usize)
        .enter(TEST_USERS[1], (GWEI_TO_WEI * 3) as usize);
    let (chain, _) = hbs.to_chain();

    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();

    hbs.assert_conforms(&extracts);

    // Additional assertions...
}
```

### Best Practices

1. **Docstring format**: Start with the test ID, followed by a colon and brief description
2. **Function name**: Use the test ID as a prefix (lowercase, underscores)
3. **Single responsibility**: Each test should verify one specific behavior
4. **Clear assertions**: Use descriptive assertion messages when helpful

---

## Running Tests

### Basic Test Commands

```bash
# Run all tests for a specific crate
cargo t -p signet-test-utils

# Run all tests with all features enabled
cargo test --all-features

# Run tests without default features (for coverage)
cargo test --no-default-features

# Run a specific test by name
cargo test tc_ext_001

# Run ignored tests (e.g., network-dependent tests)
cargo test --ignored
```

### Test Categories

#### Unit Tests
Standard tests that run locally without external dependencies:

```bash
cargo test -p signet-extract
cargo test -p signet-orders
cargo test -p signet-evm
```

#### Integration Tests
Located in `crates/test-utils/tests/`, these test cross-crate functionality:

```bash
cargo test -p signet-test-utils --test extract
cargo test -p signet-test-utils --test evm
cargo test -p signet-test-utils --test orders_filler
cargo test -p signet-test-utils --test fill_behavior
```

#### Network Tests (Ignored by Default)
Tests requiring Parmigiana testnet access are marked with `#[ignore]`:

```bash
# Run Parmigiana integration tests
cargo test -p signet-test-utils --test parmigiana -- --ignored
```

### Pre-commit Checks

Before submitting a PR, run the checks listed in [CLAUDE.md](../CLAUDE.md) under **Commands / Pre-commit**.

---

## Parmigiana Test Setup

The Parmigiana testnet is Signet's public test environment. The `signet-test-utils` crate provides a test harness for running integration tests against it.

### Prerequisites

1. Test accounts must be pre-funded on the Parmigiana testnet
2. Network access to Parmigiana RPC endpoints

### Using ParmigianaContext

```rust
use signet_test_utils::parmigiana_context::{
    new_parmigiana_context, ParmigianaContext, RollupTransport,
};

#[tokio::test]
#[ignore = "requires Parmigiana testnet access"]
async fn test_with_parmigiana() {
    // Create context with HTTPS transport
    let ctx = new_parmigiana_context(RollupTransport::Https)
        .await
        .unwrap();
    
    // Access providers
    let host_chain_id = ctx.host_provider.get_chain_id().await.unwrap();
    let ru_chain_id = ctx.ru_provider.get_chain_id().await.unwrap();
    
    // Use test signers (deterministic keys)
    let primary_signer = ctx.primary_signer();
    let wallet = ctx.wallet(0); // Get EthereumWallet for index 0
    
    // Query balances
    let host_balance = ctx.host_balance(0).await.unwrap();
    let ru_balance = ctx.ru_balance(0).await.unwrap();
}
```

### Transport Options

```rust
pub enum RollupTransport {
    Https,  // https://rpc.parmigiana.signet.sh (default)
    Wss,    // wss://rpc.parmigiana.signet.sh
}
```

### RPC Endpoints

| Chain | URL |
|-------|-----|
| Host Chain | `https://host-rpc.parmigiana.signet.sh` |
| Rollup (HTTP) | `https://rpc.parmigiana.signet.sh` |
| Rollup (WS) | `wss://rpc.parmigiana.signet.sh` |

### Test Signers

The harness provides 10 deterministic test signers derived from simple keys:

```rust
// Available signers and their addresses
ctx.signers    // &[PrivateKeySigner; 10]
ctx.users      // &[Address; 10]

// Convenience methods
ctx.primary_signer()  // First signer
ctx.primary_user()    // First user address
ctx.wallet(index)     // EthereumWallet from signer at index
```

---

## Mock Provider Usage

The `signet-test-utils` crate provides mock implementations for testing without network dependencies.

### MockOrderSubmitter

Captures submitted orders for verification:

```rust
use signet_test_utils::orders::MockOrderSubmitter;

#[tokio::test]
async fn test_order_submission() {
    let submitter = MockOrderSubmitter::new();
    
    // Use submitter in your test...
    submitter.submit_order(signed_order).await.unwrap();
    
    // Verify submissions
    let orders = submitter.submitted_orders();
    assert_eq!(orders.len(), 1);
}
```

### MockOrderSource

Returns a predefined list of orders:

```rust
use signet_test_utils::orders::{MockOrderSource, default_test_orders};

#[tokio::test]
async fn test_with_mock_orders() {
    // Create source with test orders
    let orders = default_test_orders().await;
    let source = MockOrderSource::new(orders);
    
    // Or create empty source
    let empty_source = MockOrderSource::empty();
}
```

### MockBundleSubmitter

Captures submitted bundles:

```rust
use signet_test_utils::orders::MockBundleSubmitter;

#[tokio::test]
async fn test_bundle_submission() {
    let submitter = MockBundleSubmitter::new();
    
    // After test execution...
    let bundles = submitter.submitted_bundles();
    assert_eq!(bundles.len(), 5);
    
    // Verify bundle contents
    for bundle in bundles {
        assert_eq!(bundle.bundle.txs.len(), 3);
        assert_eq!(bundle.host_txs().len(), 1);
    }
}
```

### MockTxBuilder

Pre-fills transactions with gas and nonce values for testing:

```rust
use signet_test_utils::orders::mock_tx_builder;

#[tokio::test]
async fn test_tx_building() {
    let wallet = TEST_SIGNERS[1].clone();
    let provider = mock_tx_builder(wallet, TEST_SYS.ru_chain_id());
    
    // Push mock RPC responses if needed
    provider.asserter().push_success(&U256::from(100));
    
    // Use provider for transaction building...
}
```

### MockFillSubmitter

Captures fill submissions:

```rust
use signet_test_utils::orders::MockFillSubmitter;

#[tokio::test]
async fn test_fill_submission() {
    let submitter = MockFillSubmitter::new();
    
    // After test execution...
    let submissions = submitter.submissions();
    assert!(!submissions.is_empty());
}
```

### TestOrderBuilder

Builds test orders with a fluent interface:

```rust
use signet_test_utils::orders::TestOrderBuilder;

#[tokio::test]
async fn build_test_order() {
    let signer = &TEST_SIGNERS[0];
    
    let order = TestOrderBuilder::new()
        .with_input(Address::repeat_byte(0x11), U256::from(1000))
        .with_output(
            Address::repeat_byte(0x22),
            U256::from(500),
            signer.address(),
            TEST_SYS.host_chain_id(),
        )
        .with_nonce(1)
        .sign(signer)
        .await;
}
```

---

## Adding Test Fixtures

### Test Users and Signers

Pre-defined test accounts are available in `signet_test_utils::users`:

```rust
use signet_test_utils::users::{TEST_SIGNERS, TEST_USERS};

// 10 deterministic signers (keys [1..=10])
let signer = &TEST_SIGNERS[0];  // PrivateKeySigner
let address = TEST_USERS[0];     // Address

// Signers derived from simple byte arrays:
// TEST_SIGNERS[0] = SigningKey::from_slice(&[1u8; 32])
// TEST_SIGNERS[1] = SigningKey::from_slice(&[2u8; 32])
// etc.
```

### Test Constants

System constants for test environments:

```rust
use signet_test_utils::test_constants::*;

// Chain IDs
let host_chain = HOST_CHAIN_ID;  // 1
let ru_chain = RU_CHAIN_ID;      // 15

// Contract addresses (test environment)
let orders = RU_ORDERS;
let passage = RU_PASSAGE;

// Token addresses
let host_usdc = HOST_USDC;
let host_weth = HOST_WETH;
let ru_weth = RU_WETH;

// System constants struct
let sys = TEST_SYS;  // SignetSystemConstants
```

### Creating Test EVMs

Set up in-memory EVM instances for testing:

```rust
use signet_test_utils::evm::{test_signet_evm, test_signet_evm_with_inspector};

#[test]
fn test_with_evm() {
    // Basic EVM with pre-deployed contracts and funded test users
    let evm = test_signet_evm();
    
    // With custom inspector
    let inspector = NoOpInspector;
    let evm = test_signet_evm_with_inspector(inspector);
    
    // Pre-deployed contracts include:
    // - RU_ORDERS and RU_PASSAGE system contracts
    // - Counter contract at COUNTER_TEST_ADDRESS
    // - Revert contract at REVERT_TEST_ADDRESS
    // - WBTC and WETH token contracts
    // - TEST_USERS funded with 1000 ETH each
}
```

### Creating Test Simulation Environments

For block building and simulation tests:

```rust
use signet_test_utils::evm::{rollup_sim_env, host_sim_env, test_sim_env};

// Rollup-only environment
let ru_env = rollup_sim_env();

// Host-only environment
let host_env = host_sim_env();

// Full simulation environment with both chains
let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
let sim = test_sim_env(deadline);
```

### Block Specs for Testing

Build test blocks with specific events:

```rust
use signet_test_utils::specs::{HostBlockSpec, RuBlockSpec};

// Create a host block with various events
let host_block = HostBlockSpec::test()
    .with_block_number(101)
    // Native ETH enters
    .enter(TEST_USERS[0], 1_000_000_000)
    .enter(TEST_USERS[1], 2_000_000_000)
    // Token enters
    .enter_token(TEST_USERS[2], 1_000_000, HOST_USDC)
    // L1→L2 transacts
    .simple_transact(TEST_USERS[0], TEST_USERS[3], [0xde, 0xad], 0)
    // Fills
    .fill(HOST_USDC, TEST_USERS[4], 500_000)
    // Submit a rollup block
    .submit_block(ru_block);

// Convert to a Chain for extraction testing
let (chain, sidecar) = host_block.to_chain();

// Create rollup block specs
let ru_block = RuBlockSpec::test()
    .with_gas_limit(30_000_000)
    .with_reward_address(Address::repeat_byte(0x99));

// Add transactions to rollup block
ru_block.add_simple_send(&TEST_SIGNERS[0], TEST_USERS[1], U256::from(GWEI_TO_WEI), 0);
```

### Test Contract Utilities

Pre-deployed contracts for testing:

```rust
use signet_test_utils::contracts::counter::{counter, COUNTER_TEST_ADDRESS};
use signet_test_utils::contracts::system::{orders, passage};

// Get contract instances
let counter_contract = counter(provider);
let orders_contract = orders(provider);
let passage_contract = passage(provider);
```

### Transaction Helpers

Utilities for creating test transactions:

```rust
use signet_test_utils::specs::{
    make_wallet,
    sign_tx_with_key_pair,
    simple_send,
    simple_call,
    signed_simple_send,
    signed_simple_call,
    simple_bundle,
};

// Create a wallet with deterministic key
let wallet = make_wallet(1);  // Key from [1u8; 32]

// Create and sign a simple transfer
let tx = simple_send(to, amount, nonce, chain_id);
let signed = sign_tx_with_key_pair(&wallet, tx);

// Or use the convenience function
let signed = signed_simple_send(&wallet, to, amount, nonce, chain_id);

// Create a contract call
let tx = simple_call(contract, &call_data, value, nonce, chain_id);

// Create a bundle from transactions
let bundle = simple_bundle(rollup_txs, host_txs, target_block);
```

### Chain Utilities

Create fake blocks and chains for testing:

```rust
use signet_test_utils::chain::{fake_block, Chain};

// Create a fake block at a specific number
let block = fake_block(100);

// Create a chain from a block
let chain = Chain::from_block(block, execution_outcome);

// Append blocks to chain
chain.append_block(next_block, next_outcome);
```

---

## Test Style Guidelines

Following the project's CLAUDE.md conventions:

1. **Fail fast**: Use `unwrap()` in tests instead of returning `Result`
2. **No test result types**: Tests should panic on failure, not return errors
3. **Descriptive names**: Test function names should describe what's being tested
4. **Setup comments**: Document complex test setup with inline comments
5. **Single assertion focus**: Each test should verify one logical unit

```rust
// Good: Clear, focused test
#[test]
fn extractor_filters_wrong_chain_id() {
    let hbs = HostBlockSpec::test()
        .ignored_enter(TEST_USERS[0], GWEI_TO_WEI);  // Wrong chain ID
    let (chain, _) = hbs.to_chain();
    
    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();
    
    assert_eq!(extracts.enters().count(), 0);  // Filtered out
}

// Bad: Returns Result, unclear purpose
#[test]
fn test_thing() -> Result<(), Box<dyn Error>> {
    // ...
    Ok(())
}
```

---

## Additional Resources

- [signet-test-utils crate](../crates/test-utils/) — Full source for test utilities
- [CONTRIBUTING.md](../CONTRIBUTING.md) — General contribution guidelines
- [CLAUDE.md](../CLAUDE.md) — Code style and conventions
- [Signet Documentation](https://signet.sh/docs) — Official Signet docs
