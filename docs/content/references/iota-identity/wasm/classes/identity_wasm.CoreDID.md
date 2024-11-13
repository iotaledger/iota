# Class: CoreDID

[identity\_wasm](../modules/identity_wasm.md).CoreDID

A method-agnostic Decentralized Identifier (DID).

## Table of contents

### Methods

- [toJSON](identity_wasm.CoreDID.md#tojson)
- [toString](identity_wasm.CoreDID.md#tostring)
- [parse](identity_wasm.CoreDID.md#parse)
- [setMethodName](identity_wasm.CoreDID.md#setmethodname)
- [validMethodName](identity_wasm.CoreDID.md#validmethodname)
- [setMethodId](identity_wasm.CoreDID.md#setmethodid)
- [validMethodId](identity_wasm.CoreDID.md#validmethodid)
- [scheme](identity_wasm.CoreDID.md#scheme)
- [authority](identity_wasm.CoreDID.md#authority)
- [method](identity_wasm.CoreDID.md#method)
- [methodId](identity_wasm.CoreDID.md#methodid)
- [join](identity_wasm.CoreDID.md#join)
- [toUrl](identity_wasm.CoreDID.md#tourl)
- [intoUrl](identity_wasm.CoreDID.md#intourl)
- [fromJSON](identity_wasm.CoreDID.md#fromjson)
- [clone](identity_wasm.CoreDID.md#clone)

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

Returns the [CoreDID](identity_wasm.CoreDID.md) as a string.

#### Returns

`string`

___

### parse

▸ `Static` **parse**(`input`): [`CoreDID`](identity_wasm.CoreDID.md)

Parses a [CoreDID](identity_wasm.CoreDID.md) from the given `input`.

### Errors

Throws an error if the input is not a valid [CoreDID](identity_wasm.CoreDID.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `input` | `string` |

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### setMethodName

▸ **setMethodName**(`value`): `void`

Set the method name of the [CoreDID](identity_wasm.CoreDID.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### validMethodName

▸ `Static` **validMethodName**(`value`): `boolean`

Validates whether a string is a valid DID method name.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`boolean`

___

### setMethodId

▸ **setMethodId**(`value`): `void`

Set the method-specific-id of the `DID`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### validMethodId

▸ `Static` **validMethodId**(`value`): `boolean`

Validates whether a string is a valid `DID` method-id.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`boolean`

___

### scheme

▸ **scheme**(): `string`

Returns the [CoreDID](identity_wasm.CoreDID.md) scheme.

E.g.
- `"did:example:12345678" -> "did"`
- `"did:iota:smr:12345678" -> "did"`

#### Returns

`string`

___

### authority

▸ **authority**(): `string`

Returns the [CoreDID](identity_wasm.CoreDID.md) authority: the method name and method-id.

E.g.
- `"did:example:12345678" -> "example:12345678"`
- `"did:iota:smr:12345678" -> "iota:smr:12345678"`

#### Returns

`string`

___

### method

▸ **method**(): `string`

Returns the [CoreDID](identity_wasm.CoreDID.md) method name.

E.g.
- `"did:example:12345678" -> "example"`
- `"did:iota:smr:12345678" -> "iota"`

#### Returns

`string`

___

### methodId

▸ **methodId**(): `string`

Returns the [CoreDID](identity_wasm.CoreDID.md) method-specific ID.

E.g.
- `"did:example:12345678" -> "12345678"`
- `"did:iota:smr:12345678" -> "smr:12345678"`

#### Returns

`string`

___

### join

▸ **join**(`segment`): [`DIDUrl`](identity_wasm.DIDUrl.md)

Construct a new [DIDUrl](identity_wasm.DIDUrl.md) by joining with a relative DID Url string.

#### Parameters

| Name | Type |
| :------ | :------ |
| `segment` | `string` |

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### toUrl

▸ **toUrl**(): [`DIDUrl`](identity_wasm.DIDUrl.md)

Clones the [CoreDID](identity_wasm.CoreDID.md) into a [DIDUrl](identity_wasm.DIDUrl.md).

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### intoUrl

▸ **intoUrl**(): [`DIDUrl`](identity_wasm.DIDUrl.md)

Converts the [CoreDID](identity_wasm.CoreDID.md) into a [DIDUrl](identity_wasm.DIDUrl.md), consuming it.

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`CoreDID`](identity_wasm.CoreDID.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### clone

▸ **clone**(): [`CoreDID`](identity_wasm.CoreDID.md)

Deep clones the object.

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)
