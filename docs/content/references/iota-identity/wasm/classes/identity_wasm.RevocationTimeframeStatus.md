# Class: RevocationTimeframeStatus

[identity\_wasm](../modules/identity_wasm.md).RevocationTimeframeStatus

Information used to determine the current status of a [Credential](identity_wasm.Credential.md).

## Table of contents

### Constructors

- [constructor](identity_wasm.RevocationTimeframeStatus.md#constructor)

### Methods

- [toJSON](identity_wasm.RevocationTimeframeStatus.md#tojson)
- [toString](identity_wasm.RevocationTimeframeStatus.md#tostring)
- [clone](identity_wasm.RevocationTimeframeStatus.md#clone)
- [fromJSON](identity_wasm.RevocationTimeframeStatus.md#fromjson)
- [startValidityTimeframe](identity_wasm.RevocationTimeframeStatus.md#startvaliditytimeframe)
- [endValidityTimeframe](identity_wasm.RevocationTimeframeStatus.md#endvaliditytimeframe)
- [id](identity_wasm.RevocationTimeframeStatus.md#id)
- [index](identity_wasm.RevocationTimeframeStatus.md#index)

## Constructors

### constructor

• **new RevocationTimeframeStatus**(`id`, `index`, `duration`, `start_validity?`)

Creates a new `RevocationTimeframeStatus`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `id` | `string` |
| `index` | `number` |
| `duration` | [`Duration`](identity_wasm.Duration.md) |
| `start_validity?` | [`Timestamp`](identity_wasm.Timestamp.md) |

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

▸ **clone**(): [`RevocationTimeframeStatus`](identity_wasm.RevocationTimeframeStatus.md)

Deep clones the object.

#### Returns

[`RevocationTimeframeStatus`](identity_wasm.RevocationTimeframeStatus.md)

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`RevocationTimeframeStatus`](identity_wasm.RevocationTimeframeStatus.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`RevocationTimeframeStatus`](identity_wasm.RevocationTimeframeStatus.md)

___

### startValidityTimeframe

▸ **startValidityTimeframe**(): [`Timestamp`](identity_wasm.Timestamp.md)

Get startValidityTimeframe value.

#### Returns

[`Timestamp`](identity_wasm.Timestamp.md)

___

### endValidityTimeframe

▸ **endValidityTimeframe**(): [`Timestamp`](identity_wasm.Timestamp.md)

Get endValidityTimeframe value.

#### Returns

[`Timestamp`](identity_wasm.Timestamp.md)

___

### id

▸ **id**(): `string`

Return the URL fo the `RevocationBitmapStatus`.

#### Returns

`string`

___

### index

▸ **index**(): `undefined` \| `number`

Return the index of the credential in the issuer's revocation bitmap

#### Returns

`undefined` \| `number`
