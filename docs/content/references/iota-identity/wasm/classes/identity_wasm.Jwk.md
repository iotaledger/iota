# Class: Jwk

[identity\_wasm](../modules/identity_wasm.md).Jwk

## Table of contents

### Constructors

- [constructor](identity_wasm.Jwk.md#constructor)

### Methods

- [toJSON](identity_wasm.Jwk.md#tojson)
- [toString](identity_wasm.Jwk.md#tostring)
- [kty](identity_wasm.Jwk.md#kty)
- [use](identity_wasm.Jwk.md#use)
- [keyOps](identity_wasm.Jwk.md#keyops)
- [alg](identity_wasm.Jwk.md#alg)
- [kid](identity_wasm.Jwk.md#kid)
- [x5u](identity_wasm.Jwk.md#x5u)
- [x5c](identity_wasm.Jwk.md#x5c)
- [x5t](identity_wasm.Jwk.md#x5t)
- [x5t256](identity_wasm.Jwk.md#x5t256)
- [paramsEc](identity_wasm.Jwk.md#paramsec)
- [paramsOkp](identity_wasm.Jwk.md#paramsokp)
- [paramsOct](identity_wasm.Jwk.md#paramsoct)
- [paramsRsa](identity_wasm.Jwk.md#paramsrsa)
- [toPublic](identity_wasm.Jwk.md#topublic)
- [isPublic](identity_wasm.Jwk.md#ispublic)
- [isPrivate](identity_wasm.Jwk.md#isprivate)
- [fromJSON](identity_wasm.Jwk.md#fromjson)
- [clone](identity_wasm.Jwk.md#clone)

## Constructors

### constructor

• **new Jwk**(`jwk`)

#### Parameters

| Name | Type |
| :------ | :------ |
| `jwk` | `IJwkParams` |

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

### kty

▸ **kty**(): [`JwkType`](../enums/jose_jwk_type.JwkType.md)

Returns the value for the key type parameter (kty).

#### Returns

[`JwkType`](../enums/jose_jwk_type.JwkType.md)

___

### use

▸ **use**(): `undefined` \| [`JwkUse`](../enums/jose_jwk_use.JwkUse.md)

Returns the value for the use property (use).

#### Returns

`undefined` \| [`JwkUse`](../enums/jose_jwk_use.JwkUse.md)

___

### keyOps

▸ **keyOps**(): [`JwkOperation`](../enums/jose_jwk_operation.JwkOperation.md)[]

#### Returns

[`JwkOperation`](../enums/jose_jwk_operation.JwkOperation.md)[]

___

### alg

▸ **alg**(): `undefined` \| [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md)

Returns the value for the algorithm property (alg).

#### Returns

`undefined` \| [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md)

___

### kid

▸ **kid**(): `undefined` \| `string`

Returns the value of the key ID property (kid).

#### Returns

`undefined` \| `string`

___

### x5u

▸ **x5u**(): `undefined` \| `string`

Returns the value of the X.509 URL property (x5u).

#### Returns

`undefined` \| `string`

___

### x5c

▸ **x5c**(): `string`[]

Returns the value of the X.509 certificate chain property (x5c).

#### Returns

`string`[]

___

### x5t

▸ **x5t**(): `undefined` \| `string`

Returns the value of the X.509 certificate SHA-1 thumbprint property (x5t).

#### Returns

`undefined` \| `string`

___

### x5t256

▸ **x5t256**(): `undefined` \| `string`

Returns the value of the X.509 certificate SHA-256 thumbprint property (x5t#S256).

#### Returns

`undefined` \| `string`

___

### paramsEc

▸ **paramsEc**(): `undefined` \| [`JwkParamsEc`](../interfaces/identity_wasm.JwkParamsEc.md)

If this JWK is of kty EC, returns those parameters.

#### Returns

`undefined` \| [`JwkParamsEc`](../interfaces/identity_wasm.JwkParamsEc.md)

___

### paramsOkp

▸ **paramsOkp**(): `undefined` \| [`JwkParamsOkp`](../interfaces/identity_wasm.JwkParamsOkp.md)

If this JWK is of kty OKP, returns those parameters.

#### Returns

`undefined` \| [`JwkParamsOkp`](../interfaces/identity_wasm.JwkParamsOkp.md)

___

### paramsOct

▸ **paramsOct**(): `undefined` \| [`JwkParamsOct`](../interfaces/identity_wasm.JwkParamsOct.md)

If this JWK is of kty OCT, returns those parameters.

#### Returns

`undefined` \| [`JwkParamsOct`](../interfaces/identity_wasm.JwkParamsOct.md)

___

### paramsRsa

▸ **paramsRsa**(): `undefined` \| [`JwkParamsRsa`](../interfaces/identity_wasm.JwkParamsRsa.md)

If this JWK is of kty RSA, returns those parameters.

#### Returns

`undefined` \| [`JwkParamsRsa`](../interfaces/identity_wasm.JwkParamsRsa.md)

___

### toPublic

▸ **toPublic**(): `undefined` \| [`Jwk`](identity_wasm.Jwk.md)

Returns a clone of the [Jwk](identity_wasm.Jwk.md) with _all_ private key components unset.
Nothing is returned when `kty = oct` as this key type is not considered public by this library.

#### Returns

`undefined` \| [`Jwk`](identity_wasm.Jwk.md)

___

### isPublic

▸ **isPublic**(): `boolean`

Returns `true` if _all_ private key components of the key are unset, `false` otherwise.

#### Returns

`boolean`

___

### isPrivate

▸ **isPrivate**(): `boolean`

Returns `true` if _all_ private key components of the key are set, `false` otherwise.

#### Returns

`boolean`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Jwk`](identity_wasm.Jwk.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Jwk`](identity_wasm.Jwk.md)

___

### clone

▸ **clone**(): [`Jwk`](identity_wasm.Jwk.md)

Deep clones the object.

#### Returns

[`Jwk`](identity_wasm.Jwk.md)
