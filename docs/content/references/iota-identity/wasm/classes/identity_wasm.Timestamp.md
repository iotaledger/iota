# Class: Timestamp

[identity\_wasm](../modules/identity_wasm.md).Timestamp

## Table of contents

### Constructors

- [constructor](identity_wasm.Timestamp.md#constructor)

### Methods

- [toJSON](identity_wasm.Timestamp.md#tojson)
- [toString](identity_wasm.Timestamp.md#tostring)
- [parse](identity_wasm.Timestamp.md#parse)
- [nowUTC](identity_wasm.Timestamp.md#nowutc)
- [toRFC3339](identity_wasm.Timestamp.md#torfc3339)
- [checkedAdd](identity_wasm.Timestamp.md#checkedadd)
- [checkedSub](identity_wasm.Timestamp.md#checkedsub)
- [fromJSON](identity_wasm.Timestamp.md#fromjson)

## Constructors

### constructor

• **new Timestamp**()

Creates a new [Timestamp](identity_wasm.Timestamp.md) with the current date and time.

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

### parse

▸ `Static` **parse**(`input`): [`Timestamp`](identity_wasm.Timestamp.md)

Parses a [Timestamp](identity_wasm.Timestamp.md) from the provided input string.

#### Parameters

| Name | Type |
| :------ | :------ |
| `input` | `string` |

#### Returns

[`Timestamp`](identity_wasm.Timestamp.md)

___

### nowUTC

▸ `Static` **nowUTC**(): [`Timestamp`](identity_wasm.Timestamp.md)

Creates a new [Timestamp](identity_wasm.Timestamp.md) with the current date and time.

#### Returns

[`Timestamp`](identity_wasm.Timestamp.md)

___

### toRFC3339

▸ **toRFC3339**(): `string`

Returns the [Timestamp](identity_wasm.Timestamp.md) as an RFC 3339 `String`.

#### Returns

`string`

___

### checkedAdd

▸ **checkedAdd**(`duration`): `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

Computes `self + duration`

Returns `null` if the operation leads to a timestamp not in the valid range for [RFC 3339](https://tools.ietf.org/html/rfc3339).

#### Parameters

| Name | Type |
| :------ | :------ |
| `duration` | [`Duration`](identity_wasm.Duration.md) |

#### Returns

`undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

___

### checkedSub

▸ **checkedSub**(`duration`): `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

Computes `self - duration`

Returns `null` if the operation leads to a timestamp not in the valid range for [RFC 3339](https://tools.ietf.org/html/rfc3339).

#### Parameters

| Name | Type |
| :------ | :------ |
| `duration` | [`Duration`](identity_wasm.Duration.md) |

#### Returns

`undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Timestamp`](identity_wasm.Timestamp.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Timestamp`](identity_wasm.Timestamp.md)
