# Interface: KeyIdStorage

[identity\_wasm](../modules/identity_wasm.md).KeyIdStorage

Key value Storage for key ids under [MethodDigest](../classes/identity_wasm.MethodDigest.md).

## Table of contents

### Properties

- [insertKeyId](identity_wasm.KeyIdStorage.md#insertkeyid)
- [getKeyId](identity_wasm.KeyIdStorage.md#getkeyid)
- [deleteKeyId](identity_wasm.KeyIdStorage.md#deletekeyid)

## Properties

### insertKeyId

• **insertKeyId**: (`methodDigest`: [`MethodDigest`](../classes/identity_wasm.MethodDigest.md), `keyId`: `string`) => `Promise`\<`void`\>

#### Type declaration

▸ (`methodDigest`, `keyId`): `Promise`\<`void`\>

Insert a key id into the `KeyIdStorage` under the given [MethodDigest](../classes/identity_wasm.MethodDigest.md).

If an entry for `key` already exists in the storage an error must be returned
immediately without altering the state of the storage.

##### Parameters

| Name | Type |
| :------ | :------ |
| `methodDigest` | [`MethodDigest`](../classes/identity_wasm.MethodDigest.md) |
| `keyId` | `string` |

##### Returns

`Promise`\<`void`\>

___

### getKeyId

• **getKeyId**: (`methodDigest`: [`MethodDigest`](../classes/identity_wasm.MethodDigest.md)) => `Promise`\<`string`\>

#### Type declaration

▸ (`methodDigest`): `Promise`\<`string`\>

Obtain the key id associated with the given [MethodDigest](../classes/identity_wasm.MethodDigest.md).

##### Parameters

| Name | Type |
| :------ | :------ |
| `methodDigest` | [`MethodDigest`](../classes/identity_wasm.MethodDigest.md) |

##### Returns

`Promise`\<`string`\>

___

### deleteKeyId

• **deleteKeyId**: (`methodDigest`: [`MethodDigest`](../classes/identity_wasm.MethodDigest.md)) => `Promise`\<`void`\>

#### Type declaration

▸ (`methodDigest`): `Promise`\<`void`\>

Delete the `KeyId` associated with the given [MethodDigest](../classes/identity_wasm.MethodDigest.md) from the [KeyIdStorage](identity_wasm.KeyIdStorage.md).

If `key` is not found in storage, an Error must be returned.

##### Parameters

| Name | Type |
| :------ | :------ |
| `methodDigest` | [`MethodDigest`](../classes/identity_wasm.MethodDigest.md) |

##### Returns

`Promise`\<`void`\>
