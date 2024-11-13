# Class: DecodedJptCredential

[identity\_wasm](../modules/identity_wasm.md).DecodedJptCredential

## Table of contents

### Methods

- [clone](identity_wasm.DecodedJptCredential.md#clone)
- [credential](identity_wasm.DecodedJptCredential.md#credential)
- [customClaims](identity_wasm.DecodedJptCredential.md#customclaims)
- [decodedJwp](identity_wasm.DecodedJptCredential.md#decodedjwp)

## Methods

### clone

▸ **clone**(): [`DecodedJptCredential`](identity_wasm.DecodedJptCredential.md)

Deep clones the object.

#### Returns

[`DecodedJptCredential`](identity_wasm.DecodedJptCredential.md)

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

### decodedJwp

▸ **decodedJwp**(): [`JwpIssued`](identity_wasm.JwpIssued.md)

#### Returns

[`JwpIssued`](identity_wasm.JwpIssued.md)
