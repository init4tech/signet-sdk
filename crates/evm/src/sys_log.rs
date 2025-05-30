use alloy::{consensus::TxReceipt, primitives::Log, sol_types::SolEvent};
use signet_extract::ExtractedEvent;
use signet_zenith::{Passage, Transactor, MINTER_ADDRESS};

alloy::sol! {
    event Enter(
        bytes32 indexed txHash,
        uint64 indexed logIndex,
        address indexed recipient,
        uint256 amount,
    );

    event EnterToken(
        bytes32 indexed txHash,
        uint64 indexed logIndex,
        address indexed recipient,
        address token,
        uint256 amount,
    );

    event Transact(
        bytes32 indexed txHash,
        uint64 indexed logIndex,
        address indexed sender,
        uint256 value,
        uint256 gas,
        uint256 maxFeePerGas,
    );
}

impl<R: TxReceipt<Log = Log>> From<&ExtractedEvent<'_, R, Passage::Enter>> for Enter {
    fn from(event: &ExtractedEvent<'_, R, Passage::Enter>) -> Self {
        Enter {
            recipient: event.event.rollupRecipient,
            txHash: event.tx_hash(),
            logIndex: event.log_index as u64,
            amount: event.amount(),
        }
    }
}

impl From<Enter> for Log {
    fn from(value: Enter) -> Self {
        Log { address: MINTER_ADDRESS, data: value.encode_log_data() }
    }
}

impl<R: TxReceipt<Log = Log>> From<&ExtractedEvent<'_, R, Passage::EnterToken>> for EnterToken {
    fn from(event: &ExtractedEvent<'_, R, Passage::EnterToken>) -> Self {
        EnterToken {
            recipient: event.event.rollupRecipient,
            txHash: event.tx_hash(),
            logIndex: event.log_index as u64,
            token: event.token(),
            amount: event.amount(),
        }
    }
}

impl From<EnterToken> for Log {
    fn from(value: EnterToken) -> Self {
        Log { address: MINTER_ADDRESS, data: value.encode_log_data() }
    }
}

impl<R: TxReceipt<Log = Log>> From<&ExtractedEvent<'_, R, Transactor::Transact>> for Transact {
    fn from(event: &ExtractedEvent<'_, R, Transactor::Transact>) -> Self {
        Transact {
            sender: event.event.sender,
            txHash: event.tx_hash(),
            logIndex: event.log_index as u64,
            value: event.value(),
            gas: event.event.gas,
            maxFeePerGas: event.event.maxFeePerGas,
        }
    }
}

impl From<Transact> for Log {
    fn from(value: Transact) -> Self {
        Log { address: MINTER_ADDRESS, data: value.encode_log_data() }
    }
}
