---
description: How to get the balance of L1 assets on L2
image: /img/logo/WASP_logo_dark.png
tags:
  - native assets
  - balance
  - EVM
  - how-to
---

# Get Balance

Once you have your L1 assets on L2, you might want to check their balance. This guide explains how to do so by calling the three functions `getL2BalanceBaseTokens`, `getL2BalanceNativeTokens`and `getL2NFTAmount` for the corresponding token types.


## Example Code

1. Get the [AgentID](../../../explanations/how-accounts-work.md) from the sender by calling `ISC.sandbox.getSenderAccount()`.

```solidity
ISCAgentID memory agentID = ISC.sandbox.getSenderAccount();
```

2. To get the base token balance, you can call `getL2BalanceBaseTokens` using the `agentID`.

```solidity
uint64 baseBalance = ISC.accounts.getL2BalanceBaseTokens(agentID);
```

3. To get the native token balance of a specific `NativeTokenID`, use `ISC.accounts.getL2BalanceNativeTokens` with the `id` and `agentID`.

```solidity
NativeTokenID memory id = NativeTokenID({ data: nativeTokenID});
uint256 nativeTokens = ISC.accounts.getL2BalanceNativeTokens(id, agentID);
```

4. To get the number of NFTs, use `ISC.accounts.getL2NFTAmount` with the `agentID`.

```solidity
uint256 nfts = ISC.accounts.getL2NFTAmount(agentID);
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
    event GotNFTIDs(uint256 nftBalance);

    function getBalance(bytes memory nativeTokenID) public {
        ISCAgentID memory agentID = ISC.sandbox.getSenderAccount();
        emit GotAgentID(agentID.data);
        
        uint64 baseBalance = ISC.accounts.getL2BalanceBaseTokens(agentID);
        emit GotBaseBalance(baseBalance);

        NativeTokenID memory id = NativeTokenID({ data: nativeTokenID});
        uint256 nativeTokens = ISC.accounts.getL2BalanceNativeTokens(id, agentID);
        emit GotNativeTokenBalance(nativeTokens);

        uint256 nfts = ISC.accounts.getL2NFTAmount(agentID);
        emit GotNativeTokenBalance(nfts);
    }
}
```

