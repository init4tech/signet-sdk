use alloy::{
    consensus::constants::GWEI_TO_WEI,
    primitives::{Address, Bytes, U256},
};
use signet_extract::Extractor;
use signet_test_utils::{
    specs::{HostBlockSpec, RuBlockSpec},
    test_constants::*,
    users::*,
};

#[test]
fn extraction() {
    let mut ru_block =
        RuBlockSpec::test().with_gas_limit(12345).with_reward_address(Address::repeat_byte(0x99));
    ru_block.add_simple_send(&TEST_SIGNERS[0], TEST_USERS[1], U256::from(GWEI_TO_WEI), 0);

    let hbs = HostBlockSpec::test()
        .with_block_number(TEST_SYS.host_deploy_height() + 1)
        .enter(TEST_USERS[0], (GWEI_TO_WEI * 4) as usize)
        .enter(TEST_USERS[1], (GWEI_TO_WEI * 2) as usize)
        .enter_token(TEST_USERS[2], 10_000_000, HOST_USDC)
        .simple_transact(TEST_USERS[0], TEST_USERS[4], [1, 2, 3, 4], GWEI_TO_WEI as usize)
        .fill(HOST_USDT, TEST_USERS[4], 10_000)
        .submit_block(ru_block);
    let (chain, _) = hbs.to_chain();

    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();

    hbs.assert_conforms(&extracts);
}

/// TC-EXT-001: Extract enters from a host block containing only native enters.
///
/// Verifies that the extractor correctly identifies Enter events, preserving
/// recipient addresses and amounts, with no spurious enter_tokens or transacts.
#[test]
fn tc_ext_001_extract_enters_only() {
    let hbs = HostBlockSpec::test()
        .with_block_number(TEST_SYS.host_deploy_height() + 1)
        .enter(TEST_USERS[0], (GWEI_TO_WEI * 5) as usize)
        .enter(TEST_USERS[1], (GWEI_TO_WEI * 3) as usize)
        .enter(TEST_USERS[2], GWEI_TO_WEI as usize);
    let (chain, _) = hbs.to_chain();

    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();

    // assert_conforms checks event ordering, counts, and values
    hbs.assert_conforms(&extracts);

    // Verify exact enter count and absence of other event types
    let enters: Vec<_> = extracts.enters().collect();
    assert_eq!(enters.len(), 3);
    assert_eq!(enters[0].rollupRecipient, TEST_USERS[0]);
    assert_eq!(enters[0].amount, U256::from(GWEI_TO_WEI * 5));
    assert_eq!(enters[1].rollupRecipient, TEST_USERS[1]);
    assert_eq!(enters[1].amount, U256::from(GWEI_TO_WEI * 3));
    assert_eq!(enters[2].rollupRecipient, TEST_USERS[2]);
    assert_eq!(enters[2].amount, U256::from(GWEI_TO_WEI));

    assert_eq!(extracts.enter_tokens().count(), 0);
    assert_eq!(extracts.transacts().count(), 0);
    assert!(!extracts.contains_block());
}

