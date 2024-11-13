# Class: MethodType

[identity\_wasm](../modules/identity_wasm.md).MethodType

Supported verification method types.

## Table of contents

### Methods

- [toJSON](identity_wasm.MethodType.md#tojson)
- [toString](identity_wasm.MethodType.md#tostring)
- [Ed25519VerificationKey2018](identity_wasm.MethodType.md#ed25519verificationkey2018)
- [X25519KeyAgreementKey2019](identity_wasm.MethodType.md#x25519keyagreementkey2019)
- [JsonWebKey](identity_wasm.MethodType.md#jsonwebkey)
- [JsonWebKey2020](identity_wasm.MethodType.md#jsonwebkey2020)
- [custom](identity_wasm.MethodType.md#custom)
- [fromJSON](identity_wasm.MethodType.md#fromjson)
- [clone](identity_wasm.MethodType.md#clone)

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

▸ **toString**(): `string`

Returns the [MethodType](identity_wasm.MethodType.md) as a string.

#### Returns

`string`

___

### Ed25519VerificationKey2018

▸ `Static` **Ed25519VerificationKey2018**(): [`MethodType`](identity_wasm.MethodType.md)

#### Returns

[`MethodType`](identity_wasm.MethodType.md)

___

### X25519KeyAgreementKey2019

▸ `Static` **X25519KeyAgreementKey2019**(): [`MethodType`](identity_wasm.MethodType.md)

#### Returns

[`MethodType`](identity_wasm.MethodType.md)

___

### JsonWebKey

▸ `Static` **JsonWebKey**(): [`MethodType`](identity_wasm.MethodType.md)

#### Returns

[`MethodType`](identity_wasm.MethodType.md)

**`Deprecated`**

Use [JsonWebKey2020](identity_wasm.MethodType.md#jsonwebkey2020) instead.

___

### JsonWebKey2020

▸ `Static` **JsonWebKey2020**(): [`MethodType`](identity_wasm.MethodType.md)

A verification method for use with JWT verification as prescribed by the [Jwk](identity_wasm.Jwk.md)
in the `publicKeyJwk` entry.

#### Returns

[`MethodType`](identity_wasm.MethodType.md)

___

### custom

▸ `Static` **custom**(`type_`): [`MethodType`](identity_wasm.MethodType.md)

A custom method.

#### Parameters

| Name | Type |
| :------ | :------ |
| `type_` | `string` |

#### Returns

[`MethodType`](identity_wasm.MethodType.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`MethodType`](identity_wasm.MethodType.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`MethodType`](identity_wasm.MethodType.md)

___

### clone

▸ **clone**(): [`MethodType`](identity_wasm.MethodType.md)

Deep clones the object.

#### Returns

[`MethodType`](identity_wasm.MethodType.md)
