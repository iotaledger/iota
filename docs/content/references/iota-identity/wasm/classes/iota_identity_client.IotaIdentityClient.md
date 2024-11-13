# Class: IotaIdentityClient

[iota\_identity\_client](../modules/iota_identity_client.md).IotaIdentityClient

Provides operations for IOTA DID Documents with Alias Outputs.

## Implements

- [`IIotaIdentityClient`](../interfaces/identity_wasm.IIotaIdentityClient.md)

## Table of contents

### Methods

- [newDidOutput](iota_identity_client.IotaIdentityClient.md#newdidoutput)
- [updateDidOutput](iota_identity_client.IotaIdentityClient.md#updatedidoutput)
- [deactivateDidOutput](iota_identity_client.IotaIdentityClient.md#deactivatedidoutput)
- [resolveDid](iota_identity_client.IotaIdentityClient.md#resolvedid)
- [resolveDidOutput](iota_identity_client.IotaIdentityClient.md#resolvedidoutput)
- [publishDidOutput](iota_identity_client.IotaIdentityClient.md#publishdidoutput)
- [deleteDidOutput](iota_identity_client.IotaIdentityClient.md#deletedidoutput)

## Methods

### newDidOutput

▸ **newDidOutput**(`address`, `document`, `rentStructure?`): `Promise`\<`AliasOutput`\>

Create a DID with a new Alias Output containing the given `document`.

The `address` will be set as the state controller and governor unlock conditions.
The minimum required token deposit amount will be set according to the given
`rent_structure`, which will be fetched from the node if not provided.
The returned Alias Output can be further customized before publication, if desired.

NOTE: this does *not* publish the Alias Output.

#### Parameters

| Name | Type |
| :------ | :------ |
| `address` | `Address` |
| `document` | [`IotaDocument`](identity_wasm.IotaDocument.md) |
| `rentStructure?` | `IRent` |

#### Returns

`Promise`\<`AliasOutput`\>

___

### updateDidOutput

▸ **updateDidOutput**(`document`): `Promise`\<`AliasOutput`\>

Fetches the associated Alias Output and updates it with `document` in its state metadata.
The storage deposit on the output is left unchanged. If the size of the document increased,
the amount should be increased manually.

NOTE: this does *not* publish the updated Alias Output.

#### Parameters

| Name | Type |
| :------ | :------ |
| `document` | [`IotaDocument`](identity_wasm.IotaDocument.md) |

#### Returns

`Promise`\<`AliasOutput`\>

___

### deactivateDidOutput

▸ **deactivateDidOutput**(`did`): `Promise`\<`AliasOutput`\>

Removes the DID document from the state metadata of its Alias Output,
effectively deactivating it. The storage deposit on the output is left unchanged,
and should be reallocated manually.

Deactivating does not destroy the output. Hence, it can be re-activated by publishing
an update containing a DID document.

NOTE: this does *not* publish the updated Alias Output.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | [`IotaDID`](identity_wasm.IotaDID.md) |

#### Returns

`Promise`\<`AliasOutput`\>

___

### resolveDid

▸ **resolveDid**(`did`): `Promise`\<[`IotaDocument`](identity_wasm.IotaDocument.md)\>

Resolve a [IotaDocument](identity_wasm.IotaDocument.md). Returns an empty, deactivated document if the state
metadata of the Alias Output is empty.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | [`IotaDID`](identity_wasm.IotaDID.md) |

#### Returns

`Promise`\<[`IotaDocument`](identity_wasm.IotaDocument.md)\>

___

### resolveDidOutput

▸ **resolveDidOutput**(`did`): `Promise`\<`AliasOutput`\>

Fetches the Alias Output associated with the given DID.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | [`IotaDID`](identity_wasm.IotaDID.md) |

#### Returns

`Promise`\<`AliasOutput`\>

___

### publishDidOutput

▸ **publishDidOutput**(`secretManager`, `aliasOutput`): `Promise`\<[`IotaDocument`](identity_wasm.IotaDocument.md)\>

Publish the given `aliasOutput` with the provided `secretManager`, and returns
the DID document extracted from the published block.

Note that only the state controller of an Alias Output is allowed to update its state.
This will attempt to move tokens to or from the state controller address to match
the storage deposit amount specified on `aliasOutput`.

This method modifies the on-ledger state.

#### Parameters

| Name | Type |
| :------ | :------ |
| `secretManager` | `SecretManagerType` |
| `aliasOutput` | `AliasOutput` |

#### Returns

`Promise`\<[`IotaDocument`](identity_wasm.IotaDocument.md)\>

___

### deleteDidOutput

▸ **deleteDidOutput**(`secretManager`, `address`, `did`): `Promise`\<`void`\>

Destroy the Alias Output containing the given `did`, sending its tokens to a new Basic Output
unlockable by the given address.

Note that only the governor of an Alias Output is allowed to destroy it.

### WARNING

This destroys the Alias Output and DID document, rendering them permanently unrecoverable.

#### Parameters

| Name | Type |
| :------ | :------ |
| `secretManager` | `SecretManagerType` |
| `address` | `Address` |
| `did` | [`IotaDID`](identity_wasm.IotaDID.md) |

#### Returns

`Promise`\<`void`\>
