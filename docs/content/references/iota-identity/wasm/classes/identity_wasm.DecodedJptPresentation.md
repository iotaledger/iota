# Class: DecodedJptPresentation

[identity\_wasm](../modules/identity_wasm.md).DecodedJptPresentation

## Table of contents

### Methods

- [clone](identity_wasm.DecodedJptPresentation.md#clone)
- [credential](identity_wasm.DecodedJptPresentation.md#credential)
- [customClaims](identity_wasm.DecodedJptPresentation.md#customclaims)
- [aud](identity_wasm.DecodedJptPresentation.md#aud)

## Methods

### clone

▸ **clone**(): [`DecodedJptPresentation`](identity_wasm.DecodedJptPresentation.md)

Deep clones the object.

#### Returns

[`DecodedJptPresentation`](identity_wasm.DecodedJptPresentation.md)

___

### credential

▸ **credential**(): [`Credential`](identity_wasm.Credential.md)

Returns the [Credential](identity_wasm.Credential.md) embedded into this JPT.

#### Returns

[`Credential`](identity_wasm.Credential.md)

___

### customClaims

▸ **customClaims**(): `Map`\<`string`, `any`\>

Returns the custom claims parsed from the JPT.

#### Returns

`Map`\<`string`, `any`\>

___

### aud

▸ **aud**(): `undefined` \| `string`

Returns the `aud` property parsed from the JWT claims.

#### Returns

`undefined` \| `string`
