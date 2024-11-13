# Class: IotaIdentityClientExt

[identity\_wasm](../modules/identity_wasm.md).IotaIdentityClientExt

An extension interface that provides helper functions for publication
and resolution of DID documents in Alias Outputs.

## Table of contents

### Methods

- [newDidOutput](identity_wasm.IotaIdentityClientExt.md#newdidoutput)
- [updateDidOutput](identity_wasm.IotaIdentityClientExt.md#updatedidoutput)
- [deactivateDidOutput](identity_wasm.IotaIdentityClientExt.md#deactivatedidoutput)
- [resolveDid](identity_wasm.IotaIdentityClientExt.md#resolvedid)
- [resolveDidOutput](identity_wasm.IotaIdentityClientExt.md#resolvedidoutput)

## Methods

### newDidOutput

▸ `Static` **newDidOutput**(`client`, `address`, `document`, `rentStructure?`): `Promise`\<`AliasOutputBuilderParams`\>

Create a DID with a new Alias Output containing the given `document`.

The `address` will be set as the state controller and governor unlock conditions.
The minimum required token deposit amount will be set according to the given
`rent_structure`, which will be fetched from the node if not provided.
The returned Alias Output can be further customised before publication, if desired.

NOTE: this does *not* publish the Alias Output.

#### Parameters

| Name | Type |
| :------ | :------ |
| `client` | [`IIotaIdentityClient`](../interfaces/identity_wasm.IIotaIdentityClient.md) |
| `address` | `Address` |
| `document` | [`IotaDocument`](identity_wasm.IotaDocument.md) |
| `rentStructure?` | `IRent` |

#### Returns

`Promise`\<`AliasOutputBuilderParams`\>

___

### updateDidOutput

▸ `Static` **updateDidOutput**(`client`, `document`): `Promise`\<`AliasOutputBuilderParams`\>

Fetches the associated Alias Output and updates it with `document` in its state metadata.
The storage deposit on the output is left unchanged. If the size of the document increased,
the amount should be increased manually.

NOTE: this does *not* publish the updated Alias Output.

#### Parameters

| Name | Type |
| :------ | :------ |
| `client` | [`IIotaIdentityClient`](../interfaces/identity_wasm.IIotaIdentityClient.md) |
| `document` | [`IotaDocument`](identity_wasm.IotaDocument.md) |

#### Returns

`Promise`\<`AliasOutputBuilderParams`\>

___

### deactivateDidOutput

▸ `Static` **deactivateDidOutput**(`client`, `did`): `Promise`\<`AliasOutputBuilderParams`\>

Removes the DID document from the state metadata of its Alias Output,
effectively deactivating it. The storage deposit on the output is left unchanged,
and should be reallocated manually.

Deactivating does not destroy the output. Hence, it can be re-activated by publishing
an update containing a DID document.

NOTE: this does *not* publish the updated Alias Output.

#### Parameters

| Name | Type |
| :------ | :------ |
| `client` | [`IIotaIdentityClient`](../interfaces/identity_wasm.IIotaIdentityClient.md) |
| `did` | [`IotaDID`](identity_wasm.IotaDID.md) |

#### Returns

`Promise`\<`AliasOutputBuilderParams`\>

___

### resolveDid

▸ `Static` **resolveDid**(`client`, `did`): `Promise`\<[`IotaDocument`](identity_wasm.IotaDocument.md)\>

Resolve a [IotaDocument](identity_wasm.IotaDocument.md). Returns an empty, deactivated document if the state metadata
of the Alias Output is empty.

#### Parameters

| Name | Type |
| :------ | :------ |
| `client` | [`IIotaIdentityClient`](../interfaces/identity_wasm.IIotaIdentityClient.md) |
| `did` | [`IotaDID`](identity_wasm.IotaDID.md) |

#### Returns

`Promise`\<[`IotaDocument`](identity_wasm.IotaDocument.md)\>

___

### resolveDidOutput

▸ `Static` **resolveDidOutput**(`client`, `did`): `Promise`\<`AliasOutputBuilderParams`\>

Fetches the `IAliasOutput` associated with the given DID.

#### Parameters

| Name | Type |
| :------ | :------ |
| `client` | [`IIotaIdentityClient`](../interfaces/identity_wasm.IIotaIdentityClient.md) |
| `did` | [`IotaDID`](identity_wasm.IotaDID.md) |

#### Returns

`Promise`\<`AliasOutputBuilderParams`\>
