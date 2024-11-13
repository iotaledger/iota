# Class: SdObjectEncoder

[identity\_wasm](../modules/identity_wasm.md).SdObjectEncoder

Transforms a JSON object into an SD-JWT object by substituting selected values
with their corresponding disclosure digests.

Note: digests are created using the sha-256 algorithm.

## Table of contents

### Constructors

- [constructor](identity_wasm.SdObjectEncoder.md#constructor)

### Methods

- [toJSON](identity_wasm.SdObjectEncoder.md#tojson)
- [toString](identity_wasm.SdObjectEncoder.md#tostring)
- [conceal](identity_wasm.SdObjectEncoder.md#conceal)
- [addSdAlgProperty](identity_wasm.SdObjectEncoder.md#addsdalgproperty)
- [encodeToString](identity_wasm.SdObjectEncoder.md#encodetostring)
- [encodeToObject](identity_wasm.SdObjectEncoder.md#encodetoobject)
- [addDecoys](identity_wasm.SdObjectEncoder.md#adddecoys)

## Constructors

### constructor

• **new SdObjectEncoder**(`object`)

Creates a new `SdObjectEncoder` with `sha-256` hash function.

#### Parameters

| Name | Type |
| :------ | :------ |
| `object` | `any` |

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

▸ **toJSON**(): `any`

Returns the modified object.

#### Returns

`any`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

▸ **toString**(): `string`

Returns the modified object as a string.

#### Returns

`string`

___

### conceal

▸ **conceal**(`path`, `salt?`): [`Disclosure`](identity_wasm.Disclosure.md)

Substitutes a value with the digest of its disclosure.
If no salt is provided, the disclosure will be created with a random salt value.

`path` indicates the pointer to the value that will be concealed using the syntax of
[JSON pointer](https://datatracker.ietf.org/doc/html/rfc6901).

For the following object:

 ```
{
  "id": "did:value",
  "claim1": {
     "abc": true
  },
  "claim2": ["val_1", "val_2"]
}
```

Path "/id" conceals `"id": "did:value"`
Path "/claim1/abc" conceals `"abc": true`
Path "/claim2/0" conceals `val_1`

## Errors
* `InvalidPath` if pointer is invalid.
* `DataTypeMismatch` if existing SD format is invalid.

#### Parameters

| Name | Type |
| :------ | :------ |
| `path` | `string` |
| `salt?` | `string` |

#### Returns

[`Disclosure`](identity_wasm.Disclosure.md)

___

### addSdAlgProperty

▸ **addSdAlgProperty**(): `void`

Adds the `_sd_alg` property to the top level of the object, with
its value set to "sha-256".

#### Returns

`void`

___

### encodeToString

▸ **encodeToString**(): `string`

Returns the modified object as a string.

#### Returns

`string`

___

### encodeToObject

▸ **encodeToObject**(): `Record`\<`string`, `any`\>

Returns the modified object.

#### Returns

`Record`\<`string`, `any`\>

___

### addDecoys

▸ **addDecoys**(`path`, `number_of_decoys`): `void`

Adds a decoy digest to the specified path.
If path is an empty slice, decoys will be added to the top level.

#### Parameters

| Name | Type |
| :------ | :------ |
| `path` | `string` |
| `number_of_decoys` | `number` |

#### Returns

`void`
