# Class: Disclosure

[identity\_wasm](../modules/identity_wasm.md).Disclosure

Represents an elements constructing a disclosure.
Object properties and array elements disclosures are supported.

See: https://www.ietf.org/archive/id/draft-ietf-oauth-selective-disclosure-jwt-07.html#name-disclosures

## Table of contents

### Constructors

- [constructor](identity_wasm.Disclosure.md#constructor)

### Methods

- [toJSON](identity_wasm.Disclosure.md#tojson)
- [toString](identity_wasm.Disclosure.md#tostring)
- [parse](identity_wasm.Disclosure.md#parse)
- [disclosure](identity_wasm.Disclosure.md#disclosure)
- [toEncodedString](identity_wasm.Disclosure.md#toencodedstring)
- [salt](identity_wasm.Disclosure.md#salt)
- [claimName](identity_wasm.Disclosure.md#claimname)
- [claimValue](identity_wasm.Disclosure.md#claimvalue)
- [fromJSON](identity_wasm.Disclosure.md#fromjson)

## Constructors

### constructor

• **new Disclosure**(`salt`, `claim_name`, `claim_value`)

#### Parameters

| Name | Type |
| :------ | :------ |
| `salt` | `string` |
| `claim_name` | `undefined` \| `string` |
| `claim_value` | `any` |

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

Returns a copy of the base64url-encoded string.

#### Returns

`string`

___

### parse

▸ `Static` **parse**(`disclosure`): [`Disclosure`](identity_wasm.Disclosure.md)

Parses a Base64 encoded disclosure into a `Disclosure`.

## Error

Returns an `InvalidDisclosure` if input is not a valid disclosure.

#### Parameters

| Name | Type |
| :------ | :------ |
| `disclosure` | `string` |

#### Returns

[`Disclosure`](identity_wasm.Disclosure.md)

___

### disclosure

▸ **disclosure**(): `string`

Returns a copy of the base64url-encoded string.

#### Returns

`string`

___

### toEncodedString

▸ **toEncodedString**(): `string`

Returns a copy of the base64url-encoded string.

#### Returns

`string`

___

### salt

▸ **salt**(): `string`

Returns a copy of the salt value.

#### Returns

`string`

___

### claimName

▸ **claimName**(): `undefined` \| `string`

Returns a copy of the claim name, optional for array elements.

#### Returns

`undefined` \| `string`

___

### claimValue

▸ **claimValue**(): `any`

Returns a copy of the claim Value which can be of any type.

#### Returns

`any`

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`Disclosure`](identity_wasm.Disclosure.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`Disclosure`](identity_wasm.Disclosure.md)