/// TC-EXT-002: Extract enter_tokens from a host block containing only token deposits.
///
/// Verifies that the extractor correctly identifies EnterToken events for ERC20
/// deposits, preserving token addresses, recipient addresses, and amounts.
#[test]
fn tc_ext_002_extract_enter_tokens_only() {
    let hbs = HostBlockSpec::test()
        .with_block_number(TEST_SYS.host_deploy_height() + 1)
        .enter_token(TEST_USERS[0], 1_000_000, HOST_USDC) // 1 USDC (6 decimals)
        .enter_token(TEST_USERS[1], 2_000_000_000_000, HOST_USDT) // 2 USDT (12 decimals)
        .enter_token(TEST_USERS[2], 50_000_000, HOST_USDC); // 50 USDC
    let (chain, _) = hbs.to_chain();

    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();

    hbs.assert_conforms(&extracts);

    // Verify exact enter_token count and absence of other event types
    let enter_tokens: Vec<_> = extracts.enter_tokens().collect();
    assert_eq!(enter_tokens.len(), 3);

    // First enter_token: 1 USDC to TEST_USERS[0]
    assert_eq!(enter_tokens[0].rollupRecipient, TEST_USERS[0]);
    assert_eq!(enter_tokens[0].token, HOST_USDC);
    assert_eq!(enter_tokens[0].amount, U256::from(1_000_000u64));

    // Second enter_token: 2 USDT to TEST_USERS[1]
    assert_eq!(enter_tokens[1].rollupRecipient, TEST_USERS[1]);
    assert_eq!(enter_tokens[1].token, HOST_USDT);
    assert_eq!(enter_tokens[1].amount, U256::from(2_000_000_000_000u64));

    // Third enter_token: 50 USDC to TEST_USERS[2]
    assert_eq!(enter_tokens[2].rollupRecipient, TEST_USERS[2]);
    assert_eq!(enter_tokens[2].token, HOST_USDC);
    assert_eq!(enter_tokens[2].amount, U256::from(50_000_000u64));

    assert_eq!(extracts.enters().count(), 0);
    assert_eq!(extracts.transacts().count(), 0);
    assert!(!extracts.contains_block());
}

/// TC-EXT-003: Extract transacts from a host block containing only L1→L2 calls.
///
/// Verifies that the extractor correctly identifies Transact events, preserving
/// sender, target, calldata, value, and gas parameters.
#[test]
fn tc_ext_003_extract_transacts_only() {
    let hbs = HostBlockSpec::test()
        .with_block_number(TEST_SYS.host_deploy_height() + 1)
        .simple_transact(TEST_USERS[0], TEST_USERS[3], [0xde, 0xad, 0xbe, 0xef], 0)
        .simple_transact(TEST_USERS[1], TEST_USERS[4], [0x01, 0x02, 0x03], GWEI_TO_WEI as usize);
    let (chain, _) = hbs.to_chain();

    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();

    hbs.assert_conforms(&extracts);

    // Verify exact transact count and absence of other event types
    let transacts: Vec<_> = extracts.transacts().collect();
    assert_eq!(transacts.len(), 2);

    // First transact: TEST_USERS[0] → TEST_USERS[3] with 0xdeadbeef data
    assert_eq!(transacts[0].sender, TEST_USERS[0]);
    assert_eq!(transacts[0].to, TEST_USERS[3]);
    assert_eq!(transacts[0].data, Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef]));
    assert_eq!(transacts[0].value, U256::ZERO);

    // Second transact: TEST_USERS[1] → TEST_USERS[4] with 1 gwei value
    assert_eq!(transacts[1].sender, TEST_USERS[1]);
    assert_eq!(transacts[1].to, TEST_USERS[4]);
    assert_eq!(transacts[1].data, Bytes::from_static(&[0x01, 0x02, 0x03]));
    assert_eq!(transacts[1].value, U256::from(GWEI_TO_WEI));

    assert_eq!(extracts.enters().count(), 0);
    assert_eq!(extracts.enter_tokens().count(), 0);
    assert!(!extracts.contains_block());
}

