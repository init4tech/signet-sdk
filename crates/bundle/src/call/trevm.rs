use crate::SignetCallBundle;
use alloy::primitives::U256;
use trevm::{revm::primitives::BlockEnv, Block};

impl Block for SignetCallBundle {
    fn fill_block_env(&self, block_env: &mut BlockEnv) {
        let BlockEnv {
            number,
            coinbase,
            timestamp,
            gas_limit,
            basefee,
            difficulty,
            prevrandao: _,
            blob_excess_gas_and_price: _,
        } = block_env;

        *number = self.bundle.state_block_number.as_number().map(U256::from).unwrap_or(*number);
        *coinbase = self.bundle.coinbase.unwrap_or(*coinbase);
        *timestamp = self.bundle.timestamp.map(U256::from).unwrap_or(*timestamp);
        *gas_limit = self.bundle.gas_limit.map(U256::from).unwrap_or(*gas_limit);
        *difficulty = self.bundle.difficulty.map(U256::from).unwrap_or(*difficulty);
        *basefee = self.bundle.base_fee.map(U256::from).unwrap_or(*basefee);
    }
}
