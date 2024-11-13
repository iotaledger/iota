---
title: ERC721NFTs
description: 'This contract represents the ERC721 contract for the &quot;global&quot; collection of native NFTs on the chains L1 account.'
---

## ERC721NFTs

_This contract represents the ERC721 contract for the "global" collection of native NFTs on the chains L1 account._

### Transfer

```solidity
event Transfer(address from, address to, uint256 tokenId)
```

_Emitted when a token is transferred from one address to another._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| from | address | The address transferring the token. |
| to | address | The address receiving the token. |
| tokenId | uint256 | The ID of the token being transferred. |

### Approval

```solidity
event Approval(address owner, address approved, uint256 tokenId)
```

_Emitted when the approval of a token is changed or reaffirmed._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| owner | address | The owner of the token. |
| approved | address | The new approved address. |
| tokenId | uint256 | The ID of the token. |

### ApprovalForAll

```solidity
event ApprovalForAll(address owner, address operator, bool approved)
```

_Emitted when operator gets the allowance from owner._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| owner | address | The owner of the token. |
| operator | address | The operator to get the approval. |
| approved | bool | True if the operator got approval, false if not. |

### _balanceOf

```solidity
function _balanceOf(struct ISCAgentID owner) internal view virtual returns (uint256)
```

### _isManagedByThisContract

```solidity
function _isManagedByThisContract(struct ISCNFT) internal view virtual returns (bool)
```

### balanceOf

```solidity
function balanceOf(address owner) public view returns (uint256)
```

_Returns the number of tokens owned by a specific address._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| owner | address | The address to query the balance of. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The balance of the specified address. |

### ownerOf

```solidity
function ownerOf(uint256 tokenId) public view returns (address)
```

_Returns the owner of the specified token ID._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| tokenId | uint256 | The ID of the token to query the owner for. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | address | The address of the owner of the token. |

### safeTransferFrom

```solidity
function safeTransferFrom(address from, address to, uint256 tokenId, bytes data) public payable
```

_Safely transfers an ERC721 token from one address to another.

Emits a `Transfer` event.

Requirements:
- `from` cannot be the zero address.
- `to` cannot be the zero address.
- The token must exist and be owned by `from`.
- If `to` is a smart contract, it must implement the `onERC721Received` function and return the magic value._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| from | address | The address to transfer the token from. |
| to | address | The address to transfer the token to. |
| tokenId | uint256 | The ID of the token to be transferred. |
| data | bytes | Additional data with no specified format, to be passed to the `onERC721Received` function if `to` is a smart contract. |

### safeTransferFrom

```solidity
function safeTransferFrom(address from, address to, uint256 tokenId) public payable
```

_Safely transfers an ERC721 token from one address to another.

Emits a `Transfer` event.

Requirements:
- `from` cannot be the zero address.
- `to` cannot be the zero address.
- The caller must own the token or be approved for it._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| from | address | The address to transfer the token from. |
| to | address | The address to transfer the token to. |
| tokenId | uint256 | The ID of the token to be transferred. |

### transferFrom

```solidity
function transferFrom(address from, address to, uint256 tokenId) public payable
```

_Transfers an ERC721 token from one address to another.
Emits a {Transfer} event.

Requirements:
- The caller must be approved or the owner of the token._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| from | address | The address to transfer the token from. |
| to | address | The address to transfer the token to. |
| tokenId | uint256 | The ID of the token to be transferred. |

### approve

```solidity
function approve(address approved, uint256 tokenId) public payable
```

Only the owner of the token or an approved operator can call this function.

_Approves another address to transfer the ownership of a specific token._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| approved | address | The address to be approved for token transfer. |
| tokenId | uint256 | The ID of the token to be approved for transfer. |

### setApprovalForAll

```solidity
function setApprovalForAll(address operator, bool approved) public
```

_Sets or revokes approval for the given operator to manage all of the caller's tokens._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| operator | address | The address of the operator to set approval for. |
| approved | bool | A boolean indicating whether to approve or revoke the operator's approval. |

### getApproved

```solidity
function getApproved(uint256 tokenId) public view returns (address)
```

_Returns the address that has been approved to transfer the ownership of the specified token._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| tokenId | uint256 | The ID of the token. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | address | The address approved to transfer the ownership of the token. |

### isApprovedForAll

```solidity
function isApprovedForAll(address owner, address operator) public view returns (bool)
```

_Checks if an operator is approved to manage all of the owner's tokens._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| owner | address | The address of the token owner. |
| operator | address | The address of the operator. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | A boolean value indicating whether the operator is approved for all tokens of the owner. |

### _isApprovedOrOwner

```solidity
function _isApprovedOrOwner(address spender, uint256 tokenId) internal view returns (bool)
```

### _transferFrom

```solidity
function _transferFrom(address from, address to, uint256 tokenId) internal
```

### supportsInterface

```solidity
function supportsInterface(bytes4 interfaceID) public pure returns (bool)
```

_Checks if a contract supports a given interface._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| interfaceID | bytes4 | The interface identifier. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | A boolean value indicating whether the contract supports the interface. |

### _checkOnERC721Received

```solidity
function _checkOnERC721Received(address from, address to, uint256 tokenId, bytes data) internal returns (bool)
```

### _isContract

```solidity
function _isContract(address account) internal view returns (bool)
```

### name

```solidity
function name() external view virtual returns (string)
```

### symbol

```solidity
function symbol() external pure returns (string)
```

### tokenURI

```solidity
function tokenURI(uint256 tokenId) external view returns (string)
```

## __erc721NFTs

```solidity
contract ERC721NFTs __erc721NFTs
```

## IERC721Receiver

### onERC721Received

```solidity
function onERC721Received(address _operator, address _from, uint256 _tokenId, bytes _data) external returns (bytes4)
```

