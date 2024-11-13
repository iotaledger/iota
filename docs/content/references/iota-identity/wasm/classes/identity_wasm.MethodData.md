# Class: MethodData

[identity\_wasm](../modules/identity_wasm.md).MethodData

Supported verification method data formats.

## Table of contents

### Methods

- [toJSON](identity_wasm.MethodData.md#tojson)
- [toString](identity_wasm.MethodData.md#tostring)
- [newBase58](identity_wasm.MethodData.md#newbase58)
- [newMultibase](identity_wasm.MethodData.md#newmultibase)
- [newJwk](identity_wasm.MethodData.md#newjwk)
- [newCustom](identity_wasm.MethodData.md#newcustom)
- [tryCustom](identity_wasm.MethodData.md#trycustom)
- [tryDecode](identity_wasm.MethodData.md#trydecode)
- [tryPublicKeyJwk](identity_wasm.MethodData.md#trypublickeyjwk)
- [fromJSON](identity_wasm.MethodData.md#fromjson)
- [clone](identity_wasm.MethodData.md#clone)

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

### newBase58

▸ `Static` **newBase58**(`data`): [`MethodData`](identity_wasm.MethodData.md)

Creates a new [MethodData](identity_wasm.MethodData.md) variant with Base58-BTC encoded content.

#### Parameters

| Name | Type |
| :------ | :------ |
| `data` | `Uint8Array` |

#### Returns

[`MethodData`](identity_wasm.MethodData.md)

___

### newMultibase

▸ `Static` **newMultibase**(`data`): [`MethodData`](identity_wasm.MethodData.md)

Creates a new [MethodData](identity_wasm.MethodData.md) variant with Multibase-encoded content.

#### Parameters

| Name | Type |
| :------ | :------ |
| `data` | `Uint8Array` |

#### Returns

[`MethodData`](identity_wasm.MethodData.md)

___

### newJwk

▸ `Static` **newJwk**(`key`): [`MethodData`](identity_wasm.MethodData.md)

Creates a new [MethodData](identity_wasm.MethodData.md) variant consisting of the given `key`.

### Errors
An error is thrown if the given `key` contains any private components.

#### Parameters

| Name | Type |
| :------ | :------ |
| `key` | [`Jwk`](identity_wasm.Jwk.md) |

#### Returns

[`MethodData`](identity_wasm.MethodData.md)

___

### newCustom

▸ `Static` **newCustom**(`name`, `data`): [`MethodData`](identity_wasm.MethodData.md)

Creates a new custom [MethodData](identity_wasm.MethodData.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `name` | `string` |
| `data` | `any` |

#### Returns

[`MethodData`](identity_wasm.MethodData.md)

___

### tryCustom

▸ **tryCustom**(): [`CustomMethodData`](identity_wasm.CustomMethodData.md)

Returns the wrapped custom method data format is `Custom`.

#### Returns

[`CustomMethodData`](identity_wasm.CustomMethodData.md)

___

### tryDecode

▸ **tryDecode**(): `Uint8Array`

Returns a `Uint8Array` containing the decoded bytes of the [MethodData](identity_wasm.MethodData.md).

This is generally a public key identified by a [MethodData](identity_wasm.MethodData.md) value.

### Errors
Decoding can fail if [MethodData](identity_wasm.MethodData.md) has invalid content or cannot be
represented as a vector of bytes.

#### Returns

`Uint8Array`

___

### tryPublicKeyJwk

▸ **tryPublicKeyJwk**(): [`Jwk`](identity_wasm.Jwk.md)

Returns the wrapped [Jwk](identity_wasm.Jwk.md) if the format is `PublicKeyJwk`.

#### Returns

[`Jwk`](identity_wasm.Jwk.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`MethodData`](identity_wasm.MethodData.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`MethodData`](identity_wasm.MethodData.md)

___

### clone

▸ **clone**(): [`MethodData`](identity_wasm.MethodData.md)

Deep clones the object.

#### Returns

[`MethodData`](identity_wasm.MethodData.md)
