---
description: How to take the allowance of coins and other objects
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

# Take allowed Funds

After having [allowed](allow.md) Move objects, you can take the ones you need.

## Example Code

The following example will take the Object which was allowed in the [allow how-to guide](allow.md).

### Create the `ISCAssets`

First, you need to recreate the `ISCAssets` with the ObjectID.

```solidity
IotaObjectID[] memory IotaObjectIDs = new IotaObjectID[](1);
IotaObjectIDs[0] = IotaObjectID.wrap(_allowanceIotaObjectID);
ISCAssets memory assets;
assets.objects = IotaObjectIDs;
```

### Call `takeAllowedFunds()`

After that, you can call `takeAllowedFunds()` to take the allowance of the specified address/contract

```solidity
ISC.sandbox.takeAllowedFunds(_address, ObjectID);
```

## Full Example Code

```solidity
// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import "@iota/iscmagic/ISC.sol";

contract allowance {
  function takeAllowedFunds(address _address, bytes32 ObjectID) {
    IotaObjectID[] memory IotaObjectIDs = new IotaObjectID[](1);
    IotaObjectIDs[0] = IotaObjectID.wrap(_allowanceIotaObjectID);
    ISCAssets memory assets;
    assets.objects = IotaObjectIDs;
    ISC.sandbox.takeAllowedFunds(_address, ObjectID);
  }
}
```
