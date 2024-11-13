# Class: StatusList2021

[identity\_wasm](../modules/identity_wasm.md).StatusList2021

StatusList2021 data structure as described in [W3C's VC status list 2021](https://www.w3.org/TR/2023/WD-vc-status-list-20230427/).

## Table of contents

### Constructors

- [constructor](identity_wasm.StatusList2021.md#constructor)

### Methods

- [toJSON](identity_wasm.StatusList2021.md#tojson)
- [toString](identity_wasm.StatusList2021.md#tostring)
- [clone](identity_wasm.StatusList2021.md#clone)
- [len](identity_wasm.StatusList2021.md#len)
- [get](identity_wasm.StatusList2021.md#get)
- [set](identity_wasm.StatusList2021.md#set)
- [intoEncodedStr](identity_wasm.StatusList2021.md#intoencodedstr)
- [fromEncodedStr](identity_wasm.StatusList2021.md#fromencodedstr)

## Constructors

### constructor

• **new StatusList2021**(`size?`)

Creates a new [StatusList2021](identity_wasm.StatusList2021.md) of `size` entries.

#### Parameters

| Name | Type |
| :------ | :------ |
| `size?` | `number` |

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

___

### clone

▸ **clone**(): [`StatusList2021`](identity_wasm.StatusList2021.md)

Deep clones the object.

#### Returns

[`StatusList2021`](identity_wasm.StatusList2021.md)

___

### len

▸ **len**(): `number`

Returns the number of entries in this [StatusList2021](identity_wasm.StatusList2021.md).

#### Returns

`number`

___

### get

▸ **get**(`index`): `boolean`

Returns whether the entry at `index` is set.

#### Parameters

| Name | Type |
| :------ | :------ |
| `index` | `number` |

#### Returns

`boolean`

___

### set

▸ **set**(`index`, `value`): `void`

Sets the value of the `index`-th entry.

#### Parameters

| Name | Type |
| :------ | :------ |
| `index` | `number` |
| `value` | `boolean` |

#### Returns

`void`

___

### intoEncodedStr

▸ **intoEncodedStr**(): `string`

Encodes this [StatusList2021](identity_wasm.StatusList2021.md) into its compressed
base64 string representation.

#### Returns

`string`

___

### fromEncodedStr

▸ `Static` **fromEncodedStr**(`s`): [`StatusList2021`](identity_wasm.StatusList2021.md)

Attempts to decode a [StatusList2021](identity_wasm.StatusList2021.md) from a string.

#### Parameters

| Name | Type |
| :------ | :------ |
| `s` | `string` |

#### Returns

[`StatusList2021`](identity_wasm.StatusList2021.md)
