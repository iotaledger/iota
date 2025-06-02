---
description: How to allow coins and other objects
image: /img/logo/WASP_logo_dark.png
tags:
  - allowance
  - evm
  - magic
  - solidity
  - how-to
teams:
  - iotaledger/l2-smart-contract
---

# Allow

The allowance concept is well known from the EVM contracts like ERC20.
In ISC, we have a similar concept for Move objects that you want to use in the EVM. You might want to use this, for example, to [send coins and other objects to L1](../send-assets-to-l1.mdx) (which includes sending them to other L1 chain accounts).

## Example Code

### 1. Create the `allow` Function

Create a function which allows an address or contract to access a specific ID of your account:

```solidity
function allow(address _address, bytes32 _allowanceObjectID) public {
```

### 2. Create the `ISCAssets` object

Create an `ISCAssets` object to pass as allowance:

```solidity
IotaObjectID[] memory IotaObjectIDs = new IotaObjectID[](1);
IotaObjectIDs[0] = IotaObjectID.wrap(_allowanceIotaObjectID);
ISCAssets memory assets;
assets.objects = IotaObjectIDs;
ISC.sandbox.allow(_address, assets);
```

### 3. Use the Assets as Allowance

With that asset, you can call `allow` to allow address to take our Object:

```solidity
ISC.sandbox.allow(_address, assets);
```

## Full Example Code

```solidity
// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import "@iota/iscmagic/ISC.sol";

contract Allowance {
    function allow(address _address, bytes32 _allowanceIotaObjectID) public {
        IotaObjectID[] memory IotaObjectIDs = new IotaObjectID[](1);
        IotaObjectIDs[0] = IotaObjectID.wrap(_allowanceIotaObjectID);
        ISCAssets memory assets;
        assets.objects = IotaObjectIDs;
        ISC.sandbox.allow(_address, assets);
    }
}
```
