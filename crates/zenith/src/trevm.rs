use crate::{Passage::EnterToken, Transactor, Zenith, ZenithCallBundle};
use alloy::{
    primitives::{Address, U256},
    rlp::BufMut,
    sol_types::SolCall,
};
use trevm::{
    journal::{JournalDecode, JournalDecodeError, JournalEncode},
    revm::primitives::{BlockEnv, TransactTo, TxEnv},
    Block, Tx,
};

const ZENITH_HEADER_BYTES: usize = 32 + 32 + 32 + 20 + 32;

impl Tx for Transactor::Transact {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        // destructuring here means that any changes to the fields will result
        // in breaking changes here, ensuring that they never silently add new
        // fields
        let TxEnv {
            caller,
            gas_limit,
            gas_price,
            transact_to,
            value,
            data,
            nonce,
            chain_id,
            access_list,
            gas_priority_fee,
            blob_hashes,
            max_fee_per_blob_gas,
            authorization_list,
        } = tx_env;

        *caller = self.sender;
        *gas_limit = self.gas.as_limbs()[0];
        *gas_price = self.maxFeePerGas;
        *gas_priority_fee = Some(U256::ZERO);
        *transact_to = TransactTo::Call(self.to);
        *value = self.value;
        *data = self.data.clone();
        *chain_id = Some(self.rollup_chain_id());
        // This causes nonce validation to be skipped. i.e. the Transact event
        // will always use the next available nonce
        *nonce = None;
        *access_list = vec![];
        blob_hashes.clear();
        max_fee_per_blob_gas.take();
        authorization_list.take();
    }
}

impl Tx for EnterToken {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        let TxEnv {
            caller,
            gas_limit,
            gas_price,
            transact_to,
            value,
            data,
            nonce,
            chain_id,
            access_list,
            gas_priority_fee,
            blob_hashes,
            max_fee_per_blob_gas,
            authorization_list,
        } = tx_env;

        *caller = crate::MINTER_ADDRESS;
        *gas_limit = 1_000_000;
        *gas_price = U256::ZERO;
        // This is deliberately not set, as it is not known by the event.
        *transact_to = Address::ZERO.into();
        *value = U256::ZERO;
        *data =
            crate::mintCall { amount: self.amount(), to: self.rollupRecipient }.abi_encode().into();
        *nonce = None;
        *chain_id = Some(self.rollup_chain_id());
        *access_list = vec![];
        *gas_priority_fee = Some(U256::ZERO);
        blob_hashes.clear();
        max_fee_per_blob_gas.take();
        authorization_list.take();
    }
}

impl Block for ZenithCallBundle {
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

impl JournalEncode for Zenith::BlockHeader {
    fn serialized_size(&self) -> usize {
        ZENITH_HEADER_BYTES
    }

    fn encode(&self, buf: &mut dyn BufMut) {
        let Self { rollupChainId, hostBlockNumber, gasLimit, rewardAddress, blockDataHash } = self;

        rollupChainId.encode(buf);
        hostBlockNumber.encode(buf);
        gasLimit.encode(buf);
        rewardAddress.encode(buf);
        blockDataHash.encode(buf);
    }
}

impl JournalDecode for Zenith::BlockHeader {
    fn decode(buf: &mut &[u8]) -> Result<Self, JournalDecodeError> {
        Ok(Self {
            rollupChainId: JournalDecode::decode(buf)?,
            hostBlockNumber: JournalDecode::decode(buf)?,
            gasLimit: JournalDecode::decode(buf)?,
            rewardAddress: JournalDecode::decode(buf)?,
            blockDataHash: JournalDecode::decode(buf)?,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use alloy::primitives::B256;

    fn roundtrip<T: JournalDecode + JournalEncode + PartialEq>(expected: &T) {
        let enc = JournalEncode::encoded(expected);
        assert_eq!(enc.len(), expected.serialized_size(), "{}", core::any::type_name::<T>());
        let dec = T::decode(&mut enc.as_slice()).expect("decoding failed");
        assert_eq!(&dec, expected);
    }

    #[test]
    fn journal() {
        roundtrip(&Zenith::BlockHeader {
            rollupChainId: U256::from(1),
            hostBlockNumber: U256::from(1),
            gasLimit: U256::from(1),
            rewardAddress: Address::repeat_byte(0xa),
            blockDataHash: B256::repeat_byte(0xa),
        });
    }
}
