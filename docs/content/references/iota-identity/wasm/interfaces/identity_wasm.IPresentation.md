# Interface: IPresentation

[identity\_wasm](../modules/identity_wasm.md).IPresentation

Fields for constructing a new [Presentation](../classes/identity_wasm.Presentation.md).

## Table of contents

### Properties

- [context](identity_wasm.IPresentation.md#context)
- [id](identity_wasm.IPresentation.md#id)
- [type](identity_wasm.IPresentation.md#type)
- [verifiableCredential](identity_wasm.IPresentation.md#verifiablecredential)
- [holder](identity_wasm.IPresentation.md#holder)
- [refreshService](identity_wasm.IPresentation.md#refreshservice)
- [termsOfUse](identity_wasm.IPresentation.md#termsofuse)

## Properties

### context

• `Optional` `Readonly` **context**: `string` \| `Record`\<`string`, `any`\> \| (`string` \| `Record`\<`string`, `any`\>)[]

The JSON-LD context(s) applicable to the presentation.

___

### id

• `Optional` `Readonly` **id**: `string`

A unique URI that may be used to identify the presentation.

___

### type

• `Optional` `Readonly` **type**: `string` \| `string`[]

One or more URIs defining the type of the presentation. Contains the base context by default.

___

### verifiableCredential

• `Readonly` **verifiableCredential**: `Record`\<`string`, `any`\> \| [`Jwt`](../classes/identity_wasm.Jwt.md) \| [`Credential`](../classes/identity_wasm.Credential.md) \| (`Record`\<`string`, `any`\> \| [`Jwt`](../classes/identity_wasm.Jwt.md) \| [`Credential`](../classes/identity_wasm.Credential.md))[]

JWT Credential(s) expressing the claims of the presentation.

___

### holder

• `Readonly` **holder**: `string` \| [`CoreDID`](../classes/identity_wasm.CoreDID.md) \| [`IotaDID`](../classes/identity_wasm.IotaDID.md)

The entity that generated the presentation.

___

### refreshService

• `Optional` `Readonly` **refreshService**: [`RefreshService`](identity_wasm.RefreshService.md) \| [`RefreshService`](identity_wasm.RefreshService.md)[]

Service(s) used to refresh an expired [Credential](../classes/identity_wasm.Credential.md) in the presentation.

___

### termsOfUse

• `Optional` `Readonly` **termsOfUse**: [`Policy`](identity_wasm.Policy.md) \| [`Policy`](identity_wasm.Policy.md)[]

Terms-of-use specified by the presentation holder.
