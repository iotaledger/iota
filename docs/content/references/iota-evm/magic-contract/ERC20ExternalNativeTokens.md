---
title: ERC20ExternalNativeTokens
description: 'The ERC20 contract for externally registered native tokens (off-chain foundry).'
---

## ERC20ExternalNativeTokens

_The ERC20 contract for externally registered native tokens (off-chain foundry)._

### nativeTokenID

```solidity
function nativeTokenID() public view returns (struct NativeTokenID)
```

_Returns the native token ID._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | struct NativeTokenID | The native token ID. |

### totalSupply

```solidity
function totalSupply() public view returns (uint256)
```

_Returns the total supply of the native tokens._

#### Return Values

| Name | Type | Description |
| ---- | ---- | ----------- |
| [0] | uint256 | The total supply of the native tokens. |

