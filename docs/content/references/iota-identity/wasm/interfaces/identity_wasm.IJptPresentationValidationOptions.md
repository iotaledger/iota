# Interface: IJptPresentationValidationOptions

[identity\_wasm](../modules/identity_wasm.md).IJptPresentationValidationOptions

Holds options to create a new [JptPresentationValidationOptions](../classes/identity_wasm.JptPresentationValidationOptions.md).

## Table of contents

### Properties

- [nonce](identity_wasm.IJptPresentationValidationOptions.md#nonce)
- [verificationOptions](identity_wasm.IJptPresentationValidationOptions.md#verificationoptions)

## Properties

### nonce

• `Optional` `Readonly` **nonce**: `string`

The nonce to be placed in the Presentation Protected Header.

___

### verificationOptions

• `Optional` `Readonly` **verificationOptions**: [`JwpVerificationOptions`](../classes/identity_wasm.JwpVerificationOptions.md)

Options which affect the verification of the proof on the credential.
