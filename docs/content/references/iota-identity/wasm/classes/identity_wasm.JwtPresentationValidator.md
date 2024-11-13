# Class: JwtPresentationValidator

[identity\_wasm](../modules/identity_wasm.md).JwtPresentationValidator

## Table of contents

### Constructors

- [constructor](identity_wasm.JwtPresentationValidator.md#constructor)

### Methods

- [toJSON](identity_wasm.JwtPresentationValidator.md#tojson)
- [toString](identity_wasm.JwtPresentationValidator.md#tostring)
- [validate](identity_wasm.JwtPresentationValidator.md#validate)
- [checkStructure](identity_wasm.JwtPresentationValidator.md#checkstructure)
- [extractHolder](identity_wasm.JwtPresentationValidator.md#extractholder)

## Constructors

### constructor

• **new JwtPresentationValidator**(`signatureVerifier?`)

Creates a new [JwtPresentationValidator](identity_wasm.JwtPresentationValidator.md). If a `signatureVerifier` is provided it will be used when
verifying decoded JWS signatures, otherwise a default verifier capable of handling the `EdDSA`, `ES256`, `ES256K`
algorithms will be used.

#### Parameters

| Name | Type |
| :------ | :------ |
| `signatureVerifier?` | [`IJwsVerifier`](../interfaces/identity_wasm.IJwsVerifier.md) |

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

___

### validate

▸ **validate**(`presentationJwt`, `holder`, `validation_options`): [`DecodedJwtPresentation`](identity_wasm.DecodedJwtPresentation.md)

Validates a [Presentation](identity_wasm.Presentation.md) encoded as a [Jwt](identity_wasm.Jwt.md).

The following properties are validated according to `options`:
- the JWT can be decoded into a semantically valid presentation.
- the expiration and issuance date contained in the JWT claims.
- the holder's signature.

Validation is done with respect to the properties set in `options`.

# Warning

* This method does NOT validate the constituent credentials and therefore also not the relationship between the
credentials' subjects and the presentation holder. This can be done with [JwtCredentialValidationOptions](identity_wasm.JwtCredentialValidationOptions.md).
* The lack of an error returned from this method is in of itself not enough to conclude that the presentation can
be trusted. This section contains more information on additional checks that should be carried out before and
after calling this method.

## The state of the supplied DID Documents.

The caller must ensure that the DID Documents in `holder` are up-to-date.

# Errors

An error is returned whenever a validated condition is not satisfied or when decoding fails.

#### Parameters

| Name | Type |
| :------ | :------ |
| `presentationJwt` | [`Jwt`](identity_wasm.Jwt.md) |
| `holder` | `IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md) |
| `validation_options` | [`JwtPresentationValidationOptions`](identity_wasm.JwtPresentationValidationOptions.md) |

#### Returns

[`DecodedJwtPresentation`](identity_wasm.DecodedJwtPresentation.md)

___

### checkStructure

▸ `Static` **checkStructure**(`presentation`): `void`

Validates the semantic structure of the [Presentation](identity_wasm.Presentation.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `presentation` | [`Presentation`](identity_wasm.Presentation.md) |

#### Returns

`void`

___

### extractHolder

▸ `Static` **extractHolder**(`presentation`): [`CoreDID`](identity_wasm.CoreDID.md)

Attempt to extract the holder of the presentation.

# Errors:
* If deserialization/decoding of the presentation fails.
* If the holder can't be parsed as DIDs.

#### Parameters

| Name | Type |
| :------ | :------ |
| `presentation` | [`Jwt`](identity_wasm.Jwt.md) |

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)
