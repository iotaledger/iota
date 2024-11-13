# Class: Jwt

[identity\_wasm](../modules/identity_wasm.md).Jwt

A wrapper around a JSON Web Token (JWK).

## Table of contents

### Constructors

- [constructor](identity_wasm.Jwt.md#constructor)

### Methods

- [toString](identity_wasm.Jwt.md#tostring)
- [toJSON](identity_wasm.Jwt.md#tojson)
- [fromJSON](identity_wasm.Jwt.md#fromjson)
- [clone](identity_wasm.Jwt.md#clone)

## Constructors

### constructor

• **new Jwt**(`jwt_string`)

Creates a new [Jwt](identity_wasm.Jwt.md) from the given string.

#### Parameters

| Name | Type |
| :------ | :------ |
| `jwt_string` | `string` |

## Methods

### toString

▸ **toString**(): `string`

Returns a clone of the JWT string.

#### Returns

`string`

___

### toJSON

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Jwt`](identity_wasm.Jwt.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Jwt`](identity_wasm.Jwt.md)

___

### clone

▸ **clone**(): [`Jwt`](identity_wasm.Jwt.md)

Deep clones the object.

#### Returns

[`Jwt`](identity_wasm.Jwt.md)
