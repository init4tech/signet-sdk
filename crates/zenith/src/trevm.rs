use crate::{Passage::EnterToken, Transactor, Zenith};
use alloy::{
    primitives::{Address, U256},
    rlp::BufMut,
    rpc::types::AccessList,
    sol_types::SolCall,
};
use trevm::{
    journal::{JournalDecode, JournalDecodeError, JournalEncode},
    revm::context::{TransactTo, TxEnv},
    Tx,
};

const ZENITH_HEADER_BYTES: usize = 32 + 32 + 32 + 20 + 32;

impl Tx for Transactor::Transact {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        // destructuring here means that any changes to the fields will result
        // in breaking changes here, ensuring that they never silently add new
        // fields
        let TxEnv {
            tx_type,
            caller,
            gas_limit,
            gas_price,
            kind,
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
        *tx_type = 5; // Custom
        *caller = self.sender;
        *gas_limit = self.gas.as_limbs()[0];
        *gas_price = self.maxFeePerGas.saturating_to();
        *gas_priority_fee = Some(0);
        *kind = TransactTo::Call(self.to);
        *value = self.value;
        *data = self.data.clone();
        *chain_id = Some(self.rollup_chain_id());
        *nonce = 0;
        *access_list = Default::default();
        blob_hashes.clear();
        *max_fee_per_blob_gas = 0;
        authorization_list.clear();
    }
}

impl Tx for EnterToken {
    fn fill_tx_env(&self, tx_env: &mut TxEnv) {
        let TxEnv {
            tx_type,
            caller,
            gas_limit,
            gas_price,
            kind,
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

        *tx_type = 5; // Custom
        *caller = crate::MINTER_ADDRESS;
        *gas_limit = 1_000_000;
        *gas_price = 0;
        // This is deliberately not set, as it is not known by the event.
        *kind = Address::ZERO.into();
        *value = U256::ZERO;
        *data =
            crate::mintCall { amount: self.amount(), to: self.rollupRecipient }.abi_encode().into();
        *nonce = 0;
        *chain_id = Some(self.rollup_chain_id());
        *access_list = AccessList::default();
        *gas_priority_fee = Some(0);
        blob_hashes.clear();
        *max_fee_per_blob_gas = 0;
        authorization_list.clear();
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
