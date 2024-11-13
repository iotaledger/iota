# Class: Payloads

[identity\_wasm](../modules/identity_wasm.md).Payloads

## Table of contents

### Constructors

- [constructor](identity_wasm.Payloads.md#constructor)

### Methods

- [toJSON](identity_wasm.Payloads.md#tojson)
- [toString](identity_wasm.Payloads.md#tostring)
- [fromJSON](identity_wasm.Payloads.md#fromjson)
- [clone](identity_wasm.Payloads.md#clone)
- [newFromValues](identity_wasm.Payloads.md#newfromvalues)
- [getValues](identity_wasm.Payloads.md#getvalues)
- [getUndisclosedIndexes](identity_wasm.Payloads.md#getundisclosedindexes)
- [getDisclosedIndexes](identity_wasm.Payloads.md#getdisclosedindexes)
- [getUndisclosedPayloads](identity_wasm.Payloads.md#getundisclosedpayloads)
- [getDisclosedPayloads](identity_wasm.Payloads.md#getdisclosedpayloads)
- [setUndisclosed](identity_wasm.Payloads.md#setundisclosed)
- [replacePayloadAtIndex](identity_wasm.Payloads.md#replacepayloadatindex)

## Constructors

### constructor

• **new Payloads**(`entries`)

#### Parameters

| Name | Type |
| :------ | :------ |
| `entries` | [`PayloadEntry`](identity_wasm.PayloadEntry.md)[] |

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

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Payloads`](identity_wasm.Payloads.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Payloads`](identity_wasm.Payloads.md)

___

### clone

▸ **clone**(): [`Payloads`](identity_wasm.Payloads.md)

Deep clones the object.

#### Returns

[`Payloads`](identity_wasm.Payloads.md)

___

### newFromValues

▸ `Static` **newFromValues**(`values`): [`Payloads`](identity_wasm.Payloads.md)

#### Parameters

| Name | Type |
| :------ | :------ |
| `values` | `any`[] |

#### Returns

[`Payloads`](identity_wasm.Payloads.md)

___

### getValues

▸ **getValues**(): `any`[]

#### Returns

`any`[]

___

### getUndisclosedIndexes

▸ **getUndisclosedIndexes**(): `Uint32Array`

#### Returns

`Uint32Array`

___

### getDisclosedIndexes

▸ **getDisclosedIndexes**(): `Uint32Array`

#### Returns

`Uint32Array`

___

### getUndisclosedPayloads

▸ **getUndisclosedPayloads**(): `any`[]

#### Returns

`any`[]

___

### getDisclosedPayloads

▸ **getDisclosedPayloads**(): [`Payloads`](identity_wasm.Payloads.md)

#### Returns

[`Payloads`](identity_wasm.Payloads.md)

___

### setUndisclosed

▸ **setUndisclosed**(`index`): `void`

#### Parameters

| Name | Type |
| :------ | :------ |
| `index` | `number` |

#### Returns

`void`

___

### replacePayloadAtIndex

▸ **replacePayloadAtIndex**(`index`, `value`): `any`

#### Parameters

| Name | Type |
| :------ | :------ |
| `index` | `number` |
| `value` | `any` |

#### Returns

`any`
