# Interface: IJwtPresentationValidationOptions

[identity\_wasm](../modules/identity_wasm.md).IJwtPresentationValidationOptions

Holds options to create a new [JwtPresentationValidationOptions](../classes/identity_wasm.JwtPresentationValidationOptions.md).

## Table of contents

### Properties

- [presentationVerifierOptions](identity_wasm.IJwtPresentationValidationOptions.md#presentationverifieroptions)
- [earliestExpiryDate](identity_wasm.IJwtPresentationValidationOptions.md#earliestexpirydate)
- [latestIssuanceDate](identity_wasm.IJwtPresentationValidationOptions.md#latestissuancedate)

## Properties

### presentationVerifierOptions

• `Optional` `Readonly` **presentationVerifierOptions**: [`JwsVerificationOptions`](../classes/identity_wasm.JwsVerificationOptions.md)

Options which affect the verification of the signature on the presentation.

___

### earliestExpiryDate

• `Optional` `Readonly` **earliestExpiryDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

Declare that the presentation is **not** considered valid if it expires before this [Timestamp](../classes/identity_wasm.Timestamp.md).
Uses the current datetime during validation if not set.

___

### latestIssuanceDate

• `Optional` `Readonly` **latestIssuanceDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

Declare that the presentation is **not** considered valid if it was issued later than this [Timestamp](../classes/identity_wasm.Timestamp.md).
Uses the current datetime during validation if not set.
