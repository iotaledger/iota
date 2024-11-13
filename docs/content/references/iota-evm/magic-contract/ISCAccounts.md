---
title: ISCAccounts
description: 'Functions of the ISC Magic Contract to access the core accounts functionality'
---

## ISCAccounts

_Functions of the ISC Magic Contract to access the core accounts functionality_

### getL2BalanceBaseTokens

```solidity
function getL2BalanceBaseTokens(struct ISCAgentID agentID) external view returns (uint64)
```

### getL2BalanceNativeTokens

```solidity
function getL2BalanceNativeTokens(struct NativeTokenID id, struct ISCAgentID agentID) external view returns (uint256)
```

### getL2NFTs

```solidity
function getL2NFTs(struct ISCAgentID agentID) external view returns (NFTID[])
```

### getL2NFTAmount

```solidity
function getL2NFTAmount(struct ISCAgentID agentID) external view returns (uint256)
```

### getL2NFTsInCollection

```solidity
function getL2NFTsInCollection(struct ISCAgentID agentID, NFTID collectionId) external view returns (NFTID[])
```

### getL2NFTAmountInCollection

```solidity
function getL2NFTAmountInCollection(struct ISCAgentID agentID, NFTID collectionId) external view returns (uint256)
```

### foundryCreateNew

```solidity
function foundryCreateNew(struct NativeTokenScheme tokenScheme, struct ISCAssets allowance) external returns (uint32)
```

### createNativeTokenFoundry

```solidity
function createNativeTokenFoundry(string tokenName, string tokenSymbol, uint8 tokenDecimals, struct NativeTokenScheme tokenScheme, struct ISCAssets allowance) external returns (uint32)
```

### mintNativeTokens

```solidity
function mintNativeTokens(uint32 foundrySN, uint256 amount, struct ISCAssets allowance) external
```

## __iscAccounts

```solidity
contract ISCAccounts __iscAccounts
```

