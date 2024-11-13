# Class: SdJwt

[identity\_wasm](../modules/identity_wasm.md).SdJwt

Representation of an SD-JWT of the format
`<Issuer-signed JWT>~<Disclosure 1>~<Disclosure 2>~...~<Disclosure N>~<optional KB-JWT>`.

## Table of contents

### Constructors

- [constructor](identity_wasm.SdJwt.md#constructor)

### Methods

- [toJSON](identity_wasm.SdJwt.md#tojson)
- [toString](identity_wasm.SdJwt.md#tostring)
- [presentation](identity_wasm.SdJwt.md#presentation)
- [parse](identity_wasm.SdJwt.md#parse)
- [jwt](identity_wasm.SdJwt.md#jwt)
- [disclosures](identity_wasm.SdJwt.md#disclosures)
- [keyBindingJwt](identity_wasm.SdJwt.md#keybindingjwt)
- [clone](identity_wasm.SdJwt.md#clone)

## Constructors

### constructor

• **new SdJwt**(`jwt`, `disclosures`, `key_binding_jwt?`)

Creates a new `SdJwt` from its components.

#### Parameters

| Name | Type |
| :------ | :------ |
| `jwt` | `string` |
| `disclosures` | `string`[] |
| `key_binding_jwt?` | `string` |

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

▸ **toString**(): `string`

Serializes the components into the final SD-JWT.

#### Returns

`string`

___

### presentation

▸ **presentation**(): `string`

Serializes the components into the final SD-JWT.

#### Returns

`string`

___

### parse

▸ `Static` **parse**(`sd_jwt`): [`SdJwt`](identity_wasm.SdJwt.md)

Parses an SD-JWT into its components as [`SdJwt`].

## Error
Returns `DeserializationError` if parsing fails.

#### Parameters

| Name | Type |
| :------ | :------ |
| `sd_jwt` | `string` |

#### Returns

[`SdJwt`](identity_wasm.SdJwt.md)

___

### jwt

▸ **jwt**(): `string`

The JWT part.

#### Returns

`string`

___

### disclosures

▸ **disclosures**(): `string`[]

The disclosures part.

#### Returns

`string`[]

___

### keyBindingJwt

▸ **keyBindingJwt**(): `undefined` \| `string`

The optional key binding JWT.

#### Returns

`undefined` \| `string`

___

### clone

▸ **clone**(): [`SdJwt`](identity_wasm.SdJwt.md)

Deep clones the object.

#### Returns

[`SdJwt`](identity_wasm.SdJwt.md)
