use crate::SignetCallBundle;
use alloy::primitives::U256;
use trevm::{revm::context::BlockEnv, Block};

impl Block for SignetCallBundle {
    fn fill_block_env(&self, block_env: &mut BlockEnv) {
        let BlockEnv {
            number,
            beneficiary,
            timestamp,
            gas_limit,
            basefee,
            difficulty,
            prevrandao: _,
            blob_excess_gas_and_price: _,
        } = block_env;

        *number = self.bundle.state_block_number.as_number().map(U256::from).unwrap_or(*number);
        *beneficiary = self.bundle.coinbase.unwrap_or(*beneficiary);
        *timestamp = self.bundle.timestamp.map(U256::from).unwrap_or(*timestamp);
        *gas_limit = self.bundle.gas_limit.unwrap_or(*gas_limit);
        *difficulty = self.bundle.difficulty.unwrap_or(*difficulty);
        *basefee =
            self.bundle.base_fee.map(|n| n.try_into().unwrap_or(u64::MAX)).unwrap_or(*basefee);
    }
}
