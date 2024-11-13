# Class: UnknownCredential

[identity\_wasm](../modules/identity_wasm.md).UnknownCredential

## Table of contents

### Methods

- [tryIntoJwt](identity_wasm.UnknownCredential.md#tryintojwt)
- [tryIntoCredential](identity_wasm.UnknownCredential.md#tryintocredential)
- [tryIntoRaw](identity_wasm.UnknownCredential.md#tryintoraw)
- [toJSON](identity_wasm.UnknownCredential.md#tojson)
- [fromJSON](identity_wasm.UnknownCredential.md#fromjson)
- [clone](identity_wasm.UnknownCredential.md#clone)

## Methods

### tryIntoJwt

▸ **tryIntoJwt**(): `undefined` \| [`Jwt`](identity_wasm.Jwt.md)

Returns a [Jwt](identity_wasm.Jwt.md) if the credential is of type string, `undefined` otherwise.

#### Returns

`undefined` \| [`Jwt`](identity_wasm.Jwt.md)

___

### tryIntoCredential

▸ **tryIntoCredential**(): `undefined` \| [`Credential`](identity_wasm.Credential.md)

Returns a [Credential](identity_wasm.Credential.md) if the credential is of said type, `undefined` otherwise.

#### Returns

`undefined` \| [`Credential`](identity_wasm.Credential.md)

___

### tryIntoRaw

▸ **tryIntoRaw**(): `undefined` \| `Record`\<`string`, `any`\>

Returns the contained value as an Object, if it can be converted, `undefined` otherwise.

#### Returns

`undefined` \| `Record`\<`string`, `any`\>

___

### toJSON

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`UnknownCredential`](identity_wasm.UnknownCredential.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`UnknownCredential`](identity_wasm.UnknownCredential.md)

___

### clone

▸ **clone**(): [`UnknownCredential`](identity_wasm.UnknownCredential.md)

Deep clones the object.

#### Returns

[`UnknownCredential`](identity_wasm.UnknownCredential.md)
