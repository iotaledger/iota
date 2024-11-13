# Class: DecodedJwtPresentation

[identity\_wasm](../modules/identity_wasm.md).DecodedJwtPresentation

A cryptographically verified and decoded presentation.

Note that having an instance of this type only means the JWS it was constructed from was verified.
It does not imply anything about a potentially present proof property on the presentation itself.

## Table of contents

### Methods

- [presentation](identity_wasm.DecodedJwtPresentation.md#presentation)
- [protectedHeader](identity_wasm.DecodedJwtPresentation.md#protectedheader)
- [intoPresentation](identity_wasm.DecodedJwtPresentation.md#intopresentation)
- [expirationDate](identity_wasm.DecodedJwtPresentation.md#expirationdate)
- [issuanceDate](identity_wasm.DecodedJwtPresentation.md#issuancedate)
- [audience](identity_wasm.DecodedJwtPresentation.md#audience)
- [customClaims](identity_wasm.DecodedJwtPresentation.md#customclaims)

## Methods

### presentation

▸ **presentation**(): [`Presentation`](identity_wasm.Presentation.md)

#### Returns

[`Presentation`](identity_wasm.Presentation.md)

___

### protectedHeader

▸ **protectedHeader**(): [`JwsHeader`](identity_wasm.JwsHeader.md)

Returns a copy of the protected header parsed from the decoded JWS.

#### Returns

[`JwsHeader`](identity_wasm.JwsHeader.md)

___

### intoPresentation

▸ **intoPresentation**(): [`Presentation`](identity_wasm.Presentation.md)

Consumes the object and returns the decoded presentation.

### Warning
This destroys the [DecodedJwtPresentation](identity_wasm.DecodedJwtPresentation.md) object.

#### Returns

[`Presentation`](identity_wasm.Presentation.md)

___

### expirationDate

▸ **expirationDate**(): `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

The expiration date parsed from the JWT claims.

#### Returns

`undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

___

### issuanceDate

▸ **issuanceDate**(): `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

The issuance date parsed from the JWT claims.

#### Returns

`undefined` \| [`Timestamp`](identity_wasm.Timestamp.md)

___

### audience

▸ **audience**(): `undefined` \| `string`

The `aud` property parsed from JWT claims.

#### Returns

`undefined` \| `string`

___

### customClaims

▸ **customClaims**(): `undefined` \| `Record`\<`string`, `any`\>

The custom claims parsed from the JWT.

#### Returns

`undefined` \| `Record`\<`string`, `any`\>
