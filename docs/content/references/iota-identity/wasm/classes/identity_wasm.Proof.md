# Class: Proof

[identity\_wasm](../modules/identity_wasm.md).Proof

Represents a cryptographic proof that can be used to validate verifiable credentials and
presentations.

This representation does not inherently implement any standard; instead, it
can be utilized to implement standards or user-defined proofs. The presence of the
`type` field is necessary to accommodate different types of cryptographic proofs.

Note that this proof is not related to JWT and can be used in combination or as an alternative
to it.

## Table of contents

### Constructors

- [constructor](identity_wasm.Proof.md#constructor)

### Methods

- [type](identity_wasm.Proof.md#type)
- [properties](identity_wasm.Proof.md#properties)
- [toJSON](identity_wasm.Proof.md#tojson)
- [fromJSON](identity_wasm.Proof.md#fromjson)
- [clone](identity_wasm.Proof.md#clone)

## Constructors

### constructor

• **new Proof**(`type_`, `properties`)

#### Parameters

| Name | Type |
| :------ | :------ |
| `type_` | `string` |
| `properties` | `any` |

## Methods

### type

▸ **type**(): `string`

Returns the type of proof.

#### Returns

`string`

___

### properties

▸ **properties**(): `any`

Returns the properties of the proof.

#### Returns

`any`

___

### toJSON

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Proof`](identity_wasm.Proof.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Proof`](identity_wasm.Proof.md)

___

### clone

▸ **clone**(): [`Proof`](identity_wasm.Proof.md)

Deep clones the object.

#### Returns

[`Proof`](identity_wasm.Proof.md)
