use alloy::primitives::Address;

/// Rollup pre-deploy tokens.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PermissionedToken {
    /// USDC
    Usdc,
    /// USDT
    Usdt,
    /// WBTC
    Wbtc,
}

/// Rollup configuration pre-deploy tokens.
#[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PredeployTokens {
    /// USDC.
    usdc: Address,
    /// USDT.
    usdt: Address,
    /// WBTC.
    wbtc: Address,
}

impl PredeployTokens {
    /// Create a new pre-deploy tokens configuration.
    pub const fn new(usdc: Address, usdt: Address, wbtc: Address) -> Self {
        Self { usdc, usdt, wbtc }
    }

    /// Get the hard-coded pecorino host tokens.
    pub const fn pecorino_host() -> Self {
        crate::chains::pecorino::HOST_TOKENS
    }

    /// Get the hard-coded local test rollup tokens.
    pub const fn pecorino_rollup() -> Self {
        crate::chains::pecorino::RU_TOKENS
    }

    /// Get the hard-coded local test host tokens.
    #[cfg(any(test, feature = "test-utils"))]
    pub const fn test_host() -> Self {
        crate::chains::test_utils::HOST_TOKENS
    }

    /// Get the hard-coded local test rollup tokens.
    #[cfg(any(test, feature = "test-utils"))]
    pub const fn test_rollup() -> Self {
        crate::chains::test_utils::RU_TOKENS
    }

    /// Get the token for the given address.
    pub const fn const_token_for(&self, address: Address) -> Option<PermissionedToken> {
        if address.const_eq(&self.usdc) {
            Some(PermissionedToken::Usdc)
        } else if address.const_eq(&self.usdt) {
            Some(PermissionedToken::Usdt)
        } else if address.const_eq(&self.wbtc) {
            Some(PermissionedToken::Wbtc)
        } else {
            None
        }
    }

    /// Get the token for the given address.
    pub fn token_for(&self, address: Address) -> Option<PermissionedToken> {
        if address == self.usdc {
            Some(PermissionedToken::Usdc)
        } else if address == self.usdt {
            Some(PermissionedToken::Usdt)
        } else if address == self.wbtc {
            Some(PermissionedToken::Wbtc)
        } else {
            None
        }
    }

    /// True if the address is a token.
    pub const fn const_is_token(&self, address: Address) -> bool {
        self.const_token_for(address).is_some()
    }

    /// True if the address is a token.
    pub fn is_token(&self, address: Address) -> bool {
        self.token_for(address).is_some()
    }

    /// Get the address for the given token.
    pub const fn address_for(&self, token: PermissionedToken) -> Address {
        match token {
            PermissionedToken::Usdc => self.usdc,
            PermissionedToken::Usdt => self.usdt,
            PermissionedToken::Wbtc => self.wbtc,
        }
    }

    /// Get the address of the USDC token.
    pub const fn usdc(&self) -> Address {
        self.usdc
    }

    /// Get the address of the USDT token.
    pub const fn usdt(&self) -> Address {
        self.usdt
    }

    /// Get the address of the WBTC token.
    pub const fn wbtc(&self) -> Address {
        self.wbtc
    }
}
