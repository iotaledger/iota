# Class: JwpPresentationOptions

[identity\_wasm](../modules/identity_wasm.md).JwpPresentationOptions

Options to be set in the JWT claims of a verifiable presentation.

## Table of contents

### Constructors

- [constructor](identity_wasm.JwpPresentationOptions.md#constructor)

### Properties

- [audience](identity_wasm.JwpPresentationOptions.md#audience)
- [nonce](identity_wasm.JwpPresentationOptions.md#nonce)

### Methods

- [toJSON](identity_wasm.JwpPresentationOptions.md#tojson)
- [toString](identity_wasm.JwpPresentationOptions.md#tostring)

## Constructors

### constructor

• **new JwpPresentationOptions**()

## Properties

### audience

• `Optional` **audience**: `string`

Sets the audience for presentation (`aud` property in JWP Presentation Header).

___

### nonce

• `Optional` **nonce**: `string`

The nonce to be placed in the Presentation Protected Header.

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
