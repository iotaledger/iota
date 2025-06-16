---
description: 'The evm core contract provides the necessary infrastructure to accept Ethereum transactions and execute
EVM code.'
image: /img/logo/WASP_logo_dark.png
tags:
- core-contract
- evm
- reference
---

# The `evm` Contract

The `evm` contract is one of the [core contracts](overview.md) on each IOTA Smart Contracts chain.

The `evm` core contract provides the necessary infrastructure to accept Ethereum transactions and execute EVM code.
It also includes the implementation of the [ISC Magic contract](../../../iota-evm/how-tos/core-contracts/introduction.md).

:::note

For more information about how ISC supports EVM contracts, refer to the [EVM](../../../iota-evm/getting-started/languages-and-vms.mdx#what-is-evmsolidity) section.

:::

---

## Entry Points

Most entry points of the `evm` core contract are meant to be accessed through the JSON-RPC service provided
automatically by the Wasp node so that the end users can use standard EVM tools like [MetaMask](https://metamask.io/).
We only list the entry points not exposed through the JSON-RPC interface in this document.

### `registerERC20Coin`

Registers an ERC20 contract to act as a proxy for the L1 coin, at address
`0x107402xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`, where `xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx` is the first 17 bytes of the Keccak hash of the coin type (`0x123...module::yourcoin`).

Only the foundry owner can call this endpoint.

#### Parameters

| Name     | Type                            | Optional | Description   |
| -------- | ------------------------------- | -------- | ------------- |
| coinType | [CoinType](./types.md#cointype) | No       | The coin type |

#### Returns

_None_

### `sendTransaction`

Sends a transaction to the EVM.

#### Parameters

| Name        | Type               | Optional | Description             |
| ----------- | ------------------ | -------- | ----------------------- |
| transaction | *types.Transaction | No       | The transaction to send |

#### Returns

_None_

### `callContract`

Calls a contract on the EVM.

#### Parameters

| Name        | Type             | Optional | Description      |
| ----------- | ---------------- | -------- | ---------------- |
| callMessage | ethereum.CallMsg | No       | The call message |

#### Returns

| Name           | Type | Description            |
| -------------- | ---- | ---------------------- |
| functionResult | []u8 | The result of the call |

### `registerERC721NFTCollection`

Registers an ERC721 contract to act as a proxy for an NFT collection, at address
`0x107404xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx`, where `xxx...` is the first 17
bytes of the collection ID.

The call will fail if the address is taken by another collection with the same prefix.

#### Parameters

| Name         | Type           | Optional | Description       |
| ------------ | -------------- | -------- | ----------------- |
| collectionID | iotago.Address | No       | The collection ID |

#### Returns

_None_

### `newL1Deposit`

Creates a new L1 deposit.

#### Parameters

| Name                       | Type                        | Optional | Description             |
| -------------------------- | --------------------------- | -------- | ----------------------- |
| l1DepositOriginatorAgentID | [u8; 32]                    | No       | The originator agent ID |
| targetAddress              | [u8; 32]                    | No       | The target address      |
| assets                     | [Assets](./types.md#assets) | No       | The assets              |

#### Returns

_None_

---

## Views

### `getChainID`

Returns the chain ID.

#### Parameters

_None_

#### Returns

| Name    | Type | Description  |
| ------- | ---- | ------------ |
| chainID | u16  | The chain ID |
