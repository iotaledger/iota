---
description: How to get the balance of L1 assets on L2
image: /img/logo/WASP_logo_dark.png
tags:
  - evm
  - balance
  - how-to
teams:
  - iotaledger/l2-smart-contract
---

# Get Balance

Once you have your L1 assets on L2, you might want to check their balance. This guide explains how to do so by calling the three functions `getL2BalanceBaseTokens`, `getL2BalanceCoin` and `getL2ObjectsCount` for the corresponding token types.

## Example Code

1. Get the [AgentID](../../../explanations/how-accounts-work.md) from the sender by calling `ISC.sandbox.getSenderAccount()`.

```solidity
ISCAgentID memory agentID = ISC.sandbox.getSenderAccount();
```

2. To get the base token balance, you can call `getL2BalanceBaseTokens` using the `agentID`.

```solidity
uint64 baseBalance = ISC.accounts.getL2BalanceBaseTokens(agentID);
```

3. To get the number coins/Objects, use `ISC.accounts.getL2ObjectsCount` with the `agentID`.

```solidity
uint256 object = ISC.accounts.getL2ObjectsCount(agentID);
```

### Full Example Code

```solidity
// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import "@iota/iscmagic/ISC.sol";

contract GetBalance {
    event GotAgentID(bytes agentID);
    event GotBaseBalance(uint64 baseBalance);
    event GotNativeTokenBalance(uint256 nativeTokenBalance);
    event GotObjectIDs(uint256 objectBalance);

    function getBalanceCoins() public {
        ISCAgentID memory agentID = ISC.sandbox.getSenderAccount();
        uint64 baseBalance = ISC.accounts.getL2BalanceBaseTokens(agentID);
        emit GotBaseBalance(baseBalance);
    }
    
    function getAgentID() public {
        ISCAgentID memory agentID = ISC.sandbox.getSenderAccount();
        emit GotAgentID(agentID.data);
    }
}
```
