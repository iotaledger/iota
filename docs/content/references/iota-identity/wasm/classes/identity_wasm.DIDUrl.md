# Class: DIDUrl

[identity\_wasm](../modules/identity_wasm.md).DIDUrl

A method agnostic DID Url.

## Table of contents

### Methods

- [toJSON](identity_wasm.DIDUrl.md#tojson)
- [toString](identity_wasm.DIDUrl.md#tostring)
- [parse](identity_wasm.DIDUrl.md#parse)
- [did](identity_wasm.DIDUrl.md#did)
- [urlStr](identity_wasm.DIDUrl.md#urlstr)
- [fragment](identity_wasm.DIDUrl.md#fragment)
- [setFragment](identity_wasm.DIDUrl.md#setfragment)
- [path](identity_wasm.DIDUrl.md#path)
- [setPath](identity_wasm.DIDUrl.md#setpath)
- [query](identity_wasm.DIDUrl.md#query)
- [setQuery](identity_wasm.DIDUrl.md#setquery)
- [join](identity_wasm.DIDUrl.md#join)
- [fromJSON](identity_wasm.DIDUrl.md#fromjson)
- [clone](identity_wasm.DIDUrl.md#clone)

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

Returns the [DIDUrl](identity_wasm.DIDUrl.md) as a string.

#### Returns

`string`

___

### parse

▸ `Static` **parse**(`input`): [`DIDUrl`](identity_wasm.DIDUrl.md)

Parses a [DIDUrl](identity_wasm.DIDUrl.md) from the input string.

#### Parameters

| Name | Type |
| :------ | :------ |
| `input` | `string` |

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### did

▸ **did**(): [`CoreDID`](identity_wasm.CoreDID.md)

Return a copy of the [CoreDID](identity_wasm.CoreDID.md) section of the [DIDUrl](identity_wasm.DIDUrl.md).

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### urlStr

▸ **urlStr**(): `string`

Return a copy of the relative DID Url as a string, including only the path, query, and fragment.

#### Returns

`string`

___

### fragment

▸ **fragment**(): `undefined` \| `string`

Returns a copy of the [DIDUrl](identity_wasm.DIDUrl.md) method fragment, if any. Excludes the leading '#'.

#### Returns

`undefined` \| `string`

___

### setFragment

▸ **setFragment**(`value?`): `void`

Sets the `fragment` component of the [DIDUrl](identity_wasm.DIDUrl.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value?` | `string` |

#### Returns

`void`

___

### path

▸ **path**(): `undefined` \| `string`

Returns a copy of the [DIDUrl](identity_wasm.DIDUrl.md) path.

#### Returns

`undefined` \| `string`

___

### setPath

▸ **setPath**(`value?`): `void`

Sets the `path` component of the [DIDUrl](identity_wasm.DIDUrl.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value?` | `string` |

#### Returns

`void`

___

### query

▸ **query**(): `undefined` \| `string`

Returns a copy of the [DIDUrl](identity_wasm.DIDUrl.md) method query, if any. Excludes the leading '?'.

#### Returns

`undefined` \| `string`

___

### setQuery

▸ **setQuery**(`value?`): `void`

Sets the `query` component of the [DIDUrl](identity_wasm.DIDUrl.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value?` | `string` |

#### Returns

`void`

___

### join

▸ **join**(`segment`): [`DIDUrl`](identity_wasm.DIDUrl.md)

Append a string representing a path, query, and/or fragment, returning a new [DIDUrl](identity_wasm.DIDUrl.md).

Must begin with a valid delimiter character: '/', '?', '#'. Overwrites the existing URL
segment and any following segments in order of path, query, then fragment.

I.e.
- joining a path will clear the query and fragment.
- joining a query will clear the fragment.
- joining a fragment will only overwrite the fragment.

#### Parameters

| Name | Type |
| :------ | :------ |
| `segment` | `string` |

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`DIDUrl`](identity_wasm.DIDUrl.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### clone

▸ **clone**(): [`DIDUrl`](identity_wasm.DIDUrl.md)

Deep clones the object.

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)
