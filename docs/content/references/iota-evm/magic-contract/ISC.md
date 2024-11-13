---
title: ISC
description: 'This library contains various interfaces and functions related to the IOTA Smart Contracts (ISC) system.
 It provides access to the ISCSandbox, ISCAccounts, ISCUtil, ERC20BaseTokens, ERC20NativeTokens, ERC721NFTs, and ERC721NFTCollection contracts.'
---

## ISC

_This library contains various interfaces and functions related to the IOTA Smart Contracts (ISC) system.
It provides access to the ISCSandbox, ISCAccounts, ISCUtil, ERC20BaseTokens, ERC20NativeTokens, ERC721NFTs, and ERC721NFTCollection contracts._

### sandbox

```solidity
contract ISCSandbox sandbox
```

### accounts

```solidity
contract ISCAccounts accounts
```

### util

```solidity
contract ISCUtil util
```

### baseTokens

```solidity
contract ERC20BaseTokens baseTokens
```

### nativeTokens

```solidity
function nativeTokens(uint32 foundrySN) internal view returns (contract ERC20NativeTokens)
```

### nfts

```solidity
contract ERC721NFTs nfts
```

### erc721NFTCollection

```solidity
function erc721NFTCollection(NFTID collectionID) internal view returns (contract ERC721NFTCollection)
```

