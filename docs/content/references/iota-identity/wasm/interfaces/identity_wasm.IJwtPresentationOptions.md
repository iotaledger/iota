# Interface: IJwtPresentationOptions

[identity\_wasm](../modules/identity_wasm.md).IJwtPresentationOptions

Options to be set in the JWT claims of a verifiable presentation.

## Table of contents

### Properties

- [expirationDate](identity_wasm.IJwtPresentationOptions.md#expirationdate)
- [issuanceDate](identity_wasm.IJwtPresentationOptions.md#issuancedate)
- [audience](identity_wasm.IJwtPresentationOptions.md#audience)
- [customClaims](identity_wasm.IJwtPresentationOptions.md#customclaims)

## Properties

### expirationDate

• `Optional` `Readonly` **expirationDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

Set the presentation's expiration date.
Default: `undefined`.

___

### issuanceDate

• `Optional` `Readonly` **issuanceDate**: [`Timestamp`](../classes/identity_wasm.Timestamp.md)

Set the presentation's issuance date.
Default: current datetime.

___

### audience

• `Optional` `Readonly` **audience**: `string`

Sets the audience for presentation (`aud` property in JWT claims).

## Note:
Value must be a valid URL.

Default: `undefined`.

___

### customClaims

• `Optional` `Readonly` **customClaims**: `Record`\<`string`, `any`\>

Custom claims that can be used to set additional claims on the resulting JWT.
