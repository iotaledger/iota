# Class: DIDJwk

[identity\_wasm](../modules/identity_wasm.md).DIDJwk

`did:jwk` DID.

## Table of contents

### Constructors

- [constructor](identity_wasm.DIDJwk.md#constructor)

### Methods

- [parse](identity_wasm.DIDJwk.md#parse)
- [jwk](identity_wasm.DIDJwk.md#jwk)
- [scheme](identity_wasm.DIDJwk.md#scheme)
- [authority](identity_wasm.DIDJwk.md#authority)
- [method](identity_wasm.DIDJwk.md#method)
- [methodId](identity_wasm.DIDJwk.md#methodid)
- [toString](identity_wasm.DIDJwk.md#tostring)
- [toJSON](identity_wasm.DIDJwk.md#tojson)
- [fromJSON](identity_wasm.DIDJwk.md#fromjson)
- [clone](identity_wasm.DIDJwk.md#clone)

## Constructors

### constructor

• **new DIDJwk**(`did`)

Creates a new [DIDJwk](identity_wasm.DIDJwk.md) from a [CoreDID](identity_wasm.CoreDID.md).

### Errors
Throws an error if the given did is not a valid `did:jwk` DID.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | `IToCoreDID` \| [`CoreDID`](identity_wasm.CoreDID.md) |

## Methods

### parse

▸ `Static` **parse**(`input`): [`DIDJwk`](identity_wasm.DIDJwk.md)

Parses a [DIDJwk](identity_wasm.DIDJwk.md) from the given `input`.

### Errors

Throws an error if the input is not a valid [DIDJwk](identity_wasm.DIDJwk.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `input` | `string` |

#### Returns

[`DIDJwk`](identity_wasm.DIDJwk.md)

___

### jwk

▸ **jwk**(): [`Jwk`](identity_wasm.Jwk.md)

Returns the JSON WEB KEY (JWK) encoded inside this `did:jwk`.

#### Returns

[`Jwk`](identity_wasm.Jwk.md)

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

### toString

▸ **toString**(): `string`

Returns the [CoreDID](identity_wasm.CoreDID.md) as a string.

#### Returns

`string`

___

### toJSON

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`DIDJwk`](identity_wasm.DIDJwk.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`DIDJwk`](identity_wasm.DIDJwk.md)

___

### clone

▸ **clone**(): [`DIDJwk`](identity_wasm.DIDJwk.md)

Deep clones the object.

#### Returns

[`DIDJwk`](identity_wasm.DIDJwk.md)