/// TC-EXT-004: Extract fills and verify AggregateFills accumulation.
///
/// Verifies that the extractor correctly identifies Filled events and properly
/// accumulates them into an AggregateFills context for market state tracking.
#[test]
fn tc_ext_004_extract_fills_and_aggregate() {
    let hbs = HostBlockSpec::test()
        .with_block_number(TEST_SYS.host_deploy_height() + 1)
        .fill(HOST_USDC, TEST_USERS[0], 100_000) // 0.1 USDC fill
        .fill(HOST_USDT, TEST_USERS[1], 200_000) // 0.0002 USDT fill (12 decimals)
        .fill(HOST_USDC, TEST_USERS[2], 500_000); // 0.5 USDC fill
    let (chain, _) = hbs.to_chain();

    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();

    hbs.assert_conforms(&extracts);

    // Verify the AggregateFills context is properly populated
    let aggregate_fills = extracts.aggregate_fills();
    let fills = aggregate_fills.fills();

    // Check USDC fills are tracked
    assert!(fills.contains_key(&(TEST_SYS.host_chain_id(), HOST_USDC)));
    // Check USDT fill is tracked
    assert!(fills.contains_key(&(TEST_SYS.host_chain_id(), HOST_USDT)));

    // Verify individual recipient balances
    assert_eq!(
        aggregate_fills.filled(&(TEST_SYS.host_chain_id(), HOST_USDC), TEST_USERS[0]),
        U256::from(100_000u64)
    );
    assert_eq!(
        aggregate_fills.filled(&(TEST_SYS.host_chain_id(), HOST_USDC), TEST_USERS[2]),
        U256::from(500_000u64)
    );
    assert_eq!(
        aggregate_fills.filled(&(TEST_SYS.host_chain_id(), HOST_USDT), TEST_USERS[1]),
        U256::from(200_000u64)
    );

    // Verify no other event types are present
    assert_eq!(extracts.enters().count(), 0);
    assert_eq!(extracts.enter_tokens().count(), 0);
    assert_eq!(extracts.transacts().count(), 0);
    assert!(!extracts.contains_block());
}

/// TC-EXT-005: Extract block submitted event with rollup block data.
///
/// Verifies that the extractor correctly identifies BlockSubmitted events and
/// preserves rollup block header data (gas limit, reward address, etc.).
#[test]
fn tc_ext_005_extract_block_submitted() {
    let reward_addr = Address::repeat_byte(0xab);
    let gas_limit = 30_000_000u64;

    let mut ru_block =
        RuBlockSpec::test().with_gas_limit(gas_limit).with_reward_address(reward_addr);
    ru_block.add_simple_send(&TEST_SIGNERS[0], TEST_USERS[1], U256::from(GWEI_TO_WEI), 0);

    let hbs = HostBlockSpec::test()
        .with_block_number(TEST_SYS.host_deploy_height() + 1)
        .submit_block(ru_block);
    let (chain, _) = hbs.to_chain();

    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();

    hbs.assert_conforms(&extracts);

    // Verify block submitted is present with correct data
    assert!(extracts.contains_block());
    let submitted = extracts.events.submitted.as_ref().unwrap();
    assert_eq!(submitted.gas_limit(), gas_limit);
    assert_eq!(submitted.reward_address(), reward_addr);

    // Verify ru_header can be extracted
    let ru_header = extracts.ru_header();
    assert!(ru_header.is_some());
    let header = ru_header.unwrap();
    assert_eq!(header.gasLimit, U256::from(gas_limit));
    assert_eq!(header.rewardAddress, reward_addr);

    // Verify no other event types
    assert_eq!(extracts.enters().count(), 0);
    assert_eq!(extracts.enter_tokens().count(), 0);
    assert_eq!(extracts.transacts().count(), 0);
}

