# Interface: IKeyBindingJWTValidationOptions

[identity\_wasm](../modules/identity_wasm.md).IKeyBindingJWTValidationOptions

Holds options to create a new `KeyBindingJWTValidationOptions`.

## Table of contents

### Properties

- [nonce](identity_wasm.IKeyBindingJWTValidationOptions.md#nonce)
- [aud](identity_wasm.IKeyBindingJWTValidationOptions.md#aud)
- [jwsOptions](identity_wasm.IKeyBindingJWTValidationOptions.md#jwsoptions)
- [earliestIssuanceDate](identity_wasm.IKeyBindingJWTValidationOptions.md#earliestissuancedate)
- [latestIssuanceDate](identity_wasm.IKeyBindingJWTValidationOptions.md#latestissuancedate)

## Properties

### nonce

• `Optional` `Readonly` **nonce**: `string`

Validates the nonce value of the KB-JWT claims.

___

### aud

• `Optional` `Readonly` **aud**: `string`

Validates the `aud` properties in the KB-JWT claims.

___

### jwsOptions

• `Readonly` **jwsOptions**: [`JwsVerificationOptions`](../classes/identity_wasm.JwsVerificationOptions.md)

Options which affect the verification of the signature on the KB-JWT.

___

### earliestIssuanceDate

• `Optional` `Readonly` **earliestIssuanceDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

Declares that the KB-JWT is considered invalid if the `iat` value in the claims
is earlier than this timestamp.

___

### latestIssuanceDate

• `Optional` `Readonly` **latestIssuanceDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

Declares that the KB-JWT is considered invalid if the `iat` value in the claims is
later than this timestamp.

Uses the current timestamp during validation if not set.
