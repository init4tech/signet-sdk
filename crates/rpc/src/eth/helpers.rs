use alloy::{eips::BlockId, primitives::Bytes};

/// Error output of `eth_call`.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum CallErrorData {
    /// Error output is a byte array, usually a revert message.
    Bytes(Bytes),
    /// Output is a block id.
    BlockId(BlockId),
    /// Error message.
    String(String),
}

impl From<Bytes> for CallErrorData {
    fn from(bytes: Bytes) -> Self {
        Self::Bytes(bytes)
    }
}

impl From<BlockId> for CallErrorData {
    fn from(id: BlockId) -> Self {
        Self::BlockId(id)
    }
}

impl From<String> for CallErrorData {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}
