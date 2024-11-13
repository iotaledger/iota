# Class: JwsSignatureOptions

[identity\_wasm](../modules/identity_wasm.md).JwsSignatureOptions

## Table of contents

### Constructors

- [constructor](identity_wasm.JwsSignatureOptions.md#constructor)

### Methods

- [toJSON](identity_wasm.JwsSignatureOptions.md#tojson)
- [toString](identity_wasm.JwsSignatureOptions.md#tostring)
- [setAttachJwk](identity_wasm.JwsSignatureOptions.md#setattachjwk)
- [setB64](identity_wasm.JwsSignatureOptions.md#setb64)
- [setTyp](identity_wasm.JwsSignatureOptions.md#settyp)
- [setCty](identity_wasm.JwsSignatureOptions.md#setcty)
- [serUrl](identity_wasm.JwsSignatureOptions.md#serurl)
- [setNonce](identity_wasm.JwsSignatureOptions.md#setnonce)
- [setKid](identity_wasm.JwsSignatureOptions.md#setkid)
- [setDetachedPayload](identity_wasm.JwsSignatureOptions.md#setdetachedpayload)
- [setCustomHeaderParameters](identity_wasm.JwsSignatureOptions.md#setcustomheaderparameters)
- [fromJSON](identity_wasm.JwsSignatureOptions.md#fromjson)
- [clone](identity_wasm.JwsSignatureOptions.md#clone)

## Constructors

### constructor

• **new JwsSignatureOptions**(`options?`)

#### Parameters

| Name | Type |
| :------ | :------ |
| `options?` | [`IJwsSignatureOptions`](../interfaces/identity_wasm.IJwsSignatureOptions.md) |

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

### setAttachJwk

▸ **setAttachJwk**(`value`): `void`

Replace the value of the `attachJwk` field.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `boolean` |

#### Returns

`void`

___

### setB64

▸ **setB64**(`value`): `void`

Replace the value of the `b64` field.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `boolean` |

#### Returns

`void`

___

### setTyp

▸ **setTyp**(`value`): `void`

Replace the value of the `typ` field.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### setCty

▸ **setCty**(`value`): `void`

Replace the value of the `cty` field.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### serUrl

▸ **serUrl**(`value`): `void`

Replace the value of the `url` field.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### setNonce

▸ **setNonce**(`value`): `void`

Replace the value of the `nonce` field.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### setKid

▸ **setKid**(`value`): `void`

Replace the value of the `kid` field.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### setDetachedPayload

▸ **setDetachedPayload**(`value`): `void`

Replace the value of the `detached_payload` field.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `boolean` |

#### Returns

`void`

___

### setCustomHeaderParameters

▸ **setCustomHeaderParameters**(`value`): `void`

Add additional header parameters.

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `Record`\<`string`, `any`\> |

#### Returns

`void`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`JwsSignatureOptions`](identity_wasm.JwsSignatureOptions.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`JwsSignatureOptions`](identity_wasm.JwsSignatureOptions.md)

___

### clone

▸ **clone**(): [`JwsSignatureOptions`](identity_wasm.JwsSignatureOptions.md)

Deep clones the object.

#### Returns

[`JwsSignatureOptions`](identity_wasm.JwsSignatureOptions.md)
