# Interface: JwkStorage

[identity\_wasm](../modules/identity_wasm.md).JwkStorage

Secure storage for cryptographic keys represented as JWKs.

## Table of contents

### Properties

- [generate](identity_wasm.JwkStorage.md#generate)
- [insert](identity_wasm.JwkStorage.md#insert)
- [sign](identity_wasm.JwkStorage.md#sign)
- [delete](identity_wasm.JwkStorage.md#delete)
- [exists](identity_wasm.JwkStorage.md#exists)

## Properties

### generate

• **generate**: (`keyType`: `string`, `algorithm`: [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md)) => `Promise`\<[`JwkGenOutput`](../classes/identity_wasm.JwkGenOutput.md)\>

#### Type declaration

▸ (`keyType`, `algorithm`): `Promise`\<[`JwkGenOutput`](../classes/identity_wasm.JwkGenOutput.md)\>

Generate a new key represented as a JSON Web Key.

It's recommend that the implementer exposes constants for the supported key type string.

##### Parameters

| Name | Type |
| :------ | :------ |
| `keyType` | `string` |
| `algorithm` | [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md) |

##### Returns

`Promise`\<[`JwkGenOutput`](../classes/identity_wasm.JwkGenOutput.md)\>

___

### insert

• **insert**: (`jwk`: [`Jwk`](../classes/identity_wasm.Jwk.md)) => `Promise`\<`string`\>

#### Type declaration

▸ (`jwk`): `Promise`\<`string`\>

Insert an existing JSON Web Key into the storage.

All private key components of the `jwk` must be set.

##### Parameters

| Name | Type |
| :------ | :------ |
| `jwk` | [`Jwk`](../classes/identity_wasm.Jwk.md) |

##### Returns

`Promise`\<`string`\>

___

### sign

• **sign**: (`keyId`: `string`, `data`: `Uint8Array`, `publicKey`: [`Jwk`](../classes/identity_wasm.Jwk.md)) => `Promise`\<`Uint8Array`\>

#### Type declaration

▸ (`keyId`, `data`, `publicKey`): `Promise`\<`Uint8Array`\>

Sign the provided `data` using the private key identified by `keyId` according to the requirements of the given `public_key` corresponding to `keyId`.

##### Parameters

| Name | Type |
| :------ | :------ |
| `keyId` | `string` |
| `data` | `Uint8Array` |
| `publicKey` | [`Jwk`](../classes/identity_wasm.Jwk.md) |

##### Returns

`Promise`\<`Uint8Array`\>

___

### delete

• **delete**: (`keyId`: `string`) => `Promise`\<`void`\>

#### Type declaration

▸ (`keyId`): `Promise`\<`void`\>

Deletes the key identified by `keyId`.

# Warning

This operation cannot be undone. The keys are purged permanently.

##### Parameters

| Name | Type |
| :------ | :------ |
| `keyId` | `string` |

##### Returns

`Promise`\<`void`\>

___

### exists

• **exists**: (`keyId`: `string`) => `Promise`\<`boolean`\>

#### Type declaration

▸ (`keyId`): `Promise`\<`boolean`\>

Returns `true` if the key with the given `keyId` exists in storage, `false` otherwise.

##### Parameters

| Name | Type |
| :------ | :------ |
| `keyId` | `string` |

##### Returns

`Promise`\<`boolean`\>
