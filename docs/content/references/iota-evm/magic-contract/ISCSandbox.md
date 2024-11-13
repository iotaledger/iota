---
title: ISCSandbox
description: 'This is the main interface of the ISC Magic Contract.'
---

## ISCSandbox

_This is the main interface of the ISC Magic Contract._

### getRequestID

```solidity
function getRequestID() external view returns (struct ISCRequestID)
```

### getSenderAccount

```solidity
function getSenderAccount() external view returns (struct ISCAgentID)
```

### triggerEvent

```solidity
function triggerEvent(string s) external
```

### getEntropy

```solidity
function getEntropy() external view returns (bytes32)
```

### allow

```solidity
function allow(address target, struct ISCAssets allowance) external
```

### takeAllowedFunds

```solidity
function takeAllowedFunds(address addr, struct ISCAssets allowance) external
```

### getAllowanceFrom

```solidity
function getAllowanceFrom(address addr) external view returns (struct ISCAssets)
```

### getAllowanceTo

```solidity
function getAllowanceTo(address target) external view returns (struct ISCAssets)
```

### getAllowance

```solidity
function getAllowance(address from, address to) external view returns (struct ISCAssets)
```

### send

```solidity
function send(struct L1Address targetAddress, struct ISCAssets assets, bool adjustMinimumStorageDeposit, struct ISCSendMetadata metadata, struct ISCSendOptions sendOptions) external payable
```

### call

```solidity
function call(ISCHname contractHname, ISCHname entryPoint, struct ISCDict params, struct ISCAssets allowance) external returns (struct ISCDict)
```

### callView

```solidity
function callView(ISCHname contractHname, ISCHname entryPoint, struct ISCDict params) external view returns (struct ISCDict)
```

### getChainID

```solidity
function getChainID() external view returns (ISCChainID)
```

### getChainOwnerID

```solidity
function getChainOwnerID() external view returns (struct ISCAgentID)
```

### getTimestampUnixSeconds

```solidity
function getTimestampUnixSeconds() external view returns (int64)
```

### getBaseTokenProperties

```solidity
function getBaseTokenProperties() external view returns (struct ISCTokenProperties)
```

### getNativeTokenID

```solidity
function getNativeTokenID(uint32 foundrySN) external view returns (struct NativeTokenID)
```

### getNativeTokenScheme

```solidity
function getNativeTokenScheme(uint32 foundrySN) external view returns (struct NativeTokenScheme)
```

### getNFTData

```solidity
function getNFTData(NFTID id) external view returns (struct ISCNFT)
```

### getIRC27NFTData

```solidity
function getIRC27NFTData(NFTID id) external view returns (struct IRC27NFT)
```

### erc20NativeTokensAddress

```solidity
function erc20NativeTokensAddress(uint32 foundrySN) external view returns (address)
```

### erc721NFTCollectionAddress

```solidity
function erc721NFTCollectionAddress(NFTID collectionID) external view returns (address)
```

### erc20NativeTokensFoundrySerialNumber

```solidity
function erc20NativeTokensFoundrySerialNumber(address addr) external view returns (uint32)
```

### registerERC20NativeToken

```solidity
function registerERC20NativeToken(uint32 foundrySN, string name, string symbol, uint8 decimals, struct ISCAssets allowance) external
```

## __iscSandbox

```solidity
contract ISCSandbox __iscSandbox
```

