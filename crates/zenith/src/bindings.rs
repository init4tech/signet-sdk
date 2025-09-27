#![allow(clippy::too_many_arguments)]
#![allow(missing_docs)]
use alloy::primitives::{Address, Bytes, FixedBytes, U256};
use std::borrow::Cow;

mod mint {
    alloy::sol!(
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        function mint(address to, uint256 amount);
    );
}
pub use mint::mintCall;

mod zenith {
    use super::*;

    alloy::sol!(
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[sol(rpc)]
        Zenith,
        "abi/Zenith.json"
    );

    impl Copy for Zenith::BlockHeader {}
    impl Copy for Zenith::BlockSubmitted {}
    impl Copy for Zenith::SequencerSet {}
    impl Copy for Zenith::BadSignature {}
    impl Copy for Zenith::OneRollupBlockPerHostBlock {}
    impl Copy for Zenith::OnlySequencerAdmin {}
    impl Copy for Zenith::IncorrectHostBlock {}

    impl Zenith::BlockSubmitted {
        /// Get the sequencer address that signed the block.
        pub const fn sequencer(&self) -> Address {
            self.sequencer
        }

        /// Get the chain id of the rollup.
        pub const fn rollup_chain_id(&self) -> u64 {
            self.rollupChainId.as_limbs()[0]
        }

        /// Get the gas limit of the block
        pub const fn gas_limit(&self) -> u64 {
            self.gasLimit.as_limbs()[0]
        }

        /// Get the reward address of the block.
        pub const fn reward_address(&self) -> Address {
            self.rewardAddress
        }

        /// Get the block data hash, i.e. the committment to the data of the block.
        pub const fn block_data_hash(&self) -> FixedBytes<32> {
            self.blockDataHash
        }

        /// Convert the BlockSubmitted event to a BlockHeader with the given host
        /// block number.
        pub const fn to_header(self, host_block_number: U256) -> Zenith::BlockHeader {
            Zenith::BlockHeader::from_block_submitted(self, host_block_number)
        }
    }

    impl Zenith::BlockHeader {
        /// Create a BlockHeader from a BlockSubmitted event with the given host
        /// block number
        pub const fn from_block_submitted(
            host_block_submitted: Zenith::BlockSubmitted,
            host_block_number: U256,
        ) -> Zenith::BlockHeader {
            Zenith::BlockHeader {
                rollupChainId: host_block_submitted.rollupChainId,
                hostBlockNumber: host_block_number,
                gasLimit: host_block_submitted.gasLimit,
                rewardAddress: host_block_submitted.rewardAddress,
                blockDataHash: host_block_submitted.blockDataHash,
            }
        }

        /// Get the host block number of the block
        pub const fn host_block_number(&self) -> u64 {
            self.hostBlockNumber.as_limbs()[0]
        }

        /// Get the chain ID of the block (discarding high bytes).
        pub const fn chain_id(&self) -> u64 {
            self.rollupChainId.as_limbs()[0]
        }

        /// Get the gas limit of the block (discarding high bytes).
        pub const fn gas_limit(&self) -> u64 {
            self.gasLimit.as_limbs()[0]
        }

        /// Get the reward address of the block.
        pub const fn reward_address(&self) -> Address {
            self.rewardAddress
        }

        /// Get the block data hash, i.e. the committment to the data of the block.
        pub const fn block_data_hash(&self) -> FixedBytes<32> {
            self.blockDataHash
        }
    }
}

mod passage {
    use super::*;

    alloy::sol!(
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[sol(rpc)]
        Passage,
        "abi/Passage.json"
    );

    impl Copy for Passage::EnterConfigured {}
    impl Copy for Passage::Withdrawal {}
    impl Copy for Passage::OnlyTokenAdmin {}
    impl Copy for Passage::Enter {}
    impl Copy for Passage::EnterToken {}
    impl Copy for Passage::DisallowedEnter {}
    impl Copy for Passage::FailedCall {}
    impl Copy for Passage::InsufficientBalance {}
    impl Copy for Passage::SafeERC20FailedOperation {}
    impl Copy for Passage::AddressEmptyCode {}

