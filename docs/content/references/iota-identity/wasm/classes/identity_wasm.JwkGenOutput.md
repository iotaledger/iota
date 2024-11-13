# Class: JwkGenOutput

[identity\_wasm](../modules/identity_wasm.md).JwkGenOutput

The result of a key generation in `JwkStorage`.

## Table of contents

### Constructors

- [constructor](identity_wasm.JwkGenOutput.md#constructor)

### Methods

- [toJSON](identity_wasm.JwkGenOutput.md#tojson)
- [toString](identity_wasm.JwkGenOutput.md#tostring)
- [jwk](identity_wasm.JwkGenOutput.md#jwk)
- [keyId](identity_wasm.JwkGenOutput.md#keyid)
- [fromJSON](identity_wasm.JwkGenOutput.md#fromjson)
- [clone](identity_wasm.JwkGenOutput.md#clone)

## Constructors

### constructor

• **new JwkGenOutput**(`key_id`, `jwk`)

#### Parameters

| Name | Type |
| :------ | :------ |
| `key_id` | `string` |
| `jwk` | [`Jwk`](identity_wasm.Jwk.md) |

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

### jwk

▸ **jwk**(): [`Jwk`](identity_wasm.Jwk.md)

Returns the generated public [Jwk](identity_wasm.Jwk.md).

#### Returns

[`Jwk`](identity_wasm.Jwk.md)

___

### keyId

▸ **keyId**(): `string`

Returns the key id of the generated [Jwk](identity_wasm.Jwk.md).

#### Returns

`string`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`JwkGenOutput`](identity_wasm.JwkGenOutput.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`JwkGenOutput`](identity_wasm.JwkGenOutput.md)

___

### clone

▸ **clone**(): [`JwkGenOutput`](identity_wasm.JwkGenOutput.md)

Deep clones the object.

#### Returns

[`JwkGenOutput`](identity_wasm.JwkGenOutput.md)
