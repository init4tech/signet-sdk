use alloy::{
    consensus::constants::GWEI_TO_WEI,
    primitives::{Address, U256},
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
        .with_block_number(1)
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
