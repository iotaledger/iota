---
title: ISCTypes
---

## ISC_MAGIC_ADDRESS

```solidity
address ISC_MAGIC_ADDRESS
```

## ISC_ERC20BASETOKENS_ADDRESS

```solidity
address ISC_ERC20BASETOKENS_ADDRESS
```

## ISC_ERC721_ADDRESS

```solidity
address ISC_ERC721_ADDRESS
```

## L1Address

```solidity
struct L1Address {
  bytes data;
}
```

## L1AddressTypeEd25519

```solidity
uint8 L1AddressTypeEd25519
```

## L1AddressTypeAlias

```solidity
uint8 L1AddressTypeAlias
```

## L1AddressTypeNFT

```solidity
uint8 L1AddressTypeNFT
```

## NativeTokenID

```solidity
struct NativeTokenID {
  bytes data;
}
```

## NativeToken

```solidity
struct NativeToken {
  struct NativeTokenID ID;
  uint256 amount;
}
```

## NativeTokenScheme

```solidity
struct NativeTokenScheme {
  uint256 mintedTokens;
  uint256 meltedTokens;
  uint256 maximumSupply;
}
```

## NFTID

## ISCNFT

```solidity
struct ISCNFT {
  NFTID ID;
  struct L1Address issuer;
  bytes metadata;
  struct ISCAgentID owner;
}
```

## IRC27NFTMetadata

```solidity
struct IRC27NFTMetadata {
  string standard;
  string version;
  string mimeType;
  string uri;
  string name;
}
```

## IRC27NFT

```solidity
struct IRC27NFT {
  struct ISCNFT nft;
  struct IRC27NFTMetadata metadata;
}
```

## ISCTransactionID

## ISCHname

## ISCChainID

## ISCAgentID

```solidity
struct ISCAgentID {
  bytes data;
}
```

## ISCAgentIDKindNil

```solidity
uint8 ISCAgentIDKindNil
```

## ISCAgentIDKindAddress

```solidity
uint8 ISCAgentIDKindAddress
```

## ISCAgentIDKindContract

```solidity
uint8 ISCAgentIDKindContract
```

## ISCAgentIDKindEthereumAddress

```solidity
uint8 ISCAgentIDKindEthereumAddress
```

## ISCRequestID

```solidity
struct ISCRequestID {
  bytes data;
}
```

## ISCDictItem

```solidity
struct ISCDictItem {
  bytes key;
  bytes value;
}
```

## ISCDict

```solidity
struct ISCDict {
  struct ISCDictItem[] items;
}
```

## ISCSendMetadata

```solidity
struct ISCSendMetadata {
  ISCHname targetContract;
  ISCHname entrypoint;
  struct ISCDict params;
  struct ISCAssets allowance;
  uint64 gasBudget;
}
```

## ISCAssets

```solidity
struct ISCAssets {
  uint64 baseTokens;
  struct NativeToken[] nativeTokens;
  NFTID[] nfts;
}
```

## ISCSendOptions

```solidity
struct ISCSendOptions {
  int64 timelock;
  struct ISCExpiration expiration;
}
```

## ISCExpiration

```solidity
struct ISCExpiration {
  int64 time;
  struct L1Address returnAddress;
}
```

## ISCTokenProperties

```solidity
struct ISCTokenProperties {
  string name;
  string tickerSymbol;
  uint8 decimals;
  uint256 totalSupply;
}
```

## ISCTypes

### L1AddressType

```solidity
function L1AddressType(struct L1Address addr) internal pure returns (uint8)
```

### newEthereumAgentID

```solidity
function newEthereumAgentID(address addr, ISCChainID iscChainID) internal pure returns (struct ISCAgentID)
```

### isEthereum

```solidity
function isEthereum(struct ISCAgentID a) internal pure returns (bool)
```

### ethAddress

```solidity
function ethAddress(struct ISCAgentID a) internal pure returns (address)
```

### chainID

```solidity
function chainID(struct ISCAgentID a) internal pure returns (ISCChainID)
```

### asNFTID

```solidity
function asNFTID(uint256 tokenID) internal pure returns (NFTID)
```

### isInCollection

```solidity
function isInCollection(struct ISCNFT nft, NFTID collectionId) internal pure returns (bool)
```

