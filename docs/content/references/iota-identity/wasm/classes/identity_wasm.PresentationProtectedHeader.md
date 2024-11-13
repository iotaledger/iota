# Class: PresentationProtectedHeader

[identity\_wasm](../modules/identity_wasm.md).PresentationProtectedHeader

## Table of contents

### Properties

- [alg](identity_wasm.PresentationProtectedHeader.md#alg)
- [aud](identity_wasm.PresentationProtectedHeader.md#aud)
- [kid](identity_wasm.PresentationProtectedHeader.md#kid)
- [nonce](identity_wasm.PresentationProtectedHeader.md#nonce)

### Methods

- [toJSON](identity_wasm.PresentationProtectedHeader.md#tojson)
- [toString](identity_wasm.PresentationProtectedHeader.md#tostring)

## Properties

### alg

• **alg**: [`PresentationProofAlgorithm`](../enums/identity_wasm.PresentationProofAlgorithm.md)

___

### aud

• `Optional` **aud**: `string`

Who have received the JPT.

___

### kid

• `Optional` **kid**: `string`

ID for the key used for the JWP.

___

### nonce

• `Optional` **nonce**: `string`

For replay attacks.

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
