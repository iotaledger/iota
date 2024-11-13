# Class: CustomMethodData

[identity\_wasm](../modules/identity_wasm.md).CustomMethodData

A custom verification method data format.

## Table of contents

### Constructors

- [constructor](identity_wasm.CustomMethodData.md#constructor)

### Methods

- [toJSON](identity_wasm.CustomMethodData.md#tojson)
- [toString](identity_wasm.CustomMethodData.md#tostring)
- [clone](identity_wasm.CustomMethodData.md#clone)
- [fromJSON](identity_wasm.CustomMethodData.md#fromjson)

## Constructors

### constructor

• **new CustomMethodData**(`name`, `data`)

#### Parameters

| Name | Type |
| :------ | :------ |
| `name` | `string` |
| `data` | `any` |

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

### clone

▸ **clone**(): [`CustomMethodData`](identity_wasm.CustomMethodData.md)

Deep clones the object.

#### Returns

[`CustomMethodData`](identity_wasm.CustomMethodData.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`CustomMethodData`](identity_wasm.CustomMethodData.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`CustomMethodData`](identity_wasm.CustomMethodData.md)
