---
title: ERC20BaseTokens
description: 'The ERC20 contract directly mapped to the L1 base token.'
---

## ERC20BaseTokens

_The ERC20 contract directly mapped to the L1 base token._

### Approval

```solidity
event Approval(address tokenOwner, address spender, uint256 tokens)
```

_Emitted when the approval of tokens is granted by a token owner to a spender.

This event indicates that the token owner has approved the spender to transfer a certain amount of tokens on their behalf._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| tokenOwner | address | The address of the token owner who granted the approval. |
| spender | address | The address of the spender who is granted the approval. |
| tokens | uint256 | The amount of tokens approved for transfer. |

### Transfer

```solidity
event Transfer(address from, address to, uint256 tokens)
```

_Emitted when tokens are transferred from one address to another.

This event indicates that a certain amount of tokens has been transferred from one address to another._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| from | address | The address from which the tokens are transferred. |
| to | address | The address to which the tokens are transferred. |
| tokens | uint256 | The amount of tokens transferred. |

### name

```solidity
function name() public view returns (string)
```

_Returns the name of the base token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | string | The name of the base token. |

### symbol

```solidity
function symbol() public view returns (string)
```

_Returns the symbol of the base token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | string | The symbol of the base token. |

### decimals

```solidity
function decimals() public view returns (uint8)
```

_Returns the number of decimals used by the base token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint8 | The number of decimals used by the base token. |

### totalSupply

```solidity
function totalSupply() public view returns (uint256)
```

_Returns the total supply of the base token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The total supply of the base token. |

### balanceOf

```solidity
function balanceOf(address tokenOwner) public view returns (uint256)
```

_Returns the balance of the specified token owner._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| tokenOwner | address | The address of the token owner. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The balance of the token owner. |

### transfer

```solidity
function transfer(address receiver, uint256 numTokens) public returns (bool)
```

_Transfers tokens from the caller's account to the specified receiver._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| receiver | address | The address of the receiver. |
| numTokens | uint256 | The number of tokens to transfer. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | true. |

### approve

```solidity
function approve(address delegate, uint256 numTokens) public returns (bool)
```

_Sets the allowance of `delegate` over the caller's tokens._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| delegate | address | The address of the delegate. |
| numTokens | uint256 | The number of tokens to allow. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | true. |

### allowance

```solidity
function allowance(address owner, address delegate) public view returns (uint256)
```

_Returns the allowance of the specified owner for the specified delegate._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| owner | address | The address of the owner. |
| delegate | address | The address of the delegate. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The allowance of the owner for the delegate. |

### transferFrom

```solidity
function transferFrom(address owner, address buyer, uint256 numTokens) public returns (bool)
```

_Transfers tokens from the specified owner's account to the specified buyer._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| owner | address | The address of the owner. |
| buyer | address | The address of the buyer. |
| numTokens | uint256 | The number of tokens to transfer. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | true. |

## __erc20BaseTokens

```solidity
contract ERC20BaseTokens __erc20BaseTokens
```