    impl Copy for Passage::PassageEvents {}

    impl Passage::EnterToken {
        /// Get the chain ID of the event (discarding high bytes), returns `None`
        /// if the event has no associated chain id.
        pub const fn rollup_chain_id(&self) -> u64 {
            self.rollupChainId.as_limbs()[0]
        }

        /// Get the token address of the event.
        pub const fn token(&self) -> Address {
            self.token
        }

        /// Get the recipient of the event.
        pub const fn recipient(&self) -> Address {
            self.rollupRecipient
        }

        /// Get the amount of the event.
        pub const fn amount(&self) -> U256 {
            self.amount
        }
    }

    impl Passage::Enter {
        /// Get the chain ID of the event (discarding high bytes), returns `None`
        /// if the event has no associated chain id.
        pub const fn rollup_chain_id(&self) -> u64 {
            self.rollupChainId.as_limbs()[0]
        }

        /// Get the recipient of the event.
        pub const fn recipient(&self) -> Address {
            self.rollupRecipient
        }

        /// Get the amount of the event.
        pub const fn amount(&self) -> U256 {
            self.amount
        }
    }

    impl Passage::Withdrawal {
        /// Get the token address of the request.
        pub const fn token(&self) -> Address {
            self.token
        }

        /// Get the recipient of the request.
        pub const fn recipient(&self) -> Address {
            self.recipient
        }

        /// Get the amount of the request.
        pub const fn amount(&self) -> U256 {
            self.amount
        }
    }

    impl Passage::EnterConfigured {
        /// Get the token address of the event.
        pub const fn token(&self) -> Address {
            self.token
        }

        /// Get if the token has been configured to allow or disallow enters.
        pub const fn can_enter(&self) -> bool {
            self.canEnter
        }
    }
}

mod orders {
    use super::*;
    use IOrders::Output;
    use ISignatureTransfer::TokenPermissions;

    alloy::sol!(
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[sol(rpc)]
        Orders,
        "abi/RollupOrders.json"
    );

    alloy::sol! {
       struct PermitBatchWitnessTransferFrom {
           TokenPermissions[] permitted;
           address spender;
           uint256 nonce;
           uint256 deadline;
           Output[] outputs;
       }
    }

    impl Copy for IOrders::Input {}
    impl Copy for IOrders::Output {}
    impl Copy for Orders::Sweep {}
    impl Copy for Orders::InsufficientBalance {}
    impl Copy for Orders::AddressEmptyCode {}
    impl Copy for Orders::LengthMismatch {}
    impl Copy for Orders::OrderExpired {}
    impl Copy for Orders::OutputMismatch {}
    impl Copy for Orders::SafeERC20FailedOperation {}

    impl IOrders::Input {
        pub const fn token(&self) -> Address {
            self.token
        }

        pub const fn amount(&self) -> u64 {
            self.amount.as_limbs()[0]
        }
    }

    impl IOrders::Output {
        pub const fn token(&self) -> Address {
            self.token
        }

        pub const fn amount(&self) -> u64 {
            self.amount.as_limbs()[0]
        }

        pub const fn recipient(&self) -> Address {
            self.recipient
        }

        pub const fn chain_id(&self) -> u32 {
            self.chainId
        }
    }

    impl From<&IOrders::Input> for TokenPermissions {
        fn from(input: &IOrders::Input) -> TokenPermissions {
            TokenPermissions { token: input.token, amount: input.amount }
        }
    }

    impl From<IOrders::Input> for TokenPermissions {
        fn from(input: IOrders::Input) -> TokenPermissions {
            TokenPermissions { token: input.token, amount: input.amount }
        }
    }

    impl From<TokenPermissions> for IOrders::Input {
        fn from(perm: TokenPermissions) -> IOrders::Input {
            IOrders::Input { token: perm.token, amount: perm.amount }
        }
    }

