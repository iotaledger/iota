# Class: DecodedJwtCredential

[identity\_wasm](../modules/identity_wasm.md).DecodedJwtCredential

A cryptographically verified and decoded Credential.

Note that having an instance of this type only means the JWS it was constructed from was verified.
It does not imply anything about a potentially present proof property on the credential itself.

## Table of contents

### Methods

- [credential](identity_wasm.DecodedJwtCredential.md#credential)
- [protectedHeader](identity_wasm.DecodedJwtCredential.md#protectedheader)
- [customClaims](identity_wasm.DecodedJwtCredential.md#customclaims)
- [intoCredential](identity_wasm.DecodedJwtCredential.md#intocredential)

## Methods

### credential

▸ **credential**(): [`Credential`](identity_wasm.Credential.md)

Returns a copy of the credential parsed to the [Verifiable Credentials Data model](https://www.w3.org/TR/vc-data-model/).

#### Returns

[`Credential`](identity_wasm.Credential.md)

___

### protectedHeader

▸ **protectedHeader**(): [`JwsHeader`](identity_wasm.JwsHeader.md)

Returns a copy of the protected header parsed from the decoded JWS.

#### Returns

[`JwsHeader`](identity_wasm.JwsHeader.md)

___

### customClaims

▸ **customClaims**(): `undefined` \| `Record`\<`string`, `any`\>

The custom claims parsed from the JWT.

#### Returns

`undefined` \| `Record`\<`string`, `any`\>

___

### intoCredential

▸ **intoCredential**(): [`Credential`](identity_wasm.Credential.md)

Consumes the object and returns the decoded credential.

### Warning

This destroys the [DecodedJwtCredential](identity_wasm.DecodedJwtCredential.md) object.

#### Returns

[`Credential`](identity_wasm.Credential.md)
