# Class: JwtPresentationValidationOptions

[identity\_wasm](../modules/identity_wasm.md).JwtPresentationValidationOptions

Options to declare validation criteria when validating presentation.

## Table of contents

### Constructors

- [constructor](identity_wasm.JwtPresentationValidationOptions.md#constructor)

### Methods

- [toJSON](identity_wasm.JwtPresentationValidationOptions.md#tojson)
- [fromJSON](identity_wasm.JwtPresentationValidationOptions.md#fromjson)
- [clone](identity_wasm.JwtPresentationValidationOptions.md#clone)

## Constructors

### constructor

• **new JwtPresentationValidationOptions**(`options?`)

Creates a new [JwtPresentationValidationOptions](identity_wasm.JwtPresentationValidationOptions.md) from the given fields.

Throws an error if any of the options are invalid.

#### Parameters

| Name | Type |
| :------ | :------ |
| `options?` | [`IJwtPresentationValidationOptions`](../interfaces/identity_wasm.IJwtPresentationValidationOptions.md) |

## Methods

### toJSON

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`JwtPresentationValidationOptions`](identity_wasm.JwtPresentationValidationOptions.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`JwtPresentationValidationOptions`](identity_wasm.JwtPresentationValidationOptions.md)

___

### clone

▸ **clone**(): [`JwtPresentationValidationOptions`](identity_wasm.JwtPresentationValidationOptions.md)

Deep clones the object.

#### Returns

[`JwtPresentationValidationOptions`](identity_wasm.JwtPresentationValidationOptions.md)
