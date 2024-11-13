# Interface: ICredential

[identity\_wasm](../modules/identity_wasm.md).ICredential

Fields for constructing a new [Credential](../classes/identity_wasm.Credential.md).

## Table of contents

### Properties

- [context](identity_wasm.ICredential.md#context)
- [id](identity_wasm.ICredential.md#id)
- [type](identity_wasm.ICredential.md#type)
- [credentialSubject](identity_wasm.ICredential.md#credentialsubject)
- [issuer](identity_wasm.ICredential.md#issuer)
- [issuanceDate](identity_wasm.ICredential.md#issuancedate)
- [expirationDate](identity_wasm.ICredential.md#expirationdate)
- [credentialStatus](identity_wasm.ICredential.md#credentialstatus)
- [credentialSchema](identity_wasm.ICredential.md#credentialschema)
- [refreshService](identity_wasm.ICredential.md#refreshservice)
- [termsOfUse](identity_wasm.ICredential.md#termsofuse)
- [evidence](identity_wasm.ICredential.md#evidence)
- [nonTransferable](identity_wasm.ICredential.md#nontransferable)

## Properties

### context

• `Optional` `Readonly` **context**: `string` \| `Record`\<`string`, `any`\> \| (`string` \| `Record`\<`string`, `any`\>)[]

The JSON-LD context(s) applicable to the [Credential](../classes/identity_wasm.Credential.md).

___

### id

• `Optional` `Readonly` **id**: `string`

A unique URI that may be used to identify the [Credential](../classes/identity_wasm.Credential.md).

___

### type

• `Optional` `Readonly` **type**: `string` \| `string`[]

One or more URIs defining the type of the [Credential](../classes/identity_wasm.Credential.md). Contains the base context by default.

___

### credentialSubject

• `Readonly` **credentialSubject**: [`Subject`](identity_wasm.Subject.md) \| [`Subject`](identity_wasm.Subject.md)[]

One or more objects representing the [Credential](../classes/identity_wasm.Credential.md) subject(s).

___

### issuer

• `Readonly` **issuer**: `string` \| [`CoreDID`](../classes/identity_wasm.CoreDID.md) \| [`IotaDID`](../classes/identity_wasm.IotaDID.md) \| [`Issuer`](identity_wasm.Issuer.md)

A reference to the issuer of the [Credential](../classes/identity_wasm.Credential.md).

___

### issuanceDate

• `Optional` `Readonly` **issuanceDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

A timestamp of when the [Credential](../classes/identity_wasm.Credential.md) becomes valid. Defaults to the current datetime.

___

### expirationDate

• `Optional` `Readonly` **expirationDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

A timestamp of when the [Credential](../classes/identity_wasm.Credential.md) should no longer be considered valid.

___

### credentialStatus

• `Optional` `Readonly` **credentialStatus**: [`Status`](identity_wasm.Status.md)

Information used to determine the current status of the [Credential](../classes/identity_wasm.Credential.md).

___

### credentialSchema

• `Optional` `Readonly` **credentialSchema**: [`Schema`](identity_wasm.Schema.md) \| [`Schema`](identity_wasm.Schema.md)[]

Information used to assist in the enforcement of a specific [Credential](../classes/identity_wasm.Credential.md) structure.

___

### refreshService

• `Optional` `Readonly` **refreshService**: [`RefreshService`](identity_wasm.RefreshService.md) \| [`RefreshService`](identity_wasm.RefreshService.md)[]

Service(s) used to refresh an expired [Credential](../classes/identity_wasm.Credential.md).

___

### termsOfUse

• `Optional` `Readonly` **termsOfUse**: [`Policy`](identity_wasm.Policy.md) \| [`Policy`](identity_wasm.Policy.md)[]

Terms-of-use specified by the [Credential](../classes/identity_wasm.Credential.md) issuer.

___

### evidence

• `Optional` `Readonly` **evidence**: [`Evidence`](identity_wasm.Evidence.md) \| [`Evidence`](identity_wasm.Evidence.md)[]

Human-readable evidence used to support the claims within the [Credential](../classes/identity_wasm.Credential.md).

___

### nonTransferable

• `Optional` `Readonly` **nonTransferable**: `boolean`

Indicates that the [Credential](../classes/identity_wasm.Credential.md) must only be contained within a [Presentation](../classes/identity_wasm.Presentation.md) with a proof issued from the [Credential](../classes/identity_wasm.Credential.md) subject.
