# Class: MethodDigest

[identity\_wasm](../modules/identity_wasm.md).MethodDigest

Unique identifier of a [VerificationMethod](identity_wasm.VerificationMethod.md).

NOTE:
This class does not have a JSON representation,
use the methods `pack` and `unpack` instead.

## Table of contents

### Constructors

- [constructor](identity_wasm.MethodDigest.md#constructor)

### Methods

- [toJSON](identity_wasm.MethodDigest.md#tojson)
- [toString](identity_wasm.MethodDigest.md#tostring)
- [pack](identity_wasm.MethodDigest.md#pack)
- [unpack](identity_wasm.MethodDigest.md#unpack)
- [clone](identity_wasm.MethodDigest.md#clone)

## Constructors

### constructor

• **new MethodDigest**(`verification_method`)

#### Parameters

| Name | Type |
| :------ | :------ |
| `verification_method` | [`VerificationMethod`](identity_wasm.VerificationMethod.md) |

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

### pack

▸ **pack**(): `Uint8Array`

Packs [MethodDigest](identity_wasm.MethodDigest.md) into bytes.

#### Returns

`Uint8Array`

___

### unpack

▸ `Static` **unpack**(`bytes`): [`MethodDigest`](identity_wasm.MethodDigest.md)

Unpacks bytes into [MethodDigest](identity_wasm.MethodDigest.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `bytes` | `Uint8Array` |

#### Returns

[`MethodDigest`](identity_wasm.MethodDigest.md)

___

### clone

▸ **clone**(): [`MethodDigest`](identity_wasm.MethodDigest.md)

Deep clones the object.

#### Returns

[`MethodDigest`](identity_wasm.MethodDigest.md)
