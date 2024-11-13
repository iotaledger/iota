---
title: ERC721NFTCollection
description: 'The ERC721 contract for a L2 collection of ISC NFTs, as defined in IRC27.
 Implements the ERC721 standard and extends the ERC721NFTs contract.
 For more information about IRC27, refer to: https://github.com/iotaledger/tips/blob/main/tips/TIP-0027/tip-0027.md'
---

## ERC721NFTCollection

_The ERC721 contract for a L2 collection of ISC NFTs, as defined in IRC27.
Implements the ERC721 standard and extends the ERC721NFTs contract.
For more information about IRC27, refer to: https://github.com/iotaledger/tips/blob/main/tips/TIP-0027/tip-0027.md_

### _balanceOf

```solidity
function _balanceOf(struct ISCAgentID owner) internal view virtual returns (uint256)
```

_Returns the balance of the specified owner._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| owner | struct ISCAgentID | The address to query the balance of. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The balance of the specified owner. |

### _isManagedByThisContract

```solidity
function _isManagedByThisContract(struct ISCNFT nft) internal view virtual returns (bool)
```

_Checks if the given NFT is managed by this contract._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| nft | struct ISCNFT | The NFT to check. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | True if the NFT is managed by this contract, false otherwise. |

### collectionId

```solidity
function collectionId() external view virtual returns (NFTID)
```

_Returns the ID of the collection._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | NFTID | The ID of the collection. |

### name

```solidity
function name() external view virtual returns (string)
```

_Returns the name of the collection._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | string | The name of the collection. |

