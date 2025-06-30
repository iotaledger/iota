---
description: 'The `governance` contract defines the set of identities that constitute the state controller, access nodes,
who is the chain owner, and the fees for request execution.'
image: /img/logo/WASP_logo_dark.png
tags:
- core-contract
- core-contract-governance
- reference
---

# The `governance` Contract

The `governance` contract is one of the [core contracts](overview.md) on each IOTA Smart Contracts
chain.

The `governance` contract provides the following functionalities:

- It defines the identity set that constitutes the state controller (the entity that owns the state output via the chain
  Alias Address). It is possible to add/remove addresses from the state controller (thus rotating the committee of
  validators).
- It defines the chain owner (the L1 entity that owns the chain - initially whoever deployed it). The chain owner can
  collect special fees and customize some chain-specific parameters.
- It defines the entities allowed to have an access node.
- It defines the fee policy for the chain (gas price, what token is used to pay for gas, and the validator fee share).

---

## Entry Points

### `rotateStateController`

Called when the committee is about to be rotated to the new address `newStateControllerAddr`.

If it succeeds, the next state transition will become a governance transition, thus updating the state controller in the
chain's Alias Output. If it fails, nothing happens.

It can only be invoked by the chain owner.

#### Parameters

| Name                   | Type     | Optional | Description                                                                                                                |
| ---------------------- | -------- | -------- | -------------------------------------------------------------------------------------------------------------------------- |
| newStateControllerAddr | [u8; 32] | No       | The address of the next state controller. Must be an [allowed](#addallowedstatecontrolleraddress) state controller address |

#### Returns

_None_

### `addAllowedStateControllerAddress`

Adds the address `stateControllerAddress` to the list of identities that constitute the state controller.

It can only be invoked by the chain owner.

#### Parameters

| Name                   | Type     | Optional | Description                                                |
| ---------------------- | -------- | -------- | ---------------------------------------------------------- |
| stateControllerAddress | [u8; 32] | No       | The address to add to the set of allowed state controllers |

#### Returns

_None_

### `removeAllowedStateControllerAddress`

Removes the address `stateControllerAddress` from the list of identities that constitute the state controller.

It can only be invoked by the chain owner.

#### Parameters

| Name                   | Type     | Optional | Description                                                     |
| ---------------------- | -------- | -------- | --------------------------------------------------------------- |
| stateControllerAddress | [u8; 32] | No       | The address to remove from the set of allowed state controllers |

#### Returns

_None_

### `delegateChainOwnership`

Sets the Agent ID `ownerAgentID` as the new owner for the chain. This change will only be effective
once [`claimChainOwnership`](#claimchainownership) is called by `ownerAgentID`.

It can only be invoked by the chain owner.

#### Parameters

| Name         | Type     | Optional | Description                          |
| ------------ | -------- | -------- | ------------------------------------ |
| ownerAgentID | [u8; 32] | No       | The Agent ID of the next chain owner |

#### Returns

_None_

### `claimChainOwnership`

Claims the ownership of the chain if the caller matches the identity set
in [`delegateChainOwnership`](#delegatechainownership).

#### Parameters

_None_

#### Returns

_None_

### `setFeePolicy`

Sets the fee policy for the chain. It can only be invoked by the chain owner.

#### Parameters

| Name      | Type                              | Optional | Description    |
| --------- | --------------------------------- | -------- | -------------- |
| feePolicy | [FeePolicy](./types.md#feepolicy) | No       | The fee policy |

#### Returns

_None_

### `setGasLimits`

Sets the gas limits for the chain. It can only be invoked by the chain owner.

#### Parameters

| Name      | Type                        | Optional | Description    |
| --------- | --------------------------- | -------- | -------------- |
| gasLimits | [Limits](./types.md#limits) | No       | The gas limits |

#### Returns

_None_

### `setEVMGasRatio`

Sets the EVM gas ratio for the chain. It can only be invoked by the chain owner.

#### Parameters

| Name        | Type                          | Optional | Description       |
| ----------- | ----------------------------- | -------- | ----------------- |
| evmGasRatio | [Ratio32](./types.md#ratio32) | No       | The EVM gas ratio |

#### Returns

_None_

### `addCandidateNode`

Adds a node to the list of candidates. It can only be invoked by the access node owner (verified via the Certificate field).

#### Parameters

| Name                | Type     | Optional | Description                                                                                 |
| ------------------- | -------- | -------- | ------------------------------------------------------------------------------------------- |
| nodePublicKey       | [u8; 32] | No       | The public key of the node to be added                                                      |
| nodeCertificate     | [u8]     | No       | The certificate is a signed binary containing both the node public key and their L1 address |
| nodeAccessAPI       | string   | No       | The API base URL for the node                                                               |
| isCommittee         | bool     | No       | Whether the candidate node is being added to be part of the committee or                    |
| just an access node |          |          |                                                                                             |

#### Returns

_None_

### `revokeAccessNode`

Removes a node from the list of candidates. It can only be invoked by the access node owner (verified via the Certificate field).

#### Parameters

| Name          | Type     | Optional | Description                               |
| ------------- | -------- | -------- | ----------------------------------------- |
| nodePublicKey | [u8; 32] | No       | The public key of the node to be removed  |
| certificate   | [u8]     | No       | The certificate of the node to be removed |

#### Returns

_None_

### `changeAccessNodes`

Iterates through the given map of actions and applies them. It can only be invoked by the chain owner.

#### Parameters

| Name                                                                         | Type                        | Optional | Description                                                                                                         |
| ---------------------------------------------------------------------------- | --------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------- |
| accessNodes                                                                  | []lo.Tuple2[[u8; 32], byte] | No       | [`Map`](https://github.com/iotaledger/wasp/blob/develop/packages/kv/collections/map.go) of `public key` => `byte`): |
| The list of actions to perform. Each byte value can be one of the following: |                             |          |                                                                                                                     |

- `0`: Remove the access node from the access nodes list.
- `1`: Accept a candidate node and add it to the list of access nodes.
- `2`: Drop an access node from the access node and candidate lists. |

#### Returns

_None_

### `startMaintenance`

Starts the chain maintenance mode, meaning no further requests will be processed except
calls to the governance contract.

It can only be invoked by the chain owner.

#### Parameters

_None_

#### Returns

_None_

### `stopMaintenance`

Stops the maintenance mode.

It can only be invoked by the chain owner.

#### Parameters

_None_

#### Returns

_None_

### `setPayoutAgentID`

`setPayoutAgentID` sets the payout AgentID. The default AgentID is the chain owner. Transaction fee will be taken to ensure the common account has minimum storage deposit which is in base token. The rest of transaction fee will be transferred to payout AgentID.

#### Parameters

| Name          | Type     | Optional | Description        |
| ------------- | -------- | -------- | ------------------ |
| payoutAgentID | [u8; 32] | No       | The payout AgentID |

#### Returns

_None_

---

## Views

### `getAllowedStateControllerAddresses`

Returns the list of allowed state controllers.

#### Parameters

_None_

#### Returns

| Name                     | Type       | Description                           |
| ------------------------ | ---------- | ------------------------------------- |
| stateControllerAddresses | [[u8; 32]] | The list of allowed state controllers |

### `getChainOwner`

Returns the AgentID of the chain owner.

#### Parameters

_None_

#### Returns

| Name              | Type     | Description              |
| ----------------- | -------- | ------------------------ |
| chainOwnerAgentID | [u8; 32] | The chain owner agent ID |

### `getChainInfo`

Returns information about the chain.

#### Parameters

_None_

#### Returns

| Name      | Type                              | Description    |
| --------- | --------------------------------- | -------------- |
| chainInfo | [ChainInfo](./types.md#chaininfo) | The chain info |

### `getFeePolicy`

Returns the gas fee policy.

#### Parameters

_None_

#### Returns

| Name      | Type                              | Description        |
| --------- | --------------------------------- | ------------------ |
| feePolicy | [FeePolicy](./types.md#feepolicy) | The gas fee policy |

### `getGasLimits`

Returns the gas limits.

#### Parameters

_None_

#### Returns

| Name      | Type                        | Description    |
| --------- | --------------------------- | -------------- |
| gasLimits | [Limits](./types.md#limits) | The gas limits |

### `getEVMGasRatio`

Returns the EVM gas ratio.

#### Parameters

_None_

#### Returns

| Name        | Type                          | Description       |
| ----------- | ----------------------------- | ----------------- |
| evmGasRatio | [Ratio32](./types.md#ratio32) | The EVM gas ratio |

### `getChainNodes`

Returns the current access nodes and candidates.

#### Parameters

_None_

#### Returns

| Name                                                                              | Type                                        | Description                                                                             |
| --------------------------------------------------------------------------------- | ------------------------------------------- | --------------------------------------------------------------------------------------- |
| accessNodes                                                                       | [[u8; 32]]                                  | The access node keys                                                                    |
| candidates                                                                        | [AccessNodeInfo](./types.md#accessnodeinfo) | [`Map`](https://github.com/iotaledger/wasp/blob/develop/packages/kv/collections/map.go) |
| of public key => [AccessNodeInfo](./types.md#accessnodeinfo): The candidates info |                                             |                                                                                         |

### `getMaintenanceStatus`

Returns whether the chain is undergoing maintenance.

#### Parameters

_None_

#### Returns

| Name          | Type | Description           |
| ------------- | ---- | --------------------- |
| isMaintenance | bool | Is maintenance active |

### `getPayoutAgentID`

Returns the payout AgentID.

#### Parameters

_None_

#### Returns

| Name          | Type     | Description         |
| ------------- | -------- | ------------------- |
| payoutAgentID | [u8; 32] | The payout agent ID |

### `getMetadata`

Returns the metadata.

#### Parameters

_None_

#### Returns

| Name      | Type                                                  | Description    |
| --------- | ----------------------------------------------------- | -------------- |
| publicURL | string                                                | The public URL |
| metadata  | [PublicChainMetadata](./types.md#publicchainmetadata) | The metadata   |
