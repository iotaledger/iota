# Interface: IJwtCredentialValidationOptions

[identity\_wasm](../modules/identity_wasm.md).IJwtCredentialValidationOptions

Holds options to create a new [JwtCredentialValidationOptions](../classes/identity_wasm.JwtCredentialValidationOptions.md).

## Table of contents

### Properties

- [earliestExpiryDate](identity_wasm.IJwtCredentialValidationOptions.md#earliestexpirydate)
- [latestIssuanceDate](identity_wasm.IJwtCredentialValidationOptions.md#latestissuancedate)
- [status](identity_wasm.IJwtCredentialValidationOptions.md#status)
- [subjectHolderRelationship](identity_wasm.IJwtCredentialValidationOptions.md#subjectholderrelationship)
- [verifierOptions](identity_wasm.IJwtCredentialValidationOptions.md#verifieroptions)

## Properties

### earliestExpiryDate

• `Optional` `Readonly` **earliestExpiryDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

Declare that the credential is **not** considered valid if it expires before this [Timestamp](../classes/identity_wasm.Timestamp.md).
Uses the current datetime during validation if not set.

___

### latestIssuanceDate

• `Optional` `Readonly` **latestIssuanceDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

Declare that the credential is **not** considered valid if it was issued later than this [Timestamp](../classes/identity_wasm.Timestamp.md).
Uses the current datetime during validation if not set.

___

### status

• `Optional` `Readonly` **status**: [`StatusCheck`](../enums/identity_wasm.StatusCheck.md)

Validation behaviour for `credentialStatus`.

Default: `StatusCheck.Strict`.

___

### subjectHolderRelationship

• `Optional` `Readonly` **subjectHolderRelationship**: [`string`, [`SubjectHolderRelationship`](../enums/identity_wasm.SubjectHolderRelationship.md)]

Declares how credential subjects must relate to the presentation holder during validation.

<https://www.w3.org/TR/vc-data-model/#subject-holder-relationships>

___

### verifierOptions

• `Optional` `Readonly` **verifierOptions**: [`JwsVerificationOptions`](../classes/identity_wasm.JwsVerificationOptions.md)

Options which affect the verification of the signature on the credential.
