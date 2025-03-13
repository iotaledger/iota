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

## Fee Policy

The Fee Policy looks like the following:

```go
{
  GasPerToken Ratio32 // how many gas units are paid for each token
  EVMGasRatio Ratio32 // the ratio at which EVM gas is converted to ISC gas
  ValidatorFeeShare uint8 // percentage of the fees that are credited to the validators (0 - 100)
}
```

---

## Entry Points

### `rotateStateController`

Called when the committee is about to be rotated to the new address `newStateControllerAddr`.

If it succeeds, the next state transition will become a governance transition, thus updating the state controller in the
chain's Alias Output. If it fails, nothing happens.

It can only be invoked by the chain owner.

#### Parameters

| Name                  | Type               | Optional  | Description                                                                                                                                        |
|-----------------------|--------------------|-----------|----------------------------------------------------------------------------------------------------------------------------------------------------|
| newStateControllerAddr | *cryptolib.Address | No       | The address of the next state controller. Must be an [allowed](#addallowedstatecontrolleraddresss-statecontrolleraddress) state controller address |

#### Returns

_None_

### `addAllowedStateControllerAddress`

Adds the address `stateControllerAddress` to the list of identities that constitute the state controller.

It can only be invoked by the chain owner.

#### Parameters

| Name                   | Type               | Optional | Description                                                |
|------------------------|--------------------|----------|------------------------------------------------------------|
| stateControllerAddress | *cryptolib.Address | No       | The address to add to the set of allowed state controllers |

#### Returns

_None_

### `removeAllowedStateControllerAddress`

Removes the address `stateControllerAddress` from the list of identities that constitute the state controller.

It can only be invoked by the chain owner.

#### Parameters

| Name                   | Type               | Optional | Description                                                     |
|------------------------|--------------------|----------|-----------------------------------------------------------------|
| stateControllerAddress | *cryptolib.Address | No       | The address to remove from the set of allowed state controllers |

#### Returns

_None_

### `delegateChainOwnership`

Sets the Agent ID `ownerAgentID` as the new owner for the chain. This change will only be effective
once [`claimChainOwnership`](#claimchainownership) is called by `ownerAgentID`.

It can only be invoked by the chain owner.

#### Parameters

| Name         | Type        | Optional | Description                          |
|--------------|-------------|----------|--------------------------------------|
| ownerAgentID | isc.AgentID | No       | The Agent ID of the next chain owner |

#### Returns

_None_

### `claimChainOwnership`

Claims the ownership of the chain if the caller matches the identity set
in [`delegateChainOwnership`](#delegatechainownershipo-agentid).

#### Parameters

_None_

#### Returns

_None_

### `setFeePolicy`

Sets the fee policy for the chain. It can only be invoked by the chain owner.

#### Parameters

| Name      | Type           | Optional | Description    |
|-----------|----------------|----------|----------------|
| feePolicy | *gas.FeePolicy | No       | The fee policy |

#### Returns

_None_

### `setGasLimits`

Sets the gas limits for the chain. It can only be invoked by the chain owner.

#### Parameters

| Name      | Type        | Optional | Description    |
|-----------|-------------|----------|----------------|
| gasLimits | *gas.Limits | No       | The gas limits |

#### Returns

_None_

### `setEVMGasRatio`

Sets the EVM gas ratio for the chain. It can only be invoked by the chain owner.

#### Parameters

| Name        | Type         | Optional | Description       |
|-------------|--------------|----------|-------------------|
| evmGasRatio | util.Ratio32 | No       | The EVM gas ratio |

#### Returns

_None_

### `addCandidateNode`

Adds a node to the list of candidates. It can only be invoked by the access node owner (verified via the Certificate field).

#### Parameters

| Name            | Type                | Optional | Description |
|-----------------|----------------------|----------|-------------|
| nodePublicKey   | *cryptolib.PublicKey | No       | The public key of the node to be added |
| nodeCertificate | []uint8             | No       | The certificate is a signed binary containing both the node public key and their L1 address |
| nodeAccessAPI   | string              | No       | The API base URL for the node |
| isCommittee     | bool                | No       | Whether the candidate node is being added to be part of the committee or
  just an access node |

#### Returns

_None_

### `revokeAccessNode`

Removes a node from the list of candidates. It can only be invoked by the access node owner (verified via the Certificate field).

#### Parameters

| Name            | Type                 | Optional | Description                               |
|-----------------|----------------------|----------|-------------------------------------------|
| nodePublicKey   | *cryptolib.PublicKey | No       | The public key of the node to be removed  |
| certificate     | []uint8              | No       | The certificate of the node to be removed |

#### Returns

_None_

### `changeAccessNodes`

Iterates through the given map of actions and applies them. It can only be invoked by the chain owner.
#### Parameters

| Name        | Type                                                                 | Optional | Description |
|-------------|----------------------------------------------------------------------|----------|-------------|
| accessNodes | []lo.Tuple2[*cryptolib.PublicKey,governance.ChangeAccessNodeAction]  | No       | [`Map`](https://github.com/iotaledger/wasp/blob/develop/packages/kv/collections/map.go) of `public key` => `byte`):
  The list of actions to perform. Each byte value can be one of the following:
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

| Name          | Type        | Optional | Description        |
|---------------|-------------|----------|--------------------|
| payoutAgentID | isc.AgentID | No       | The payout AgentID |

#### Returns

_None_

---

## Views

### `getAllowedStateControllerAddresses`

Returns the list of allowed state controllers.

#### Parameters

_None_

#### Returns

| Name                     | Type                 | Description |
|--------------------------|----------------------|-------------|
| stateControllerAddresses | []*cryptolib.Address | [`Array`](https://github.com/iotaledger/wasp/blob/develop/packages/kv/collections/array.go)
  of [`iotago::Address`](https://github.com/iotaledger/iota.go/blob/develop/address.go)): The list of allowed state
  controllers |

### `getChainOwner`

Returns the AgentID of the chain owner.

#### Parameters

_None_

#### Returns

| Name              | Type        | Description              |
|-------------------|-------------|--------------------------|
| chainOwnerAgentID | isc.AgentID | The chain owner agent ID |

### `getChainInfo`

Returns information about the chain.

#### Parameters

_None_

#### Returns

| Name       | Type          | Description |
|------------|---------------|-------------|
| chainInfo  | *isc.ChainInfo | The chain info |

### `getFeePolicy`

Returns the gas fee policy.

#### Parameters

_None_

#### Returns

| Name      | Type           | Description        |
|-----------|----------------|--------------------|
| feePolicy | *gas.FeePolicy | The gas fee policy |

### `getGasLimits`

Returns the gas limits.

#### Parameters

_None_

#### Returns

| Name      | Type        | Description    |
|-----------|-------------|----------------|
| gasLimits | *gas.Limits | The gas limits |

### `getEVMGasRatio`

Returns the EVM gas ratio.

#### Parameters

_None_

#### Returns

| Name        | Type         | Description       |
|-------------|--------------|-------------------|
| evmGasRatio | util.Ratio32 | The EVM gas ratio |

### `getChainNodes`

Returns the current access nodes and candidates.

#### Parameters

_None_

#### Returns

| Name        | Type                         | Description          |
|-------------|------------------------------|----------------------|
| accessNodes | []*cryptolib.PublicKey       | The access node keys |
| candidates  | []*governance.AccessNodeInfo | [`Map`](https://github.com/iotaledger/wasp/blob/develop/packages/kv/collections/map.go)
  of public key => [`AccessNodeInfo`](#accessnodeinfo): The candidates info |

### `getMaintenanceStatus`

Returns whether the chain is undergoing maintenance.

#### Parameters

_None_

#### Returns

| Name          | Type | Description           |
|---------------|------|-----------------------|
| isMaintenance | bool | Is maintenance active |

### `getPayoutAgentID`

Returns the payout AgentID.

#### Parameters

_None_

#### Returns

| Name          | Type        | Description         |
|---------------|-------------|---------------------|
| payoutAgentID | isc.AgentID | The payout agent ID |

### `getMetadata`

Returns the metadata.

#### Parameters

_None_

#### Returns

| Name      | Type                     | Description    |
|-----------|--------------------------|----------------|
| publicURL | string                   | The public URL |
| metadata  | *isc.PublicChainMetadata | The metadata   |
