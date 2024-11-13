# Interface: IDomainLinkageCredential

[identity\_wasm](../modules/identity_wasm.md).IDomainLinkageCredential

Fields to create a new Domain Linkage [Credential](../classes/identity_wasm.Credential.md).

## Table of contents

### Properties

- [issuer](identity_wasm.IDomainLinkageCredential.md#issuer)
- [issuanceDate](identity_wasm.IDomainLinkageCredential.md#issuancedate)
- [expirationDate](identity_wasm.IDomainLinkageCredential.md#expirationdate)
- [origin](identity_wasm.IDomainLinkageCredential.md#origin)

## Properties

### issuer

• `Readonly` **issuer**: [`CoreDID`](../classes/identity_wasm.CoreDID.md) \| [`IotaDID`](../classes/identity_wasm.IotaDID.md)

A reference to the issuer of the [Credential](../classes/identity_wasm.Credential.md).

___

### issuanceDate

• `Optional` `Readonly` **issuanceDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

A timestamp of when the [Credential](../classes/identity_wasm.Credential.md) becomes valid. Defaults to the current datetime.

___

### expirationDate

• `Readonly` **expirationDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

A timestamp of when the [Credential](../classes/identity_wasm.Credential.md) should no longer be considered valid.

___

### origin

• `Readonly` **origin**: `string`

The origin, on which the [Credential](../classes/identity_wasm.Credential.md) is issued.
