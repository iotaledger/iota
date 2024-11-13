# Class: SelectiveDisclosurePresentation

[identity\_wasm](../modules/identity_wasm.md).SelectiveDisclosurePresentation

Used to construct a JwpPresentedBuilder and handle the selective disclosure of attributes
-

**`Context`**

MUST NOT be blinded
- id MUST be blinded
- type MUST NOT be blinded
- issuer MUST NOT be blinded
- issuanceDate MUST be blinded (if Timeframe Revocation mechanism is used)
- expirationDate MUST be blinded (if Timeframe Revocation mechanism is used)
- credentialSubject (User have to choose which attribute must be blinded)
- credentialSchema MUST NOT be blinded
- credentialStatus MUST NOT be blinded
- refreshService MUST NOT be blinded (probably will be used for Timeslot Revocation mechanism)
- termsOfUse NO reason to use it in ZK VC (will be in any case blinded)
- evidence (User have to choose which attribute must be blinded)

## Table of contents

### Constructors

- [constructor](identity_wasm.SelectiveDisclosurePresentation.md#constructor)

### Methods

- [concealInSubject](identity_wasm.SelectiveDisclosurePresentation.md#concealinsubject)
- [concealInEvidence](identity_wasm.SelectiveDisclosurePresentation.md#concealinevidence)
- [setPresentationHeader](identity_wasm.SelectiveDisclosurePresentation.md#setpresentationheader)

## Constructors

### constructor

• **new SelectiveDisclosurePresentation**(`issued_jwp`)

Initialize a presentation starting from an Issued JWP.
The properties `jti`, `nbf`, `issuanceDate`, `expirationDate` and `termsOfUse` are concealed by default.

#### Parameters

| Name | Type |
| :------ | :------ |
| `issued_jwp` | [`JwpIssued`](identity_wasm.JwpIssued.md) |

## Methods

### concealInSubject

▸ **concealInSubject**(`path`): `void`

Selectively disclose "credentialSubject" attributes.
# Example
```
{
    "id": 1234,
    "name": "Alice",
    "mainCourses": ["Object-oriented Programming", "Mathematics"],
    "degree": {
        "type": "BachelorDegree",
        "name": "Bachelor of Science and Arts",
    },
    "GPA": "4.0",
}
```
If you want to undisclose for example the Mathematics course and the name of the degree:
```
undisclose_subject("mainCourses[1]");
undisclose_subject("degree.name");
```

#### Parameters

| Name | Type |
| :------ | :------ |
| `path` | `string` |

#### Returns

`void`

___

### concealInEvidence

▸ **concealInEvidence**(`path`): `void`

Undiscloses "evidence" attributes.

#### Parameters

| Name | Type |
| :------ | :------ |
| `path` | `string` |

#### Returns

`void`

___

### setPresentationHeader

▸ **setPresentationHeader**(`header`): `void`

Sets presentation protected header.

#### Parameters

| Name | Type |
| :------ | :------ |
| `header` | [`PresentationProtectedHeader`](identity_wasm.PresentationProtectedHeader.md) |

#### Returns

`void`
