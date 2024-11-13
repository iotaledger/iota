# Class: JptPresentationValidatorUtils

[identity\_wasm](../modules/identity_wasm.md).JptPresentationValidatorUtils

Utility functions for verifying JPT presentations.

## Table of contents

### Methods

- [extractIssuerFromPresentedJpt](identity_wasm.JptPresentationValidatorUtils.md#extractissuerfrompresentedjpt)
- [checkTimeframesWithValidityTimeframe2024](identity_wasm.JptPresentationValidatorUtils.md#checktimeframeswithvaliditytimeframe2024)

## Methods

### extractIssuerFromPresentedJpt

▸ `Static` **extractIssuerFromPresentedJpt**(`presentation`): [`CoreDID`](identity_wasm.CoreDID.md)

Utility for extracting the issuer field of a credential in JPT representation as DID.
# Errors
If the JPT decoding fails or the issuer field is not a valid DID.

#### Parameters

| Name | Type |
| :------ | :------ |
| `presentation` | [`Jpt`](identity_wasm.Jpt.md) |

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### checkTimeframesWithValidityTimeframe2024

▸ `Static` **checkTimeframesWithValidityTimeframe2024**(`credential`, `validity_timeframe`, `status_check`): `void`

Check timeframe interval in credentialStatus with `RevocationTimeframeStatus`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `validity_timeframe` | `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md) |
| `status_check` | [`StatusCheck`](../enums/identity_wasm.StatusCheck.md) |

#### Returns

`void`
