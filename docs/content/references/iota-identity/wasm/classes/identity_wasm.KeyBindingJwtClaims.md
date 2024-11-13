# Class: KeyBindingJwtClaims

[identity\_wasm](../modules/identity_wasm.md).KeyBindingJwtClaims

Claims set for key binding JWT.

## Table of contents

### Constructors

- [constructor](identity_wasm.KeyBindingJwtClaims.md#constructor)

### Methods

- [toJSON](identity_wasm.KeyBindingJwtClaims.md#tojson)
- [toString](identity_wasm.KeyBindingJwtClaims.md#tostring)
- [iat](identity_wasm.KeyBindingJwtClaims.md#iat)
- [aud](identity_wasm.KeyBindingJwtClaims.md#aud)
- [nonce](identity_wasm.KeyBindingJwtClaims.md#nonce)
- [sdHash](identity_wasm.KeyBindingJwtClaims.md#sdhash)
- [customProperties](identity_wasm.KeyBindingJwtClaims.md#customproperties)
- [keyBindingJwtHeaderTyp](identity_wasm.KeyBindingJwtClaims.md#keybindingjwtheadertyp)
- [fromJSON](identity_wasm.KeyBindingJwtClaims.md#fromjson)
- [clone](identity_wasm.KeyBindingJwtClaims.md#clone)

## Constructors

### constructor

• **new KeyBindingJwtClaims**(`jwt`, `disclosures`, `nonce`, `aud`, `issued_at?`, `custom_properties?`)

Creates a new [`KeyBindingJwtClaims`].
When `issued_at` is left as None, it will automatically default to the current time.

# Error
When `issued_at` is set to `None` and the system returns time earlier than `SystemTime::UNIX_EPOCH`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `jwt` | `string` |
| `disclosures` | `string`[] |
| `nonce` | `string` |
| `aud` | `string` |
| `issued_at?` | [`Timestamp`](identity_wasm.Timestamp.md) |
| `custom_properties?` | `Record`\<`string`, `any`\> |

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

▸ **toString**(): `string`

Returns a string representation of the claims.

#### Returns

`string`

___

### iat

▸ **iat**(): `bigint`

Returns a copy of the issued at `iat` property.

#### Returns

`bigint`

___

### aud

▸ **aud**(): `string`

Returns a copy of the audience `aud` property.

#### Returns

`string`

___

### nonce

▸ **nonce**(): `string`

Returns a copy of the `nonce` property.

#### Returns

`string`

___

### sdHash

▸ **sdHash**(): `string`

Returns a copy of the `sd_hash` property.

#### Returns

`string`

___

### customProperties

▸ **customProperties**(): `Record`\<`string`, `any`\>

Returns a copy of the custom properties.

#### Returns

`Record`\<`string`, `any`\>

___

### keyBindingJwtHeaderTyp

▸ `Static` **keyBindingJwtHeaderTyp**(): `string`

Returns the value of the `typ` property of the JWT header according to
https://www.ietf.org/archive/id/draft-ietf-oauth-selective-disclosure-jwt-07.html#name-key-binding-jwt

#### Returns

`string`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`KeyBindingJwtClaims`](identity_wasm.KeyBindingJwtClaims.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`KeyBindingJwtClaims`](identity_wasm.KeyBindingJwtClaims.md)

___

### clone

▸ **clone**(): [`KeyBindingJwtClaims`](identity_wasm.KeyBindingJwtClaims.md)

Deep clones the object.

#### Returns

[`KeyBindingJwtClaims`](identity_wasm.KeyBindingJwtClaims.md)
