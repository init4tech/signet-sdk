//! Common contract interfaces used across the Signet SDK.

alloy::sol! {
    /// Minimal ERC20 interface for balance and allowance checks.
    #[sol(rpc)]
    interface IERC20 {
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

alloy::sol! {
    /// Permit2 interface for nonce validation.
    #[sol(rpc)]
    interface IPermit2 {
        function nonceBitmap(address owner, uint256 wordPos) external view returns (uint256);
    }
}
