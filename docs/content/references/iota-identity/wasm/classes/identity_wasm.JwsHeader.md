# Class: JwsHeader

[identity\_wasm](../modules/identity_wasm.md).JwsHeader

## Table of contents

### Constructors

- [constructor](identity_wasm.JwsHeader.md#constructor)

### Methods

- [alg](identity_wasm.JwsHeader.md#alg)
- [setAlg](identity_wasm.JwsHeader.md#setalg)
- [b64](identity_wasm.JwsHeader.md#b64)
- [setB64](identity_wasm.JwsHeader.md#setb64)
- [custom](identity_wasm.JwsHeader.md#custom)
- [has](identity_wasm.JwsHeader.md#has)
- [isDisjoint](identity_wasm.JwsHeader.md#isdisjoint)
- [jku](identity_wasm.JwsHeader.md#jku)
- [setJku](identity_wasm.JwsHeader.md#setjku)
- [jwk](identity_wasm.JwsHeader.md#jwk)
- [setJwk](identity_wasm.JwsHeader.md#setjwk)
- [kid](identity_wasm.JwsHeader.md#kid)
- [setKid](identity_wasm.JwsHeader.md#setkid)
- [x5u](identity_wasm.JwsHeader.md#x5u)
- [setX5u](identity_wasm.JwsHeader.md#setx5u)
- [x5c](identity_wasm.JwsHeader.md#x5c)
- [setX5c](identity_wasm.JwsHeader.md#setx5c)
- [x5t](identity_wasm.JwsHeader.md#x5t)
- [setX5t](identity_wasm.JwsHeader.md#setx5t)
- [x5tS256](identity_wasm.JwsHeader.md#x5ts256)
- [setX5tS256](identity_wasm.JwsHeader.md#setx5ts256)
- [typ](identity_wasm.JwsHeader.md#typ)
- [setTyp](identity_wasm.JwsHeader.md#settyp)
- [cty](identity_wasm.JwsHeader.md#cty)
- [setCty](identity_wasm.JwsHeader.md#setcty)
- [crit](identity_wasm.JwsHeader.md#crit)
- [setCrit](identity_wasm.JwsHeader.md#setcrit)
- [url](identity_wasm.JwsHeader.md#url)
- [setUrl](identity_wasm.JwsHeader.md#seturl)
- [nonce](identity_wasm.JwsHeader.md#nonce)
- [setNonce](identity_wasm.JwsHeader.md#setnonce)
- [toJSON](identity_wasm.JwsHeader.md#tojson)
- [fromJSON](identity_wasm.JwsHeader.md#fromjson)
- [clone](identity_wasm.JwsHeader.md#clone)

## Constructors

### constructor

• **new JwsHeader**()

Create a new empty [JwsHeader](identity_wasm.JwsHeader.md).

## Methods

### alg

▸ **alg**(): `undefined` \| [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md)

Returns the value for the algorithm claim (alg).

#### Returns

`undefined` \| [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md)

___

### setAlg

▸ **setAlg**(`value`): `void`

Sets a value for the algorithm claim (alg).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | [`JwsAlgorithm`](../enums/jose_jws_algorithm.JwsAlgorithm.md) |

#### Returns

`void`

___

### b64

▸ **b64**(): `undefined` \| `boolean`

Returns the value of the base64url-encode payload claim (b64).

#### Returns

`undefined` \| `boolean`

___

### setB64

▸ **setB64**(`value`): `void`

Sets a value for the base64url-encode payload claim (b64).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `boolean` |

#### Returns

`void`

___

### custom

▸ **custom**(): `undefined` \| `Record`\<`string`, `any`\>

Additional header parameters.

#### Returns

`undefined` \| `Record`\<`string`, `any`\>

___

### has

▸ **has**(`claim`): `boolean`

#### Parameters

| Name | Type |
| :------ | :------ |
| `claim` | `string` |

#### Returns

`boolean`

___

### isDisjoint

▸ **isDisjoint**(`other`): `boolean`

Returns `true` if none of the fields are set in both `self` and `other`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `other` | [`JwsHeader`](identity_wasm.JwsHeader.md) |

#### Returns

`boolean`

___

### jku

▸ **jku**(): `undefined` \| `string`

Returns the value of the JWK Set URL claim (jku).

#### Returns

`undefined` \| `string`

___

### setJku

▸ **setJku**(`value`): `void`

Sets a value for the JWK Set URL claim (jku).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### jwk

▸ **jwk**(): `undefined` \| [`Jwk`](identity_wasm.Jwk.md)

Returns the value of the JWK claim (jwk).

#### Returns

`undefined` \| [`Jwk`](identity_wasm.Jwk.md)

___

### setJwk

▸ **setJwk**(`value`): `void`

Sets a value for the JWK claim (jwk).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | [`Jwk`](identity_wasm.Jwk.md) |

#### Returns

`void`

___

### kid

▸ **kid**(): `undefined` \| `string`

Returns the value of the key ID claim (kid).

#### Returns

`undefined` \| `string`

___

### setKid

▸ **setKid**(`value`): `void`

Sets a value for the key ID claim (kid).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### x5u

▸ **x5u**(): `undefined` \| `string`

Returns the value of the X.509 URL claim (x5u).

#### Returns

`undefined` \| `string`

___

### setX5u

▸ **setX5u**(`value`): `void`

Sets a value for the X.509 URL claim (x5u).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### x5c

▸ **x5c**(): `string`[]

Returns the value of the X.509 certificate chain claim (x5c).

#### Returns

`string`[]

___

### setX5c

▸ **setX5c**(`value`): `void`

Sets values for the X.509 certificate chain claim (x5c).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string`[] |

#### Returns

`void`

___

### x5t

▸ **x5t**(): `undefined` \| `string`

Returns the value of the X.509 certificate SHA-1 thumbprint claim (x5t).

#### Returns

`undefined` \| `string`

___

### setX5t

▸ **setX5t**(`value`): `void`

Sets a value for the X.509 certificate SHA-1 thumbprint claim (x5t).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### x5tS256

▸ **x5tS256**(): `undefined` \| `string`

Returns the value of the X.509 certificate SHA-256 thumbprint claim
(x5t#S256).

#### Returns

`undefined` \| `string`

___

### setX5tS256

▸ **setX5tS256**(`value`): `void`

Sets a value for the X.509 certificate SHA-256 thumbprint claim
(x5t#S256).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### typ

▸ **typ**(): `undefined` \| `string`

Returns the value of the token type claim (typ).

#### Returns

`undefined` \| `string`

___

### setTyp

▸ **setTyp**(`value`): `void`

Sets a value for the token type claim (typ).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### cty

▸ **cty**(): `undefined` \| `string`

Returns the value of the content type claim (cty).

#### Returns

`undefined` \| `string`

___

### setCty

▸ **setCty**(`value`): `void`

Sets a value for the content type claim (cty).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### crit

▸ **crit**(): `string`[]

Returns the value of the critical claim (crit).

#### Returns

`string`[]

___

### setCrit

▸ **setCrit**(`value`): `void`

Sets values for the critical claim (crit).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string`[] |

#### Returns

`void`

___

### url

▸ **url**(): `undefined` \| `string`

Returns the value of the url claim (url).

#### Returns

`undefined` \| `string`

___

### setUrl

▸ **setUrl**(`value`): `void`

Sets a value for the url claim (url).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### nonce

▸ **nonce**(): `undefined` \| `string`

Returns the value of the nonce claim (nonce).

#### Returns

`undefined` \| `string`

___

### setNonce

▸ **setNonce**(`value`): `void`

Sets a value for the nonce claim (nonce).

#### Parameters

| Name | Type |
| :------ | :------ |
| `value` | `string` |

#### Returns

`void`

___

### toJSON

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`JwsHeader`](identity_wasm.JwsHeader.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`JwsHeader`](identity_wasm.JwsHeader.md)

___

### clone

▸ **clone**(): [`JwsHeader`](identity_wasm.JwsHeader.md)

Deep clones the object.

#### Returns

[`JwsHeader`](identity_wasm.JwsHeader.md)