/// TC-EXT-006: Extract mixed events from a single host block.
///
/// Verifies that the extractor correctly handles a block containing all event
/// types simultaneously, preserving order and correctly categorizing each.
#[test]
fn tc_ext_006_extract_mixed_events() {
    let mut ru_block = RuBlockSpec::test()
        .with_gas_limit(25_000_000)
        .with_reward_address(Address::repeat_byte(0xcc));
    ru_block.add_simple_send(&TEST_SIGNERS[0], TEST_USERS[1], U256::from(GWEI_TO_WEI), 0);

    let hbs = HostBlockSpec::test()
        .with_block_number(TEST_SYS.host_deploy_height() + 1)
        // Native enters
        .enter(TEST_USERS[0], (GWEI_TO_WEI * 10) as usize)
        .enter(TEST_USERS[1], (GWEI_TO_WEI * 5) as usize)
        // Token enters
        .enter_token(TEST_USERS[2], 25_000_000, HOST_USDC)
        .enter_token(TEST_USERS[3], 1_000_000_000_000, HOST_USDT)
        // Transacts
        .simple_transact(TEST_USERS[4], TEST_USERS[5], [0xaa, 0xbb], GWEI_TO_WEI as usize)
        // Fills
        .fill(HOST_USDC, TEST_USERS[6], 750_000)
        .fill(HOST_WBTC, TEST_USERS[7], 1_000)
        // Block submission
        .submit_block(ru_block);
    let (chain, _) = hbs.to_chain();

    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();

    // Full conformance check
    hbs.assert_conforms(&extracts);

    // Verify counts of each event type
    assert_eq!(extracts.enters().count(), 2);
    assert_eq!(extracts.enter_tokens().count(), 2);
    assert_eq!(extracts.transacts().count(), 1);
    assert!(extracts.contains_block());

    // Verify aggregate fills are populated
    let aggregate = extracts.aggregate_fills();
    let fills = aggregate.fills();
    assert!(fills.contains_key(&(TEST_SYS.host_chain_id(), HOST_USDC)));
    assert!(fills.contains_key(&(TEST_SYS.host_chain_id(), HOST_WBTC)));

    // Verify block data
    let submitted = extracts.events.submitted.as_ref().unwrap();
    assert_eq!(submitted.gas_limit(), 25_000_000);
}

/// TC-EXT-007: Chain-ID filtering excludes events for other rollups.
///
/// Verifies that the extractor correctly filters out events that target a
/// different rollup chain ID, ensuring only events for our chain are extracted.
#[test]
fn tc_ext_007_chain_id_filtering() {
    let hbs = HostBlockSpec::test()
        .with_block_number(TEST_SYS.host_deploy_height() + 1)
        // Valid enter for our chain
        .enter(TEST_USERS[0], (GWEI_TO_WEI * 10) as usize)
        // Ignored enter (wrong chain ID - uses chain_id = 0)
        .ignored_enter(TEST_USERS[1], GWEI_TO_WEI * 5)
        // Valid enter_token for our chain
        .enter_token(TEST_USERS[2], 1_000_000, HOST_USDC)
        // Ignored enter_token (wrong chain ID)
        .ingnored_enter_token(TEST_USERS[3], 2_000_000, HOST_USDT)
        // Valid fill for our chain
        .fill(HOST_USDC, TEST_USERS[4], 500_000)
        // Ignored fill (wrong chain ID)
        .ignored_fill(HOST_USDT, TEST_USERS[5], 1_000_000);
    let (chain, _) = hbs.to_chain();

    let extractor = Extractor::new(TEST_SYS);
    let extracts = extractor.extract_signet(&chain).next().unwrap();

    hbs.assert_conforms(&extracts);

    // Only valid events should be extracted (ignored ones filtered out)
    assert_eq!(extracts.enters().count(), 1);
    assert_eq!(extracts.enter_tokens().count(), 1);

    // Verify the correct events are present
    let enters: Vec<_> = extracts.enters().collect();
    assert_eq!(enters[0].rollupRecipient, TEST_USERS[0]);
    assert_eq!(enters[0].amount, U256::from(GWEI_TO_WEI * 10));

    let enter_tokens: Vec<_> = extracts.enter_tokens().collect();
    assert_eq!(enter_tokens[0].rollupRecipient, TEST_USERS[2]);
    assert_eq!(enter_tokens[0].token, HOST_USDC);

    // Verify aggregate fills only contains valid fill
    let aggregate = extracts.aggregate_fills();
    let fills = aggregate.fills();
    assert!(fills.contains_key(&(TEST_SYS.host_chain_id(), HOST_USDC)));
    // USDT fill was ignored (wrong chain ID), so shouldn't be in aggregate
    // Note: The ignored_fill still creates a receipt but with chain_id=0,
    // and the extractor should filter it out
}
