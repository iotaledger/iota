// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title IBridgeConfig
/// @dev Interface for the BridgeConfig contract.
interface IBridgeConfig {
    /* ========== STRUCTS ========== */

    /// @notice The data struct for the supported bridge tokens.
    struct Token {
        address tokenAddress;
        uint8 iotaDecimal;
        bool native;
    }

    /* ========== VIEW FUNCTIONS ========== */

    /// @notice Returns the address of the token with the given ID.
    /// @param tokenID The ID of the token.
    /// @return address of the provided token.
    function tokenAddressOf(uint8 tokenID) external view returns (address);

    /// @notice Returns the iota decimal places of the token with the given ID.
    /// @param tokenID The ID of the token.
    /// @return amount of iota decimal places of the provided token.
    function tokenIotaDecimalOf(uint8 tokenID) external view returns (uint8);

    /// @notice Returns the price of the token with the given ID.
    /// @param tokenID The ID of the token.
    /// @return price of the provided token.
    function tokenPriceOf(uint8 tokenID) external view returns (uint64);

    /// @notice Returns the supported status of the token with the given ID.
    /// @param tokenID The ID of the token.
    /// @return true if the token is supported, false otherwise.
    function isTokenSupported(uint8 tokenID) external view returns (bool);

    /// @notice Returns whether a chain is supported in IotaBridge with the given ID.
    /// @param chainId The ID of the chain.
    /// @return true if the chain is supported, false otherwise.
    function isChainSupported(uint8 chainId) external view returns (bool);

    /// @notice Returns the chain ID of the bridge.
    function chainID() external view returns (uint8);

    event TokenAdded(uint8 tokenID, address tokenAddress, uint8 iotaDecimal, uint64 tokenPrice);
    event TokenPriceUpdated(uint8 tokenID, uint64 tokenPrice);
}
