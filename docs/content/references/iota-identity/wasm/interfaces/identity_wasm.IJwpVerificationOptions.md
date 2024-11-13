# Interface: IJwpVerificationOptions

[identity\_wasm](../modules/identity_wasm.md).IJwpVerificationOptions

Holds options to create a new [JwpVerificationOptions](../classes/identity_wasm.JwpVerificationOptions.md).

## Table of contents

### Properties

- [methodScope](identity_wasm.IJwpVerificationOptions.md#methodscope)
- [methodId](identity_wasm.IJwpVerificationOptions.md#methodid)

## Properties

### methodScope

• `Optional` `Readonly` **methodScope**: [`MethodScope`](../classes/identity_wasm.MethodScope.md)

Verify the signing verification method relation matches this.

___

### methodId

• `Optional` `Readonly` **methodId**: [`DIDUrl`](../classes/identity_wasm.DIDUrl.md)

The DID URL of the method, whose JWK should be used to verify the JWP.
If unset, the `kid` of the JWP is used as the DID URL.
