use crate::NATIVE_TOKEN_ADDRESS;
use alloy::primitives::Address;
use core::fmt;
use serde::ser::SerializeSeq;
use std::{borrow::Cow, mem::MaybeUninit};

/// Permissioned tokens for the host chain.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HostPermitted {
    /// USDC
    Usdc,
    /// USDT
    Usdt,
    /// WBTC
    Wbtc,
    /// ETH or WETH
    Weth,
}

/// Permissioned tokens for the rollup.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RollupPermitted {
    /// USD (Native Asset)
    Usd,
    /// WBTC
    Wbtc,
    /// WETH
    Weth,
}

impl From<HostPermitted> for RollupPermitted {
    fn from(value: HostPermitted) -> Self {
        match value {
            HostPermitted::Usdc => RollupPermitted::Usd,
            HostPermitted::Usdt => RollupPermitted::Usd,
            HostPermitted::Wbtc => RollupPermitted::Wbtc,
            HostPermitted::Weth => RollupPermitted::Weth,
        }
    }
}

/// A host-chain USD record, which contains the address of the token and its
/// decimals./s
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize, Hash)]
pub struct HostUsdRecord {
    /// Host chain address of the USD token.
    address: Address,
    /// Name of the USD token.
    ticker: Cow<'static, str>,
    /// Decimals of the USD token.
    decimals: u8,
}

impl HostUsdRecord {
    /// Create a new host USD record.
    pub const fn new(address: Address, ticker: Cow<'static, str>, decimals: u8) -> Self {
        Self { address, ticker, decimals }
    }

    /// Get the address of the USD token.
    pub const fn address(&self) -> Address {
        self.address
    }

    /// Get the ticker of the USD token.
    pub fn ticker(&self) -> &str {
        &self.ticker
    }

    /// Get the decimals of the USD token.
    pub const fn decimals(&self) -> u8 {
        self.decimals
    }
}

/// Host tokens configuration for the rollup.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct HostTokens {
    usds: UsdRecords,
    wbtc: Address,
    weth: Address,
}

impl HostTokens {
    /// Instantiate a new host tokens configuration.
    pub const fn new(usds: UsdRecords, wbtc: Address, weth: Address) -> Self {
        Self { usds, wbtc, weth }
    }

    /// Get the USD record for the given address, if it is a USD token.
    pub fn usd_record(&self, address: Address) -> Option<&HostUsdRecord> {
        self.usds.iter().find(|record| record.address == address)
    }

    /// Check if the address is an approved USD token.
    pub fn is_usd(&self, address: Address) -> bool {
        self.usd_record(address).is_some()
    }

    /// Check if the address is WETH or ETH.
    pub fn is_eth(&self, address: Address) -> bool {
        address == self.weth || address == NATIVE_TOKEN_ADDRESS
    }

    /// Get the decimals for the given address, if the address is a USD token.
    pub fn decimals_for(&self, address: Address) -> Option<u8> {
        self.usd_record(address).map(|r| r.decimals)
    }

    /// Returns true if the token is a permitted host token.
    pub fn is_host_token(&self, address: Address) -> bool {
        address == self.wbtc || self.is_eth(address) || self.usd_record(address).is_some()
    }

    /// Get the [`HostPermitted`] for the given address, if it is a
    /// permitted token.
    pub fn token_for(&self, address: Address) -> Option<HostPermitted> {
        if address == self.wbtc {
            Some(HostPermitted::Wbtc)
        } else if address == self.weth || address == NATIVE_TOKEN_ADDRESS {
            Some(HostPermitted::Weth)
        } else if let Some(record) = self.usd_record(address) {
            match record.ticker.as_ref() {
                "USDC" => Some(HostPermitted::Usdc),
                "USDT" => Some(HostPermitted::Usdt),
                _ => None,
            }
        } else {
            None
        }
    }

    /// Check if the address is a permitted token.
    pub fn is_token(&self, address: Address) -> bool {
        self.token_for(address).is_some()
    }

    /// Get the address for the given token.
    pub fn address_for(&self, token: HostPermitted) -> Address {
        match token {
            HostPermitted::Usdc => self
                .usds
                .iter()
                .find_map(
                    |record| if record.ticker == "USDC" { Some(record.address) } else { None },
                )
                .expect("must contain USDC"),
            HostPermitted::Usdt => self
                .usds
                .iter()
                .find_map(
                    |record| if record.ticker == "USDT" { Some(record.address) } else { None },
                )
                .expect("must contain USDT"),
            HostPermitted::Wbtc => self.wbtc,
            HostPermitted::Weth => self.weth,
        }
    }

    /// Get the host address for USDC.
    pub fn usdc(&self) -> Address {
        self.address_for(HostPermitted::Usdc)
    }

    /// Get the host address for USDT.
    pub fn usdt(&self) -> Address {
        self.address_for(HostPermitted::Usdt)
    }

    /// Get the host address for WBTC.
    pub fn wbtc(&self) -> Address {
        self.address_for(HostPermitted::Wbtc)
    }

    /// Get the host address for WETH.
    pub fn weth(&self) -> Address {
        self.address_for(HostPermitted::Weth)
    }
}

/// Rollup tokens configuration for the rollup.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct RollupTokens {
    wbtc: Address,
    weth: Address,
}

impl RollupTokens {
    /// Instantiate a new rollup tokens configuration.
    pub const fn new(wbtc: Address, weth: Address) -> Self {
        Self { wbtc, weth }
    }

    /// Get the address of the WBTC token on the rollup chain.
    pub const fn wbtc(&self) -> Address {
        self.wbtc
    }

    /// Get the address of the WETH token on the rollup chain.
    pub const fn weth(&self) -> Address {
        self.weth
    }

