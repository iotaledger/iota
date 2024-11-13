---
title: ISCPrivileged
description: 'The ISCPrivileged interface represents a contract that has some extra methods not included in the standard ISC interface.
 These methods can only be called from privileged contracts.'
---

## ISCPrivileged

_The ISCPrivileged interface represents a contract that has some extra methods not included in the standard ISC interface.
These methods can only be called from privileged contracts._

### moveBetweenAccounts

```solidity
function moveBetweenAccounts(address sender, address receiver, struct ISCAssets allowance) external
```

### setAllowanceBaseTokens

```solidity
function setAllowanceBaseTokens(address from, address to, uint256 numTokens) external
```

### setAllowanceNativeTokens

```solidity
function setAllowanceNativeTokens(address from, address to, struct NativeTokenID nativeTokenID, uint256 numTokens) external
```

### moveAllowedFunds

```solidity
function moveAllowedFunds(address from, address to, struct ISCAssets allowance) external
```

## __iscPrivileged

```solidity
contract ISCPrivileged __iscPrivileged
```

