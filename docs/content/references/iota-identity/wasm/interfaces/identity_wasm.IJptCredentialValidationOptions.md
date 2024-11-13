# Interface: IJptCredentialValidationOptions

[identity\_wasm](../modules/identity_wasm.md).IJptCredentialValidationOptions

Holds options to create a new [JptCredentialValidationOptions](../classes/identity_wasm.JptCredentialValidationOptions.md).

## Table of contents

### Properties

- [earliestExpiryDate](identity_wasm.IJptCredentialValidationOptions.md#earliestexpirydate)
- [latestIssuanceDate](identity_wasm.IJptCredentialValidationOptions.md#latestissuancedate)
- [status](identity_wasm.IJptCredentialValidationOptions.md#status)
- [subjectHolderRelationship](identity_wasm.IJptCredentialValidationOptions.md#subjectholderrelationship)
- [verificationOptions](identity_wasm.IJptCredentialValidationOptions.md#verificationoptions)

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

Validation behaviour for [`credentialStatus`](https://www.w3.org/TR/vc-data-model/#status).

___

### subjectHolderRelationship

• `Optional` `Readonly` **subjectHolderRelationship**: [`string`, [`SubjectHolderRelationship`](../enums/identity_wasm.SubjectHolderRelationship.md)]

Declares how credential subjects must relate to the presentation holder during validation.

<https://www.w3.org/TR/vc-data-model/#subject-holder-relationships>

___

### verificationOptions

• `Optional` `Readonly` **verificationOptions**: [`JwpVerificationOptions`](../classes/identity_wasm.JwpVerificationOptions.md)

Options which affect the verification of the proof on the credential.
