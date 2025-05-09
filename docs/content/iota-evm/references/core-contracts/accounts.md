---
description: 'The `accounts` contract keeps the ledger of on-chain accounts.'
image: /img/logo/WASP_logo_dark.png
tags:
  - core-contract
  - isc-accounts
  - reference
---

# The `accounts` Contract

The `accounts` contract is one of the [core contracts](overview.md) on each IOTA Smart Contracts
chain.

This contract keeps a consistent ledger of on-chain accounts in its state,
i.e. [the L2 ledger](../../../iota-evm/explanations/how-accounts-work.md).

---

## Entry Points

The `accounts` contract provides functions to deposit and withdraw tokens, information about the assets deposited on the
chain, and the functionality to create and use foundries.

### `deposit`

A no-op that has the side effect of crediting any transferred tokens to the sender's account.

:::note Gas Fees

As with every call, the gas fee is debited from the L2 account right after executing the request.

:::

### `withdraw`

Moves tokens from the caller's on-chain account to the caller's L1 address. The number of
tokens to be withdrawn must be specified via the allowance of the request.

:::note Contract Account

Because contracts does not have a corresponding L1 address it does not make sense to
have them call this function. It will fail with an error.

:::

:::note Storage Deposit

A call to withdraw means that a L1 output will be created. Because of this, the withdrawn
amount must be able to cover the L1 storage deposit. Otherwise, it will fail.

:::

### `transferAllowanceTo`

Transfers the specified allowance from the sender's L2 account to the given L2 account on
the chain.

:::note

When a transfer is made into an EVM account, an EVM tx will be created on the EVM side from the zero address (0x0000...) to the target account.
Information about what is being transferred will be encoded in the transaction's data using the following format:

```
<Sender_AgentID bytes> + <Assets bytes>
```

The data will be BCS encoded

:::

#### Parameters

| Name    | Type    | Optional | Description           |
| ------- | ------- | -------- | --------------------- |
| agentID | AgentID | No       | The target L2 account |

### `setCoinMetadata`

Sets metadata for a specific coin.

:::info

Only callable by the chain owner.

:::

#### Parameters

| Name     | Type         | Optional | Description            |
| -------- | ------------ | -------- | ---------------------- |
| coinInfo | IotaCoinInfo | Yes      | Metadata for the coin. |

### `deleteCoinMetadata`

Deletes metadata for a specific coin.

:::info

Only callable by the chain owner.

:::

#### Parameters

| Name     | Type     | Optional | Description           |
| -------- | -------- | -------- | --------------------- |
| coinType | CoinType | No       | The type of the coin. |

## Views

### `balance`

Returns the fungible tokens owned by the given Agent ID on the chain.

#### Parameters

| Name            | Type    | Optional | Description          |
| --------------- | ------- | -------- | -------------------- |
| optionalAgentID | AgentID | Yes      | The account Agent ID |

#### Returns

| Name         | Type         | Description                                                                                                        |
| ------------ | ------------ | ------------------------------------------------------------------------------------------------------------------ |
| coinBalances | CoinBalances | A map of Coin type => Coin value(`u64`). An empty token ID (a string of zero length) represents the L1 base token. |

### `balanceBaseToken`

Returns the amount of base tokens owned by any AgentID `optionalAgentID` on the chain.

#### Parameters

| Name            | Type    | Optional | Description          |
| --------------- | ------- | -------- | -------------------- |
| optionalAgentID | AgentID | Yes      | The account Agent ID |

#### Returns

| Name             | Type | Description                              |
| ---------------- | ---- | ---------------------------------------- |
| baseTokenBalance | u64  | The amount of base tokens in the account |

### `balanceBaseTokenEVM`

Returns the amount of base tokens owned by any AgentID `optionalAgentID` on the chain (in the EVM format with 18 decimals).

#### Parameters

| Name            | Type    | Optional | Description          |
| --------------- | ------- | -------- | -------------------- |
| optionalAgentID | AgentID | Yes      | The account Agent ID |

#### Returns

| Name                | Type | Description                              |
| ------------------- | ---- | ---------------------------------------- |
| evmBaseTokenBalance | u64  | The amount of base tokens in the account |

### `balanceCoin`

Returns the amount of coins with coin ID `coinID` owned by any AgentID `agentID` on the chain.

#### Parameters

| Name            | Type     | Optional | Description          |
| --------------- | -------- | -------- | -------------------- |
| optionalAgentID | AgentID  | Yes      | The account Agent ID |
| coinID          | CoinType | No       | The coin ID          |

#### Returns

| Name        | Type | Description                        |
| ----------- | ---- | ---------------------------------- |
| coinBalance | u64  | The amount of coins in the account |

### `totalAssets`

Returns the sum of all fungible tokens controlled by the chain.

#### Returns

| Name         | Type         | Description                                                                                                        |
| ------------ | ------------ | ------------------------------------------------------------------------------------------------------------------ |
| coinBalances | CoinBalances | A map of Coin type => Coin value(`u64`). An empty token ID (a string of zero length) represents the L1 base token. |

### `accountObjects`

Returns the Object IDs for all Objects owned by the given account.

#### Parameters

| Name            | Type    | Optional | Description          |
| --------------- | ------- | -------- | -------------------- |
| optionalAgentID | AgentID | Yes      | The account Agent ID |

#### Returns

| Name            | Type | Description                       |
| --------------- | ---- | --------------------------------- |
| bcsEncodedBytes | [u8] | A BSC encoded array of Object IDs |

### `getAccountNonce`

Returns the current account nonce for a give AgentID `agentID`.
The account nonce is used to issue off-ledger requests.

#### Parameters

| Name            | Type    | Optional | Description          |
| --------------- | ------- | -------- | -------------------- |
| optionalAgentID | AgentID | Yes      | The account Agent ID |

#### Returns

| Name  | Type | Description       |
| ----- | ---- | ----------------- |
| nonce | u64  | The account Nonce |
