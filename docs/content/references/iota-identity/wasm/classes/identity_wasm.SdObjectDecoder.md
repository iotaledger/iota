# Class: SdObjectDecoder

[identity\_wasm](../modules/identity_wasm.md).SdObjectDecoder

Substitutes digests in an SD-JWT object by their corresponding plaintext values provided by disclosures.

## Table of contents

### Constructors

- [constructor](identity_wasm.SdObjectDecoder.md#constructor)

### Methods

- [toJSON](identity_wasm.SdObjectDecoder.md#tojson)
- [toString](identity_wasm.SdObjectDecoder.md#tostring)
- [decode](identity_wasm.SdObjectDecoder.md#decode)

## Constructors

### constructor

• **new SdObjectDecoder**()

Creates a new `SdObjectDecoder` with `sha-256` hasher.

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

### decode

▸ **decode**(`object`, `disclosures`): `Record`\<`string`, `any`\>

Decodes an SD-JWT `object` containing by Substituting the digests with their corresponding
plaintext values provided by `disclosures`.

## Notes
* Claims like `exp` or `iat` are not validated in the process of decoding.
* `_sd_alg` property will be removed if present.

#### Parameters

| Name | Type |
| :------ | :------ |
| `object` | `Record`\<`string`, `any`\> |
| `disclosures` | `string`[] |

#### Returns

`Record`\<`string`, `any`\>
