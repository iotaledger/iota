// SPDX-License-Identifier: MIT

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/extensions/IERC20Metadata.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "../interfaces/IBridgeConfig.sol";

/// @title BridgeConfig
/// @notice This contract manages a registry of supported tokens and supported chain IDs for the IotaBridge.
/// It also provides functions to convert token amounts to Iota decimal adjusted amounts and vice versa.
contract BridgeConfig is IBridgeConfig {
    /* ========== STATE VARIABLES ========== */

    uint8 public chainID;
    mapping(uint8 tokenID => Token) public supportedTokens;
    mapping(uint8 chainId => bool isSupported) public supportedChains;

    /// @notice Constructor function for the BridgeConfig contract.
    /// @dev the provided arrays must have the same length.
    /// @param _chainID The ID of the chain this contract is deployed on.
    /// @param _supportedTokens The addresses of the supported tokens.
    /// @param _supportedChains array of supported chain IDs.
    constructor(
        uint8 _chainID,
        address[] memory _supportedTokens,
        uint8[] memory _supportedChains
    ) {
        require(_supportedTokens.length == 4, "BridgeConfig: Invalid supported token addresses");

        uint8[] memory _iotaDecimals = new uint8[](5);
        _iotaDecimals[0] = 9; // IOTA
        _iotaDecimals[1] = 8; // wBTC
        _iotaDecimals[2] = 8; // wETH
        _iotaDecimals[3] = 6; // USDC
        _iotaDecimals[4] = 6; // USDT

        // Add IOTA as the first supported token
        supportedTokens[0] = Token(address(0), _iotaDecimals[0]);

        for (uint8 i; i < _supportedTokens.length; i++) {
            supportedTokens[i + 1] = Token(_supportedTokens[i], _iotaDecimals[i + 1]);
        }

        for (uint8 i; i < _supportedChains.length; i++) {
            require(_supportedChains[i] != _chainID, "BridgeConfig: Cannot support self");
            supportedChains[_supportedChains[i]] = true;
        }

        chainID = _chainID;
    }

    /* ========== VIEW FUNCTIONS ========== */

    /// @notice Returns the address of the token with the given ID.
    /// @param tokenID The ID of the token.
    /// @return address of the provided token.
    function getTokenAddress(uint8 tokenID) public view override returns (address) {
        return supportedTokens[tokenID].tokenAddress;
    }

    /// @notice Returns the iota decimal places of the token with the given ID.
    /// @param tokenID The ID of the token.
    /// @return amount of iota decimal places of the provided token.
    function getIotaDecimal(uint8 tokenID) public view override returns (uint8) {
        return supportedTokens[tokenID].iotaDecimal;
    }

    /// @notice Returns whether a token is supported in IotaBridge with the given ID.
    /// @param tokenID The ID of the token.
    /// @return true if the token is supported, false otherwise.
    function isTokenSupported(uint8 tokenID) public view override returns (bool) {
        return supportedTokens[tokenID].tokenAddress != address(0);
    }

    /// @notice Returns whether a chain is supported in IotaBridge with the given ID.
    /// @param chainId The ID of the chain.
    /// @return true if the chain is supported, false otherwise.
    function isChainSupported(uint8 chainId) public view override returns (bool) {
        return supportedChains[chainId];
    }

    /// @notice Converts the provided token amount to the Iota decimal adjusted amount.
    /// @param tokenID The ID of the token to convert.
    /// @param amount The ERC20 amount of the tokens to convert to Iota.
    /// @return Iota converted amount.
    function convertERC20ToIotaDecimal(uint8 tokenID, uint256 amount)
        public
        view
        override
        returns (uint64)
    {
        uint8 ethDecimal = IERC20Metadata(getTokenAddress(tokenID)).decimals();
        uint8 iotaDecimal = getIotaDecimal(tokenID);

        if (ethDecimal == iotaDecimal) {
            // Ensure converted amount fits within uint64
            require(amount <= type(uint64).max, "BridgeConfig: Amount too large for uint64");
            return uint64(amount);
        }

        require(ethDecimal > iotaDecimal, "BridgeConfig: Invalid Iota decimal");

        // Difference in decimal places
        uint256 factor = 10 ** (ethDecimal - iotaDecimal);
        amount = amount / factor;

        // Ensure the converted amount fits within uint64
        require(amount <= type(uint64).max, "BridgeConfig: Amount too large for uint64");

        return uint64(amount);
    }

    /// @notice Converts the provided Iota decimal adjusted amount to the ERC20 token amount.
    /// @param tokenID The ID of the token to convert.
    /// @param amount The Iota amount of the tokens to convert to ERC20.
    /// @return ERC20 converted amount.
    function convertIotaToERC20Decimal(uint8 tokenID, uint64 amount)
        public
        view
        override
        returns (uint256)
    {
        uint8 ethDecimal = IERC20Metadata(getTokenAddress(tokenID)).decimals();
        uint8 iotaDecimal = getIotaDecimal(tokenID);

        if (iotaDecimal == ethDecimal) {
            return uint256(amount);
        }

        require(ethDecimal > iotaDecimal, "BridgeConfig: Invalid Iota decimal");

        // Difference in decimal places
        uint256 factor = 10 ** (ethDecimal - iotaDecimal);
        return uint256(amount * factor);
    }

    /* ========== MODIFIERS ========== */

    /// @notice Requires the given token to be supported.
    /// @param tokenID The ID of the token to check.
    modifier tokenSupported(uint8 tokenID) {
        require(isTokenSupported(tokenID), "BridgeConfig: Unsupported token");
        _;
    }
}
