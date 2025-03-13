use alloy::primitives::{Address, PrimitiveSignature as Signature, B256};
use signet_zenith::MINTER_ADDRESS;

/// A sentinel value to identify the magic signature. This is encoded in the
/// S value, and renders the S value invalid for Ethereum-based chains
/// supporting EIP-2.
///
/// [EIP-2]: https://eips.ethereum.org/EIPS/eip-2
pub(crate) const MAGIC_SIG_SENTINEL: [u8; 4] = [0xff, 0xee, 0xdd, 0xcc];

/// Enum of flags
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub(crate) enum Flags {
    /// Flag used to identify Enters
    Enter = 0x01,
    /// Flag used to identify EnterTokens
    EnterToken = 0x02,
    /// Flag used to identify Transacts
    Transact = 0x03,
}

impl From<Flags> for u8 {
    fn from(flag: Flags) -> u8 {
        flag as u8
    }
}

impl TryFrom<u8> for Flags {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Enter),
            0x02 => Ok(Self::EnterToken),
            0x03 => Ok(Self::Transact),
            _ => Err(()),
        }
    }
}

/// Type flag used to identify the Signet event that cauesd the rollup
/// consensus to create this magic signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum MagicSigInfo {
    /// An enter event.
    Enter,
    /// An enter token event.
    EnterToken,
    /// A transact event.
    Transact {
        /// The address of the sender.
        sender: Address,
    },
}

impl MagicSigInfo {
    /// Get the flag for the magic signature info.
    fn flag(&self) -> u8 {
        match self {
            Self::Enter => Flags::Enter as u8,
            Self::EnterToken => Flags::EnterToken as u8,
            Self::Transact { .. } => Flags::Transact as u8,
        }
    }

    /// Write the magic signature info into a buffer.
    pub fn write_into_s(&self, buf: &mut [u8]) {
        debug_assert_eq!(buf.len(), 32);

        buf[8] = self.flag();
        if let Self::Transact { sender } = self {
            buf[12..32].copy_from_slice(sender.as_slice());
        }
    }

    /// Read the magic signature info from a signature S value.
    pub fn read_from_s(s: impl AsRef<[u8]>) -> Option<Self> {
        let s = s.as_ref();
        if s.len() < 32 {
            return None;
        }
        let flag = s[8].try_into().ok()?;
        match flag {
            Flags::Enter => Some(Self::Enter),
            Flags::EnterToken => Some(Self::EnterToken),
            Flags::Transact => Some(Self::Transact { sender: Address::from_slice(&s[12..]) }),
        }
    }

    /// Get the sender from the magic signature info. For enter and enter token
    /// events, this is the [`MINTER_ADDRESS`]. For transact events, this is the
    /// sender.
    pub const fn sender(&self) -> Address {
        match self {
            Self::Transact { sender } => *sender,
            _ => MINTER_ADDRESS,
        }
    }
}

/// A magic signature, containing information about the host-chain event
/// which caused the rollup transaction to occur.
///
/// This is used to "recover" the sender of a transaction that is not actually
/// signed by the sender. This is used for enter events, enter token events,
/// and transact events.
///
///  Magic signatures are used for Signet system events to allow the system to
/// "recover" the sender of a transaction that is not actually signed by the
/// sender. This is used for enter events, enter token events, and transact
/// events. These signatures contain the sender, a sentinel, and a type flag,
/// and are used by the RPC and other systems to determine the sender of the
/// "transaction".
///
/// The magic signature format is as follows:
/// - odd_y_parity: RESERVED (false)
/// - r: 32-byte txid of the transaction that emitted the event
/// - s:
///   - `[0..4]`: A 4-byte sentinel value (0xffeeddcc).
///   - `[4..8]`: A 4-bytes BE u32 containing the index of the event in the
///     transaction's log array.
///   - `[8..9]`: A 1-byte flag (0x01 for enter, 0x02 for enter token, 0x03
///     for transact).
///   - `[9..12]`: A 3-byte RESERVED region.
///   - `[12..32]`: For transact events, (flag byte 3) the sender's address.
///      RESERVED otherwise.
///
/// Because Ethereum-like chains enforce low-S signatures, the S value of the
/// magic signature is invalid for Ethereum-based chains supporting [EIP-2].
/// This means that the magic signature is never a valid signature for any
/// relevant Ethereum-like chain.
///
/// [EIP-2]: https://eips.ethereum.org/EIPS/eip-2
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MagicSig {
    /// The type of event, with any additional information.
    pub ty: MagicSigInfo,
    /// The transaction ID of the host chain transaction that emitted the event.
    pub txid: B256,
    /// The index of the event in the transaction's log array.
    pub event_idx: usize,
}

impl MagicSig {
    /// Try to [`MagicSig`] from a signature.
    pub fn try_from_signature(sig: &Signature) -> Option<Self> {
        let s = sig.s();
        let s_bytes: [u8; 32] = s.to_be_bytes();
        if !s_bytes.starts_with(&MAGIC_SIG_SENTINEL) {
            return None;
        }

        let ty = MagicSigInfo::read_from_s(s_bytes)?;
        let txid = sig.r().to_le_bytes().into();

        let mut buf = [0u8; 4];
        buf.copy_from_slice(&s_bytes[4..8]);
        let event_idx = u32::from_be_bytes(buf) as usize;

        Some(Self { ty, txid, event_idx })
    }

    /// Create a new [`MagicSig`] for an enter event.
    pub const fn enter(txid: B256, event_idx: usize) -> Self {
        Self { ty: MagicSigInfo::Enter, txid, event_idx }
    }

    /// Create a new [`MagicSig`] for an enter token event.
    pub const fn enter_token(txid: B256, event_idx: usize) -> Self {
        Self { ty: MagicSigInfo::EnterToken, txid, event_idx }
    }

    /// Create a new [`MagicSig`] for a transact event.
    pub const fn transact(txid: B256, event_idx: usize, sender: Address) -> Self {
        Self { ty: MagicSigInfo::Transact { sender }, txid, event_idx }
    }

    /// Get the sender of the magic signature.
    pub const fn sender(&self) -> Address {
        self.ty.sender()
    }
}

impl From<MagicSig> for Signature {
    fn from(value: MagicSig) -> Self {
        let mut buf = [0u8; 64];
        buf[..32].copy_from_slice(value.txid.as_ref());
        buf[32..36].copy_from_slice(&MAGIC_SIG_SENTINEL);
        buf[36..40].copy_from_slice(&(value.event_idx as u32).to_be_bytes());
        value.ty.write_into_s(&mut buf[32..]);

        Signature::from_bytes_and_parity(&buf, false)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_enter_roundtrip() {
        let txid = B256::repeat_byte(0xcc);

        let msig = MagicSig::enter_token(txid, 333);
        let sig: Signature = msig.into();

        assert_eq!(MagicSig::try_from_signature(&sig), Some(msig))
    }

    #[test]
    fn test_enter_token_roundtrip() {
        let txid = B256::repeat_byte(0xcc);

        let msig = MagicSig::enter_token(txid, 3821);
        let sig: Signature = msig.into();

        assert_eq!(MagicSig::try_from_signature(&sig), Some(msig))
    }

    #[test]
    fn test_transact_roundtrip() {
        let txid = B256::repeat_byte(0xcc);
        let sender = Address::repeat_byte(0x12);

        let msig = MagicSig::transact(txid, u32::MAX as usize, sender);
        let sig: Signature = msig.into();

        assert_eq!(MagicSig::try_from_signature(&sig), Some(msig))
    }
}
