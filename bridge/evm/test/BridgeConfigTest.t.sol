// SPDX-License-Identifier: MIT

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.20;

import "./mocks/MockTokens.sol";
import "./BridgeBaseTest.t.sol";

contract BridgeConfigTest is BridgeBaseTest {
    function setUp() public {
        setUpBridgeTest();
    }

    function testBridgeConfigInitialization() public {
        assertTrue(config.getTokenAddress(1) == wBTC);
        assertTrue(config.getTokenAddress(2) == wETH);
        assertTrue(config.getTokenAddress(3) == USDC);
        assertTrue(config.getTokenAddress(4) == USDT);
        assertEq(config.getIotaDecimal(0), 9);
        assertEq(config.getIotaDecimal(1), 8);
        assertEq(config.getIotaDecimal(2), 8);
        assertEq(config.getIotaDecimal(3), 6);
        assertEq(config.getIotaDecimal(4), 6);
        assertEq(config.chainID(), chainID);
        assertTrue(config.supportedChains(0));
    }

    function testGetAddress() public {
        assertEq(config.getTokenAddress(1), wBTC);
    }

    function testconvertERC20ToIotaDecimalAmountTooLargeForUint64() public {
        vm.expectRevert(bytes("BridgeConfig: Amount too large for uint64"));
        config.convertERC20ToIotaDecimal(BridgeMessage.ETH, type(uint256).max);
    }

    function testconvertERC20ToIotaDecimalInvalidIotaDecimal() public {
        vm.startPrank(address(bridge));
        address smallUSDC = address(new MockSmallUSDC());
        address[] memory _supportedTokens = new address[](4);
        _supportedTokens[0] = wBTC;
        _supportedTokens[1] = wETH;
        _supportedTokens[2] = smallUSDC;
        _supportedTokens[3] = USDT;
        uint8[] memory _supportedDestinationChains = new uint8[](1);
        _supportedDestinationChains[0] = 0;
        BridgeConfig newBridgeConfig =
            new BridgeConfig(chainID, _supportedTokens, _supportedDestinationChains);
        vm.expectRevert(bytes("BridgeConfig: Invalid Iota decimal"));
        newBridgeConfig.convertERC20ToIotaDecimal(3, 100);
    }

    function testconvertIotaToERC20DecimalInvalidIotaDecimal() public {
        vm.startPrank(address(bridge));
        address smallUSDC = address(new MockSmallUSDC());
        address[] memory _supportedTokens = new address[](4);
        _supportedTokens[0] = wBTC;
        _supportedTokens[1] = wETH;
        _supportedTokens[2] = smallUSDC;
        _supportedTokens[3] = USDT;
        uint8[] memory _supportedDestinationChains = new uint8[](1);
        _supportedDestinationChains[0] = 0;
        BridgeConfig newBridgeConfig =
            new BridgeConfig(chainID, _supportedTokens, _supportedDestinationChains);
        vm.expectRevert(bytes("BridgeConfig: Invalid Iota decimal"));
        newBridgeConfig.convertIotaToERC20Decimal(3, 100);
    }

    function testIsTokenSupported() public {
        assertTrue(config.isTokenSupported(1));
        assertTrue(!config.isTokenSupported(0));
    }

    function testGetIotaDecimal() public {
        assertEq(config.getIotaDecimal(1), 8);
    }

    function testconvertERC20ToIotaDecimal() public {
        // ETH
        assertEq(IERC20Metadata(wETH).decimals(), 18);
        uint256 ethAmount = 10 ether;
        uint64 iotaAmount = config.convertERC20ToIotaDecimal(BridgeMessage.ETH, ethAmount);
        assertEq(iotaAmount, 10_000_000_00); // 10 * 10 ^ 8

        // USDC
        assertEq(IERC20Metadata(USDC).decimals(), 6);
        ethAmount = 50_000_000; // 50 USDC
        iotaAmount = config.convertERC20ToIotaDecimal(BridgeMessage.USDC, ethAmount);
        assertEq(iotaAmount, ethAmount);

        // USDT
        assertEq(IERC20Metadata(USDT).decimals(), 6);
        ethAmount = 60_000_000; // 60 USDT
        iotaAmount = config.convertERC20ToIotaDecimal(BridgeMessage.USDT, ethAmount);
        assertEq(iotaAmount, ethAmount);

        // BTC
        assertEq(IERC20Metadata(wBTC).decimals(), 8);
        ethAmount = 2_00_000_000; // 2 BTC
        iotaAmount = config.convertERC20ToIotaDecimal(BridgeMessage.BTC, ethAmount);
        assertEq(iotaAmount, ethAmount);
    }

    function testconvertIotaToERC20Decimal() public {
        // ETH
        assertEq(IERC20Metadata(wETH).decimals(), 18);
        uint64 iotaAmount = 11_000_000_00; // 11 eth
        uint256 ethAmount = config.convertIotaToERC20Decimal(BridgeMessage.ETH, iotaAmount);
        assertEq(ethAmount, 11 ether);

        // USDC
        assertEq(IERC20Metadata(USDC).decimals(), 6);
        iotaAmount = 50_000_000; // 50 USDC
        ethAmount = config.convertIotaToERC20Decimal(BridgeMessage.USDC, iotaAmount);
        assertEq(iotaAmount, ethAmount);

        // USDT
        assertEq(IERC20Metadata(USDT).decimals(), 6);
        iotaAmount = 50_000_000; // 50 USDT
        ethAmount = config.convertIotaToERC20Decimal(BridgeMessage.USDT, iotaAmount);
        assertEq(iotaAmount, ethAmount);

        // BTC
        assertEq(IERC20Metadata(wBTC).decimals(), 8);
        iotaAmount = 3_000_000_00; // 3 BTC
        ethAmount = config.convertIotaToERC20Decimal(BridgeMessage.BTC, iotaAmount);
        assertEq(iotaAmount, ethAmount);
    }
}
