# Class: IotaDID

[identity\_wasm](../modules/identity_wasm.md).IotaDID

A DID conforming to the IOTA DID method specification.

**`Typicalname`**

did

## Table of contents

### Constructors

- [constructor](identity_wasm.IotaDID.md#constructor)

### Properties

- [DEFAULT\_NETWORK](identity_wasm.IotaDID.md#default_network)
- [METHOD](identity_wasm.IotaDID.md#method-1)

### Methods

- [toJSON](identity_wasm.IotaDID.md#tojson)
- [toString](identity_wasm.IotaDID.md#tostring)
- [fromAliasId](identity_wasm.IotaDID.md#fromaliasid)
- [placeholder](identity_wasm.IotaDID.md#placeholder)
- [parse](identity_wasm.IotaDID.md#parse)
- [network](identity_wasm.IotaDID.md#network)
- [tag](identity_wasm.IotaDID.md#tag)
- [toCoreDid](identity_wasm.IotaDID.md#tocoredid)
- [scheme](identity_wasm.IotaDID.md#scheme)
- [authority](identity_wasm.IotaDID.md#authority)
- [method](identity_wasm.IotaDID.md#method)
- [methodId](identity_wasm.IotaDID.md#methodid)
- [join](identity_wasm.IotaDID.md#join)
- [toUrl](identity_wasm.IotaDID.md#tourl)
- [toAliasId](identity_wasm.IotaDID.md#toaliasid)
- [intoUrl](identity_wasm.IotaDID.md#intourl)
- [fromJSON](identity_wasm.IotaDID.md#fromjson)
- [clone](identity_wasm.IotaDID.md#clone)

## Constructors

### constructor

• **new IotaDID**(`bytes`, `network`)

Constructs a new [IotaDID](identity_wasm.IotaDID.md) from a byte representation of the tag and the given
network name.

See also [placeholder](identity_wasm.IotaDID.md#placeholder).

#### Parameters

| Name | Type |
| :------ | :------ |
| `bytes` | `Uint8Array` |
| `network` | `string` |

## Properties

### DEFAULT\_NETWORK

▪ `Static` `Readonly` **DEFAULT\_NETWORK**: `string`

The default Tangle network (`"iota"`).

___

### METHOD

▪ `Static` `Readonly` **METHOD**: `string`

The IOTA DID method name (`"iota"`).

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

Returns the `DID` as a string.

#### Returns

`string`

___

### fromAliasId

▸ `Static` **fromAliasId**(`aliasId`, `network`): [`IotaDID`](identity_wasm.IotaDID.md)

Constructs a new [IotaDID](identity_wasm.IotaDID.md) from a hex representation of an Alias Id and the given
network name.

#### Parameters

| Name | Type |
| :------ | :------ |
| `aliasId` | `string` |
| `network` | `string` |

#### Returns

[`IotaDID`](identity_wasm.IotaDID.md)

___

### placeholder

▸ `Static` **placeholder**(`network`): [`IotaDID`](identity_wasm.IotaDID.md)

Creates a new placeholder [IotaDID](identity_wasm.IotaDID.md) with the given network name.

E.g. `did:iota:smr:0x0000000000000000000000000000000000000000000000000000000000000000`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `network` | `string` |

#### Returns

[`IotaDID`](identity_wasm.IotaDID.md)

___

### parse

▸ `Static` **parse**(`input`): [`IotaDID`](identity_wasm.IotaDID.md)

Parses a [IotaDID](identity_wasm.IotaDID.md) from the input string.

#### Parameters

| Name | Type |
| :------ | :------ |
| `input` | `string` |

#### Returns

[`IotaDID`](identity_wasm.IotaDID.md)

___

### network

▸ **network**(): `string`

Returns the Tangle network name of the [IotaDID](identity_wasm.IotaDID.md).

#### Returns

`string`

___

### tag

▸ **tag**(): `string`

Returns a copy of the unique tag of the [IotaDID](identity_wasm.IotaDID.md).

#### Returns

`string`

___

### toCoreDid

▸ **toCoreDid**(): [`CoreDID`](identity_wasm.CoreDID.md)

Returns the DID represented as a [CoreDID](identity_wasm.CoreDID.md).

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### scheme

▸ **scheme**(): `string`

Returns the `DID` scheme.

E.g.
- `"did:example:12345678" -> "did"`
- `"did:iota:main:12345678" -> "did"`

#### Returns

`string`

___

### authority

▸ **authority**(): `string`

Returns the `DID` authority: the method name and method-id.

E.g.
- `"did:example:12345678" -> "example:12345678"`
- `"did:iota:main:12345678" -> "iota:main:12345678"`

#### Returns

`string`

___

### method

▸ **method**(): `string`

Returns the `DID` method name.

E.g.
- `"did:example:12345678" -> "example"`
- `"did:iota:main:12345678" -> "iota"`

#### Returns

`string`

___

### methodId

▸ **methodId**(): `string`

Returns the `DID` method-specific ID.

E.g.
- `"did:example:12345678" -> "12345678"`
- `"did:iota:main:12345678" -> "main:12345678"`

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

Clones the `DID` into a [DIDUrl](identity_wasm.DIDUrl.md).

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### toAliasId

▸ **toAliasId**(): `string`

Returns the hex-encoded AliasId with a '0x' prefix, from the DID tag.

#### Returns

`string`

___

### intoUrl

▸ **intoUrl**(): [`DIDUrl`](identity_wasm.DIDUrl.md)

Converts the `DID` into a [DIDUrl](identity_wasm.DIDUrl.md), consuming it.

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`IotaDID`](identity_wasm.IotaDID.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`IotaDID`](identity_wasm.IotaDID.md)

___

### clone

▸ **clone**(): [`IotaDID`](identity_wasm.IotaDID.md)

Deep clones the object.

#### Returns

[`IotaDID`](identity_wasm.IotaDID.md)
