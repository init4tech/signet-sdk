use alloy::{
    primitives::{b256, bytes, Address, Bytes, B256, U256},
    providers::Provider,
};

alloy::sol! {
    #[sol(rpc)]
    #[derive(Debug)]
    contract Counter {
        uint256 public count;
        event Count(uint256 indexed count);
        function increment() public;

   }
}

/// The storage slot where the counter value is stored in the Counter contract.
pub const COUNTER_SLOT: U256 = U256::ZERO;

/// A test address for the Counter.sol contract, which will be pre-deployed in
/// test EVMs.
pub const COUNTER_TEST_ADDRESS: Address = Address::repeat_byte(0x49);

/// Calculated bytecode hash for the Counter.sol contract. Keccak256[`COUNTER_BYTECODE`].
pub const COUNTER_BYTECODE_HASH: B256 =
    b256!("0x905f7a46c9105d8c0d5d6368b601da50e09cdd1fffa5ed6b6548ed6a15bf3a6b");

/// Deploycode for the Counter.sol contract. Sending a transaction with this
/// code will deploy the contract, and the account will then contain
/// [`COUNTER_BYTECODE`].
///
/// Generated from:
/// solc --optimize --via-ir --bin crates/test-utils/tests/artifacts/Counter.sol
pub const COUNTER_DEPLOY_CODE: Bytes = bytes!(
    "608080604052346016575f805560d79081601b8239f35b5f80fdfe60808060405260043610156011575f80fd5b5f3560e01c90816306661abd14608a575063d09de08a14602f575f80fd5b346086575f3660031901126086575f5460018101809111607257805f557face32e4392fafee7f8245a5ae6a32722dc74442d018c52e460835648cbeeeba15f80a2005b634e487b7160e01b5f52601160045260245ffd5b5f80fd5b346086575f3660031901126086576020905f548152f3fea264697066735822122027a064635c397ba96bfe6d499e93133378e80d56d94510a0ffa4a51969bcf09464736f6c634300081a0033"
);

/// Post-deployment bytecode for the Counter.sol contract.
pub const COUNTER_BYTECODE: Bytes = bytes!(
    "60808060405260043610156011575f80fd5b5f3560e01c90816306661abd14608a575063d09de08a14602f575f80fd5b346086575f3660031901126086575f5460018101809111607257805f557face32e4392fafee7f8245a5ae6a32722dc74442d018c52e460835648cbeeeba15f80a2005b634e487b7160e01b5f52601160045260245ffd5b5f80fd5b346086575f3660031901126086576020905f548152f3fea264697066735822122027a064635c397ba96bfe6d499e93133378e80d56d94510a0ffa4a51969bcf09464736f6c634300081a0033"
);

/// Get an instance of the pre-deployed Counter contract.
pub fn counter<P: Provider>(p: P) -> Counter::CounterInstance<P> {
    Counter::CounterInstance::new(COUNTER_TEST_ADDRESS, p)
}
