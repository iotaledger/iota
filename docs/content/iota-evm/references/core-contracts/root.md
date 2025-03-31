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

---

## Entry Points

### `init`

The constructor. Automatically called immediately after confirmation of the origin transaction and never called again. When executed, this function:

- Initializes base values of the chain according to parameters.
- Sets the caller as the _chain owner_.
- Deploys all the core contracts.

### `deployContract`

Deploys a non-EVM smart contract on the chain if the caller has deployment permission.

#### Parameters

| Name | Type       | Optional | Description                                                                 |
|------|------------|----------|-----------------------------------------------------------------------------|
| ph   | [32]byte   | No       | The hash of the binary _blob_ (previously stored in the [`blob` contract](blob.md)). |
| nm   | string     | No       | The name of the contract to be deployed, used to calculate the contract's _hname_. The hname must be unique among all contract hnames in the chain. |
| ds   | string     | No       | Description of the contract to be deployed.                                 |

:::note

Any other parameters passed to the `deployContract` function will be forwarded to the `init` function of the smart contract being deployed. Note that the `init` parameter names cannot be the above ones, as they will have been filtered out.

:::

### `grantDeployPermission`

The chain owner grants deploy permission to the agent ID `dp`.

#### Parameters

| Name | Type    | Optional | Description           |
|------|---------|----------|-----------------------|
| dp   | AgentID | No       | The agent ID to grant permission to. |

### `revokeDeployPermission`

The chain owner revokes the deploy permission of the agent ID `dp`.

#### Parameters

| Name | Type    | Optional | Description           |
|------|---------|----------|-----------------------|
| dp   | AgentID | No       | The agent ID to revoke permission from. |

### `requireDeployPermissions`

Sets whether permissions are required to deploy a contract on the chain.

#### Parameters

| Name | Type | Optional | Description                                                                 |
|------|------|----------|-----------------------------------------------------------------------------|
| de   | bool | No       | Whether permissions should be required to deploy a contract on the chain. By default, permissions are enabled, but the chain owner can override this setting to allow anyone to deploy contracts. |

---

## Views

### `findContract`

Returns the record for a given smart contract with Hname `hn` (if it exists).

#### Parameters

| Name | Type  | Optional | Description                     |
|------|-------|----------|---------------------------------|
| hn   | Hname | No       | The smart contract’s Hname.    |

#### Returns

| Name | Type              | Description                                      |
|------|-------------------|--------------------------------------------------|
| cf   | bool              | Whether or not the contract exists.             |
| dt   | [`ContractRecord`](#contractrecord) | The requested contract record (if it exists). |

### `getContractRecords`

Returns the list of all smart contracts deployed on the chain and related records.

#### Returns

| Name | Type              | Description                                      |
|------|-------------------|--------------------------------------------------|
| records | map[Hname][`ContractRecord`](#contractrecord) | A map of Hname to contract records. |

---

## Schemas

### `ContractRecord`

A `ContractRecord` is encoded as the concatenation of:

- Program hash (`[32]byte`).
- Contract description (`string`).
- Contract name (`string`).
