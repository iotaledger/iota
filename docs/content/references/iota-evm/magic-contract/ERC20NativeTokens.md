---
title: ERC20NativeTokens
description: 'The ERC20 contract native tokens (on-chain foundry).'
---

## ERC20NativeTokens

_The ERC20 contract native tokens (on-chain foundry)._

### Approval

```solidity
event Approval(address tokenOwner, address spender, uint256 tokens)
```

_Emitted when the allowance of a spender for an owner is set._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| tokenOwner | address | The owner of the tokens. |
| spender | address | The address allowed to spend the tokens. |
| tokens | uint256 | The amount of tokens allowed to be spent. |

### Transfer

```solidity
event Transfer(address from, address to, uint256 tokens)
```

_Emitted when tokens are transferred from one address to another._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| from | address | The address tokens are transferred from. |
| to | address | The address tokens are transferred to. |
| tokens | uint256 | The amount of tokens transferred. |

### foundrySerialNumber

```solidity
function foundrySerialNumber() internal view returns (uint32)
```

_Returns the foundry serial number of the native token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint32 | The foundry serial number. |

### nativeTokenID

```solidity
function nativeTokenID() public view virtual returns (struct NativeTokenID)
```

_Returns the native token ID of the native token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | struct NativeTokenID | The native token ID. |

### name

```solidity
function name() public view returns (string)
```

_Returns the name of the native token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | string | The name of the token. |

### symbol

```solidity
function symbol() public view returns (string)
```

_Returns the ticker symbol of the native token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | string | The ticker symbol of the token. |

### decimals

```solidity
function decimals() public view returns (uint8)
```

_Returns the number of decimals used for the native token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint8 | The number of decimals. |

### totalSupply

```solidity
function totalSupply() public view virtual returns (uint256)
```

_Returns the total supply of the native token._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The total supply of the token. |

### balanceOf

```solidity
function balanceOf(address tokenOwner) public view returns (uint256)
```

_Returns the balance of a token owner._

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

_Transfers tokens from the sender's address to the receiver's address._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| receiver | address | The address to transfer tokens to. |
| numTokens | uint256 | The amount of tokens to transfer. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | true. |

### approve

```solidity
function approve(address delegate, uint256 numTokens) public returns (bool)
```

_Sets the allowance of a spender to spend tokens on behalf of the owner._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| delegate | address | The address allowed to spend the tokens. |
| numTokens | uint256 | The amount of tokens allowed to be spent. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | true. |

### allowance

```solidity
function allowance(address owner, address delegate) public view returns (uint256)
```

_Returns the amount of tokens that the spender is allowed to spend on behalf of the owner._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| owner | address | The address of the token owner. |
| delegate | address | The address of the spender. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The amount of tokens the spender is allowed to spend. |

### bytesEqual

```solidity
function bytesEqual(bytes a, bytes b) internal pure returns (bool)
```

_Compares two byte arrays for equality._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| a | bytes | The first byte array. |
| b | bytes | The second byte array. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | A boolean indicating whether the byte arrays are equal or not. |

### transferFrom

```solidity
function transferFrom(address owner, address buyer, uint256 numTokens) public returns (bool)
```

_Transfers tokens from one address to another on behalf of a token owner._

#### Parameters

| Name | Type | Description |
| ---- | ---- | ----------- |
| owner | address | The address from which tokens are transferred. |
| buyer | address | The address to which tokens are transferred. |
| numTokens | uint256 | The amount of tokens to transfer. |

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | bool | A boolean indicating whether the transfer was successful or not. |