    /// Get the [`RollupPermitted`] for the given address, if it is a
    /// permitted token.
    pub const fn token_for(&self, address: Address) -> Option<RollupPermitted> {
        if address.const_eq(&self.wbtc) {
            Some(RollupPermitted::Wbtc)
        } else if address.const_eq(&self.weth) || address.const_eq(&NATIVE_TOKEN_ADDRESS) {
            Some(RollupPermitted::Weth)
        } else {
            None
        }
    }

    /// Check if the provided address is a predeployed token contract on the
    /// rollup.
    pub const fn is_token(&self, address: Address) -> bool {
        self.token_for(address).is_some()
    }

    /// Get the address for the given token.
    pub const fn address_for(&self, token: RollupPermitted) -> Address {
        match token {
            RollupPermitted::Wbtc => self.wbtc,
            RollupPermitted::Weth => self.weth,
            RollupPermitted::Usd => NATIVE_TOKEN_ADDRESS,
        }
    }
}

const USD_RECORD_CAPACITY: usize = 5;

/// A set of USD records, which can hold up to 5 records. 5 is chosen as a
/// reasonable limit for the number of USD tokens that can be predeployed on
/// the host chain. This limit is arbitrary but should be sufficient for most
/// use cases, and can be adjusted trivially if needed.
pub struct UsdRecords {
    len: usize,
    data: [MaybeUninit<HostUsdRecord>; USD_RECORD_CAPACITY],
}

impl fmt::Debug for UsdRecords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UsdRecords")
            .field("len", &self.len)
            .field("data", &self.as_slice())
            .finish()
    }
}

impl Clone for UsdRecords {
    fn clone(&self) -> Self {
        let mut data = [const { MaybeUninit::uninit() }; USD_RECORD_CAPACITY];
        for (idx, item) in self.iter().enumerate() {
            data[idx].write(item.clone());
        }

        Self { len: self.len, data }
    }
}

impl PartialEq for UsdRecords {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for UsdRecords {}

impl Default for UsdRecords {
    fn default() -> Self {
        Self::new()
    }
}

impl UsdRecords {
    /// Create a new empty set of USD records.
    pub const fn new() -> Self {
        Self { len: 0, data: [const { MaybeUninit::uninit() }; USD_RECORD_CAPACITY] }
    }

    /// Get the number of USD records in the set.
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Get the maximum capacity of the set.
    pub const fn capacity(&self) -> usize {
        USD_RECORD_CAPACITY
    }

    /// Check if the set is empty.
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Push a new USD record into the set, if there is space. Panic otherwise
    pub const fn push(&mut self, record: HostUsdRecord) {
        if self.len < self.capacity() {
            self.data[self.len].write(record);
            self.len += 1;
        } else {
            panic!("Cannot push more than 5 USD records");
        }
    }

    /// Get the record at the specified index, if it exists.
    pub const fn get(&self, index: usize) -> Option<HostUsdRecord> {
        if index < self.len {
            // SAFETY: We ensure that the data is initialized when pushing records.
            Some(unsafe { self.data[index].assume_init_read() })
        } else {
            None
        }
    }

    /// Get a slice of the USD records.
    pub const fn as_slice(&self) -> &[HostUsdRecord] {
        // SAFETY: We ensure that the data is initialized when pushing records.
        unsafe { std::slice::from_raw_parts(self.data.as_ptr() as *const HostUsdRecord, self.len) }
    }

    /// Get an iterator over the USD records.
    pub fn iter(&self) -> impl Iterator<Item = &HostUsdRecord> {
        self.data[..self.len].iter().map(|init| {
            // SAFETY: We ensure that the data is initialized when pushing records.
            unsafe { init.assume_init_ref() }
        })
    }
}


impl serde::Serialize for UsdRecords {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.len))?;

        for element in self.iter() {
            seq.serialize_element(element)?;
        }

        seq.end()
    }
}

impl<'de> serde::Deserialize<'de> for UsdRecords {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vec: Vec<HostUsdRecord> = Vec::deserialize(deserializer)?;
        if vec.len() > 5 {
            return Err(serde::de::Error::custom("Too many USD records"));
        }
        let mut records = UsdRecords::new();
        for record in vec {
            records.push(record);
        }
        Ok(records)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn usd_records_serde() {
        let json = r#"[
            {"address": "0x0000000000000000000000000000000000000001", "ticker": "USDC", "decimals": 6},
            {"address": "0x0000000000000000000000000000000000000002", "ticker": "USDT", "decimals": 12},
            {"address": "0x0000000000000000000000000000000000000003", "ticker": "DAI", "decimals": 18}
        ]"#;

        let mut records = UsdRecords::new();
        records.push(HostUsdRecord {
            address: Address::with_last_byte(0x01),
            ticker: "USDC".into(),
            decimals: 6,
        });
        records.push(HostUsdRecord {
            address: Address::with_last_byte(0x02),
            ticker: "USDT".into(),
            decimals: 12,
        });
        records.push(HostUsdRecord {
            address: Address::with_last_byte(0x03),
            ticker: "DAI".into(),
            decimals: 18,
        });

        let records: UsdRecords = serde_json::from_str(json).unwrap();
        let serialized = serde_json::to_string(&records).unwrap();
        let deserialized: UsdRecords = serde_json::from_str(&serialized).unwrap();

        assert_eq!(records, deserialized);
        assert_eq!(records.len(), 3);
        assert_eq!(deserialized.len(), 3);
        assert!(records.iter().any(|r| r.ticker == "USDC"));
        assert!(records.iter().any(|r| r.ticker == "USDT"));
        assert!(records.iter().any(|r| r.ticker == "DAI"));
    }
}
