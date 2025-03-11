use crate::SignetCallBundle;
use alloy::primitives::U256;
use trevm::{revm::primitives::BlockEnv, Block};

impl Block for SignetCallBundle {
    fn fill_block_env(&self, block_env: &mut BlockEnv) {
        block_env.number =
            self.bundle.state_block_number.as_number().map(U256::from).unwrap_or(block_env.number);
        block_env.timestamp = self.bundle.timestamp.map(U256::from).unwrap_or(block_env.timestamp);
        block_env.gas_limit = self.bundle.gas_limit.map(U256::from).unwrap_or(block_env.gas_limit);
        block_env.difficulty =
            self.bundle.difficulty.map(U256::from).unwrap_or(block_env.difficulty);
        block_env.basefee = self.bundle.base_fee.map(U256::from).unwrap_or(block_env.basefee);
    }
}