    impl From<&IOrders::Output> for TokenPermissions {
        fn from(output: &IOrders::Output) -> TokenPermissions {
            TokenPermissions { token: output.token, amount: output.amount }
        }
    }

    impl From<IOrders::Output> for TokenPermissions {
        fn from(output: IOrders::Output) -> TokenPermissions {
            TokenPermissions { token: output.token, amount: output.amount }
        }
    }

    impl Orders::Order {
        /// Get the inputs of the order.
        #[allow(clippy::missing_const_for_fn)] // false positive
        pub fn inputs(&self) -> &[IOrders::Input] {
            &self.inputs
        }

        /// Get the outputs of the order.
        #[allow(clippy::missing_const_for_fn)] // false positive
        pub fn outputs(&self) -> &[IOrders::Output] {
            &self.outputs
        }

        /// Get the deadline of the order.
        pub const fn deadline(&self) -> u64 {
            self.deadline.as_limbs()[0]
        }
    }

    impl<'a> From<&'a Orders::Order> for Cow<'a, Orders::Order> {
        fn from(order: &'a Orders::Order) -> Self {
            Cow::Borrowed(order)
        }
    }

    impl Orders::Sweep {
        pub const fn recipient(&self) -> Address {
            self.recipient
        }

        pub const fn token(&self) -> Address {
            self.token
        }

        pub const fn amount(&self) -> u64 {
            self.amount.as_limbs()[0]
        }
    }

    impl Orders::Filled {
        pub fn outputs(&self) -> &[IOrders::Output] {
            self.outputs.as_slice()
        }
    }
}

mod transactor {
    use super::*;

    alloy::sol!(
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[sol(rpc)]
        Transactor,
        "abi/Transactor.json"
    );

    impl Copy for Transactor::GasConfigured {}

    impl Transactor::Transact {
        pub const fn rollup_chain_id(&self) -> u64 {
            self.rollupChainId.as_limbs()[0]
        }

        pub const fn sender(&self) -> Address {
            self.sender
        }

        pub const fn to(&self) -> Address {
            self.to
        }

        pub const fn data(&self) -> &Bytes {
            &self.data
        }

        pub const fn value(&self) -> U256 {
            self.value
        }

        pub fn max_fee_per_gas(&self) -> u128 {
            self.maxFeePerGas.to::<u128>()
        }

        pub fn gas(&self) -> u128 {
            self.gas.to::<u128>()
        }
    }
}

mod rollup_passage {

    alloy::sol!(
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[sol(rpc)]
        RollupPassage,
        "abi/RollupPassage.json"
    );

    impl Copy for RollupPassage::Exit {}
    impl Copy for RollupPassage::ExitToken {}
    impl Copy for RollupPassage::AddressEmptyCode {}
    impl Copy for RollupPassage::InsufficientBalance {}
    impl Copy for RollupPassage::SafeERC20FailedOperation {}

    impl Copy for RollupPassage::RollupPassageEvents {}
}

mod bundle_helper {
    use super::*;

    use ISignatureTransfer::{PermitBatchTransferFrom, TokenPermissions};
    use UsesPermit2::Permit2Batch;

    alloy::sol!(
        #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[sol(rpc)]
        BundleHelper,
        "abi/BundleHelper.json"
    );

    impl From<&RollupOrders::Output> for IOrders::Output {
        fn from(output: &RollupOrders::Output) -> IOrders::Output {
            IOrders::Output {
                token: output.token,
                amount: output.amount,
                recipient: output.recipient,
                chainId: output.chainId,
            }
        }
    }

    impl From<RollupOrders::Output> for IOrders::Output {
        fn from(output: RollupOrders::Output) -> IOrders::Output {
            IOrders::Output {
                token: output.token,
                amount: output.amount,
                recipient: output.recipient,
                chainId: output.chainId,
            }
        }
    }

    impl From<RollupOrders::Permit2Batch> for Permit2Batch {
        fn from(permit: HostOrders::Permit2Batch) -> Permit2Batch {
            Permit2Batch {
                permit: permit.permit.into(),
                owner: permit.owner,
                signature: permit.signature,
            }
        }
    }

