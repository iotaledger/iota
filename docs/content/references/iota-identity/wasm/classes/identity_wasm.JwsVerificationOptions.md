# Class: JwsVerificationOptions

[identity\_wasm](../modules/identity_wasm.md).JwsVerificationOptions

## Table of contents

### Constructors

- [constructor](identity_wasm.JwsVerificationOptions.md#constructor)

### Methods

- [toJSON](identity_wasm.JwsVerificationOptions.md#tojson)
- [toString](identity_wasm.JwsVerificationOptions.md#tostring)
- [setNonce](identity_wasm.JwsVerificationOptions.md#setnonce)
- [setMethodScope](identity_wasm.JwsVerificationOptions.md#setmethodscope)
- [setMethodId](identity_wasm.JwsVerificationOptions.md#setmethodid)
- [fromJSON](identity_wasm.JwsVerificationOptions.md#fromjson)
- [clone](identity_wasm.JwsVerificationOptions.md#clone)

## Constructors

### constructor

• **new JwsVerificationOptions**(`options?`)

Creates a new [JwsVerificationOptions](identity_wasm.JwsVerificationOptions.md) from the given fields.

#### Parameters

| Name | Type |
| :------ | :------ |
| `options?` | [`IJwsVerificationOptions`](../interfaces/identity_wasm.IJwsVerificationOptions.md) |

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

### setNonce

▸ **setNonce**(`value`): `void`

Set the expected value for the `nonce` parameter of the protected header.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### setMethodScope

▸ **setMethodScope**(`value`): `void`

Set the scope of the verification methods that may be used to verify the given JWS.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | [`MethodScope`](identity_wasm.MethodScope.md) |

#### Returns

`void`

___

### setMethodId

▸ **setMethodId**(`value`): `void`

Set the DID URl of the method, whose JWK should be used to verify the JWS.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | [`DIDUrl`](identity_wasm.DIDUrl.md) |

#### Returns

`void`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`JwsVerificationOptions`](identity_wasm.JwsVerificationOptions.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`JwsVerificationOptions`](identity_wasm.JwsVerificationOptions.md)

___

### clone

▸ **clone**(): [`JwsVerificationOptions`](identity_wasm.JwsVerificationOptions.md)

Deep clones the object.

#### Returns

[`JwsVerificationOptions`](identity_wasm.JwsVerificationOptions.md)
