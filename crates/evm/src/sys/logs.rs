use alloy::{consensus::TxReceipt, primitives::Log, sol_types::SolEvent};
use signet_extract::ExtractedEvent;
use signet_zenith::{Transactor, MINTER_ADDRESS};

alloy::sol! {
    event MintNative(
        bytes32 indexed txHash,
        uint64 indexed logIndex,
        address indexed recipient,
        uint256 amount,
    );

    event MintToken(
        bytes32 indexed txHash,
        uint64 indexed logIndex,
        address indexed recipient,
        address hostToken,
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

impl From<MintNative> for Log {
    fn from(value: MintNative) -> Self {
        Log { address: MINTER_ADDRESS, data: value.encode_log_data() }
    }
}

impl From<MintToken> for Log {
    fn from(value: MintToken) -> Self {
        Log { address: MINTER_ADDRESS, data: value.encode_log_data() }
    }
}

impl From<Transact> for Log {
    fn from(value: Transact) -> Self {
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
