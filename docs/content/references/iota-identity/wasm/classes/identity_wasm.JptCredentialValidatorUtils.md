# Class: JptCredentialValidatorUtils

[identity\_wasm](../modules/identity_wasm.md).JptCredentialValidatorUtils

Utility functions for validating JPT credentials.

## Table of contents

### Constructors

- [constructor](identity_wasm.JptCredentialValidatorUtils.md#constructor)

### Methods

- [extractIssuer](identity_wasm.JptCredentialValidatorUtils.md#extractissuer)
- [extractIssuerFromIssuedJpt](identity_wasm.JptCredentialValidatorUtils.md#extractissuerfromissuedjpt)
- [checkTimeframesWithValidityTimeframe2024](identity_wasm.JptCredentialValidatorUtils.md#checktimeframeswithvaliditytimeframe2024)
- [checkRevocationWithValidityTimeframe2024](identity_wasm.JptCredentialValidatorUtils.md#checkrevocationwithvaliditytimeframe2024)
- [checkTimeframesAndRevocationWithValidityTimeframe2024](identity_wasm.JptCredentialValidatorUtils.md#checktimeframesandrevocationwithvaliditytimeframe2024)

## Constructors

### constructor

• **new JptCredentialValidatorUtils**()

## Methods

### extractIssuer

▸ `Static` **extractIssuer**(`credential`): [`CoreDID`](identity_wasm.CoreDID.md)

Utility for extracting the issuer field of a [Credential](identity_wasm.Credential.md) as a DID.
# Errors
Fails if the issuer field is not a valid DID.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### extractIssuerFromIssuedJpt

▸ `Static` **extractIssuerFromIssuedJpt**(`credential`): [`CoreDID`](identity_wasm.CoreDID.md)

Utility for extracting the issuer field of a credential in JPT representation as DID.
# Errors
If the JPT decoding fails or the issuer field is not a valid DID.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Jpt`](identity_wasm.Jpt.md) |

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)

___

### checkTimeframesWithValidityTimeframe2024

▸ `Static` **checkTimeframesWithValidityTimeframe2024**(`credential`, `validity_timeframe`, `status_check`): `void`

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `validity_timeframe` | `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md) |
| `status_check` | [`StatusCheck`](../enums/identity_wasm.StatusCheck.md) |

#### Returns

`void`

___

### checkRevocationWithValidityTimeframe2024

▸ `Static` **checkRevocationWithValidityTimeframe2024**(`credential`, `issuer`, `status_check`): `void`

Checks whether the credential status has been revoked.

Only supports `RevocationTimeframe2024`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `issuer` | `IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md) |
| `status_check` | [`StatusCheck`](../enums/identity_wasm.StatusCheck.md) |

#### Returns

`void`

___

### checkTimeframesAndRevocationWithValidityTimeframe2024

▸ `Static` **checkTimeframesAndRevocationWithValidityTimeframe2024**(`credential`, `issuer`, `validity_timeframe`, `status_check`): `void`

Checks whether the credential status has been revoked or the timeframe interval is INVALID

Only supports `RevocationTimeframe2024`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `credential` | [`Credential`](identity_wasm.Credential.md) |
| `issuer` | `IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md) |
| `validity_timeframe` | `undefined` \| [`Timestamp`](identity_wasm.Timestamp.md) |
| `status_check` | [`StatusCheck`](../enums/identity_wasm.StatusCheck.md) |

#### Returns

`void`
