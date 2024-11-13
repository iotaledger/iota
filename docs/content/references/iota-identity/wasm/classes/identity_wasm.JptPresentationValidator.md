# Class: JptPresentationValidator

[identity\_wasm](../modules/identity_wasm.md).JptPresentationValidator

## Table of contents

### Methods

- [validate](identity_wasm.JptPresentationValidator.md#validate)

## Methods

### validate

▸ `Static` **validate**(`presentation_jpt`, `issuer`, `options`, `fail_fast`): [`DecodedJptPresentation`](identity_wasm.DecodedJptPresentation.md)

Decodes and validates a Presented [Credential](identity_wasm.Credential.md) issued as a JPT (JWP Presented Form). A
[DecodedJptPresentation](identity_wasm.DecodedJptPresentation.md) is returned upon success.

The following properties are validated according to `options`:
- the holder's proof on the JWP,
- the expiration date,
- the issuance date,
- the semantic structure.

#### Parameters

| Name | Type |
| :------ | :------ |
| `presentation_jpt` | [`Jpt`](identity_wasm.Jpt.md) |
| `issuer` | `IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md) |
| `options` | [`JptPresentationValidationOptions`](identity_wasm.JptPresentationValidationOptions.md) |
| `fail_fast` | [`FailFast`](../enums/identity_wasm.FailFast.md) |

#### Returns

[`DecodedJptPresentation`](identity_wasm.DecodedJptPresentation.md)