    impl From<&RollupOrders::Permit2Batch> for Permit2Batch {
        fn from(permit: &HostOrders::Permit2Batch) -> Permit2Batch {
            Permit2Batch {
                permit: (&permit.permit).into(),
                owner: permit.owner,
                signature: permit.signature.clone(),
            }
        }
    }

    impl From<&RollupOrders::PermitBatchTransferFrom> for PermitBatchTransferFrom {
        fn from(permit: &HostOrders::PermitBatchTransferFrom) -> PermitBatchTransferFrom {
            PermitBatchTransferFrom {
                permitted: permit.permitted.iter().map(TokenPermissions::from).collect(),
                nonce: permit.nonce,
                deadline: permit.deadline,
            }
        }
    }

    impl From<RollupOrders::PermitBatchTransferFrom> for PermitBatchTransferFrom {
        fn from(permit: HostOrders::PermitBatchTransferFrom) -> PermitBatchTransferFrom {
            PermitBatchTransferFrom {
                permitted: permit.permitted.into_iter().map(TokenPermissions::from).collect(),
                nonce: permit.nonce,
                deadline: permit.deadline,
            }
        }
    }

    impl From<&crate::bindings::orders::ISignatureTransfer::TokenPermissions> for TokenPermissions {
        fn from(perm: &HostOrders::TokenPermissions) -> TokenPermissions {
            TokenPermissions { token: perm.token, amount: perm.amount }
        }
    }

    impl From<crate::bindings::orders::ISignatureTransfer::TokenPermissions> for TokenPermissions {
        fn from(perm: HostOrders::TokenPermissions) -> TokenPermissions {
            TokenPermissions { token: perm.token, amount: perm.amount }
        }
    }
}

pub use zenith::Zenith;

/// Contract Bindings for the RollupOrders contract.
#[allow(non_snake_case)]
pub mod RollupOrders {
    pub use super::orders::IOrders::*;
    pub use super::orders::ISignatureTransfer::*;
    pub use super::orders::Orders::*;
    pub use super::orders::PermitBatchWitnessTransferFrom;
    pub use super::orders::UsesPermit2::*;

    pub use super::orders::Orders::OrdersCalls as RollupOrdersCalls;
    pub use super::orders::Orders::OrdersErrors as RollupOrdersErrors;
    pub use super::orders::Orders::OrdersEvents as RollupOrdersEvents;
    pub use super::orders::Orders::OrdersInstance as RollupOrdersInstance;
}

/// Contract Bindings for the HostOrders contract.
#[allow(non_snake_case)]
pub mod HostOrders {
    pub use super::orders::Orders::*;

    pub use super::orders::IOrders::*;
    pub use super::orders::ISignatureTransfer::*;
    pub use super::orders::UsesPermit2::*;

    pub use super::orders::Orders::OrdersCalls as HostOrdersCalls;
    pub use super::orders::Orders::OrdersErrors as HostOrdersErrors;
    pub use super::orders::Orders::OrdersEvents as HostOrdersEvents;
    pub use super::orders::Orders::OrdersInstance as HostOrdersInstance;
}

/// Contract Bindings for the Passage contract.
#[allow(non_snake_case)]
pub mod Passage {
    pub use super::passage::Passage::*;

    pub use super::passage::ISignatureTransfer::*;
    pub use super::passage::UsesPermit2::*;
}

pub use transactor::Transactor;

/// Contract Bindings for the RollupPassage contract.
#[allow(non_snake_case)]
pub mod RollupPassage {
    pub use super::rollup_passage::RollupPassage::*;

    pub use super::rollup_passage::ISignatureTransfer::*;
    pub use super::rollup_passage::UsesPermit2::*;
}

/// Contract Bindings for the BundleHelper contract.
#[allow(non_snake_case)]
pub mod BundleHelper {
    pub use super::bundle_helper::BundleHelper::*;
    pub use super::bundle_helper::IOrders;
    pub use super::bundle_helper::Zenith::BlockHeader;
}
