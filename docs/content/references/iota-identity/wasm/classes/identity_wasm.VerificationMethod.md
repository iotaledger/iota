# Class: VerificationMethod

[identity\_wasm](../modules/identity_wasm.md).VerificationMethod

A DID Document Verification Method.

## Table of contents

### Constructors

- [constructor](identity_wasm.VerificationMethod.md#constructor)

### Methods

- [toJSON](identity_wasm.VerificationMethod.md#tojson)
- [toString](identity_wasm.VerificationMethod.md#tostring)
- [newFromJwk](identity_wasm.VerificationMethod.md#newfromjwk)
- [id](identity_wasm.VerificationMethod.md#id)
- [setId](identity_wasm.VerificationMethod.md#setid)
- [controller](identity_wasm.VerificationMethod.md#controller)
- [setController](identity_wasm.VerificationMethod.md#setcontroller)
- [type](identity_wasm.VerificationMethod.md#type)
- [setType](identity_wasm.VerificationMethod.md#settype)
- [data](identity_wasm.VerificationMethod.md#data)
- [setData](identity_wasm.VerificationMethod.md#setdata)
- [properties](identity_wasm.VerificationMethod.md#properties)
- [setPropertyUnchecked](identity_wasm.VerificationMethod.md#setpropertyunchecked)
- [fromJSON](identity_wasm.VerificationMethod.md#fromjson)
- [clone](identity_wasm.VerificationMethod.md#clone)

## Constructors

### constructor

• **new VerificationMethod**(`id`, `controller`, `type_`, `data`)

Create a custom [VerificationMethod](identity_wasm.VerificationMethod.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `id` | [`DIDUrl`](identity_wasm.DIDUrl.md) |
| `controller` | [`CoreDID`](identity_wasm.CoreDID.md) |
| `type_` | [`MethodType`](identity_wasm.MethodType.md) |
| `data` | [`MethodData`](identity_wasm.MethodData.md) |

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

### newFromJwk

▸ `Static` **newFromJwk**(`did`, `key`, `fragment?`): [`VerificationMethod`](identity_wasm.VerificationMethod.md)

Creates a new [VerificationMethod](identity_wasm.VerificationMethod.md) from the given `did` and [Jwk](identity_wasm.Jwk.md). If `fragment` is not given
the `kid` value of the given `key` will be used, if present, otherwise an error is returned.

### Recommendations
The following recommendations are essentially taken from the `publicKeyJwk` description from the [DID specification](https://www.w3.org/TR/did-core/#dfn-publickeyjwk):
- It is recommended that verification methods that use `Jwks` to represent their public keys use the value of
  `kid` as their fragment identifier. This is
done automatically if `None` is passed in as the fragment.
- It is recommended that [Jwk](identity_wasm.Jwk.md) kid values are set to the public key fingerprint.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | `IToCoreDID` \| [`CoreDID`](identity_wasm.CoreDID.md) |
| `key` | [`Jwk`](identity_wasm.Jwk.md) |
| `fragment?` | `string` |

#### Returns

[`VerificationMethod`](identity_wasm.VerificationMethod.md)

___

### id

▸ **id**(): [`DIDUrl`](identity_wasm.DIDUrl.md)

Returns a copy of the [DIDUrl](identity_wasm.DIDUrl.md) of the [VerificationMethod](identity_wasm.VerificationMethod.md)'s `id`.

#### Returns

[`DIDUrl`](identity_wasm.DIDUrl.md)

___

### setId

▸ **setId**(`id`): `void`

Sets the id of the [VerificationMethod](identity_wasm.VerificationMethod.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `id` | [`DIDUrl`](identity_wasm.DIDUrl.md) |

#### Returns

`void`

___

### controller

▸ **controller**(): [`CoreDID`](identity_wasm.CoreDID.md)

Returns a copy of the `controller` `DID` of the [VerificationMethod](identity_wasm.VerificationMethod.md).

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### setController

▸ **setController**(`did`): `void`

Sets the `controller` `DID` of the [VerificationMethod](identity_wasm.VerificationMethod.md) object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | [`CoreDID`](identity_wasm.CoreDID.md) |

#### Returns

`void`

___

### type

▸ **type**(): [`MethodType`](identity_wasm.MethodType.md)

Returns a copy of the [VerificationMethod](identity_wasm.VerificationMethod.md) type.

#### Returns

[`MethodType`](identity_wasm.MethodType.md)

___

### setType

▸ **setType**(`type_`): `void`

Sets the [VerificationMethod](identity_wasm.VerificationMethod.md) type.

#### Parameters

| Name | Type |
| :------ | :------ |
| `type_` | [`MethodType`](identity_wasm.MethodType.md) |

#### Returns

`void`

___

### data

▸ **data**(): [`MethodData`](identity_wasm.MethodData.md)

Returns a copy of the [VerificationMethod](identity_wasm.VerificationMethod.md) public key data.

#### Returns

[`MethodData`](identity_wasm.MethodData.md)

___

### setData

▸ **setData**(`data`): `void`

Sets [VerificationMethod](identity_wasm.VerificationMethod.md) public key data.

#### Parameters

| Name | Type |
| :------ | :------ |
| `data` | [`MethodData`](identity_wasm.MethodData.md) |

#### Returns

`void`

___

### properties

▸ **properties**(): `Map`\<`string`, `any`\>

Get custom properties of the Verification Method.

#### Returns

`Map`\<`string`, `any`\>

___

### setPropertyUnchecked

▸ **setPropertyUnchecked**(`key`, `value`): `void`

Adds a custom property to the Verification Method.
If the value is set to `null`, the custom property will be removed.

### WARNING
This method can overwrite existing properties like `id` and result
in an invalid Verification Method.

#### Parameters

| Name | Type |
| :------ | :------ |
| `key` | `string` |
| `value` | `any` |

#### Returns

`void`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`VerificationMethod`](identity_wasm.VerificationMethod.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`VerificationMethod`](identity_wasm.VerificationMethod.md)

___

### clone

▸ **clone**(): [`VerificationMethod`](identity_wasm.VerificationMethod.md)

Deep clones the object.

#### Returns

[`VerificationMethod`](identity_wasm.VerificationMethod.md)
