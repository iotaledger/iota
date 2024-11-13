# Class: IotaDocumentMetadata

[identity\_wasm](../modules/identity_wasm.md).IotaDocumentMetadata

Additional attributes related to an IOTA DID Document.

## Table of contents

### Methods

- [toJSON](identity_wasm.IotaDocumentMetadata.md#tojson)
- [toString](identity_wasm.IotaDocumentMetadata.md#tostring)
- [created](identity_wasm.IotaDocumentMetadata.md#created)
- [updated](identity_wasm.IotaDocumentMetadata.md#updated)
- [deactivated](identity_wasm.IotaDocumentMetadata.md#deactivated)
- [stateControllerAddress](identity_wasm.IotaDocumentMetadata.md#statecontrolleraddress)
- [governorAddress](identity_wasm.IotaDocumentMetadata.md#governoraddress)
- [properties](identity_wasm.IotaDocumentMetadata.md#properties)
- [fromJSON](identity_wasm.IotaDocumentMetadata.md#fromjson)
- [clone](identity_wasm.IotaDocumentMetadata.md#clone)

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

___

### created

▸ **created**(): `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

Returns a copy of the timestamp of when the DID document was created.

#### Returns

`undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

___

### updated

▸ **updated**(): `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

Returns a copy of the timestamp of the last DID document update.

#### Returns

`undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

___

### deactivated

▸ **deactivated**(): `undefined` \| `boolean`

Returns a copy of the deactivated status of the DID document.

#### Returns

`undefined` \| `boolean`

___

### stateControllerAddress

▸ **stateControllerAddress**(): `undefined` \| `string`

Returns a copy of the Bech32-encoded state controller address, if present.

#### Returns

`undefined` \| `string`

___

### governorAddress

▸ **governorAddress**(): `undefined` \| `string`

Returns a copy of the Bech32-encoded governor address, if present.

#### Returns

`undefined` \| `string`

___

### properties

▸ **properties**(): `Map`\<`string`, `any`\>

Returns a copy of the custom metadata properties.

#### Returns

`Map`\<`string`, `any`\>

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`IotaDocumentMetadata`](identity_wasm.IotaDocumentMetadata.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`IotaDocumentMetadata`](identity_wasm.IotaDocumentMetadata.md)

___

### clone

▸ **clone**(): [`IotaDocumentMetadata`](identity_wasm.IotaDocumentMetadata.md)

Deep clones the object.

#### Returns

[`IotaDocumentMetadata`](identity_wasm.IotaDocumentMetadata.md)
