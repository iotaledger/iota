---
description: 'The `root` contract is the first smart contract deployed on the chain. It functions as a smart contract factory for the chain.'
image: /img/logo/WASP_logo_dark.png
tags:
  - core-contract
  - core-contract-root
  - reference
---

# The `root` Contract

The `root` contract is one of the [core contracts](overview.md) on each IOTA Smart Contracts chain.

This contract is responsible for the initialization of the chain. It is the first smart contract deployed on the chain and, upon receiving the `init` request, bootstraps the state of the chain. Deploying all of the other core contracts is a part of the state initialization.

The `root` contract also functions as a smart contract factory for the chain: upon request, it deploys other smart contracts and maintains an on-chain registry of smart contracts in its state. The contract registry keeps a list of contract records containing their respective name, hname, description, and creator.

## Views

### `findContract`

Returns the record for a given smart contract with Hname `hn` (if it exists).

#### Parameters

| Name | Type | Optional | Description                 |
| ---- | ---- | -------- | --------------------------- |
| hn   | u32  | No       | The smart contract’s Hname. |

#### Returns

| Name | Type                                        | Description                                   |
| ---- | ------------------------------------------- | --------------------------------------------- |
| cf   | bool                                        | Whether or not the contract exists.           |
| dt   | [ContractRecord](./types.md#contractrecord) | The requested contract record (if it exists). |

### `getContractRecords`

Returns the list of all smart contracts deployed on the chain and related records.

#### Returns

| Name    | Type                                                  | Description                         |
| ------- | ----------------------------------------------------- | ----------------------------------- |
| records | map[u32][[ContractRecord](./types.md#contractrecord)] | A map of Hname to contract records. |
