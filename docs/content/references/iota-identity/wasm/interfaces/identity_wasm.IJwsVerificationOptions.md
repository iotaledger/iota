# Interface: IJwsVerificationOptions

[identity\_wasm](../modules/identity_wasm.md).IJwsVerificationOptions

Holds options to create [JwsVerificationOptions](../classes/identity_wasm.JwsVerificationOptions.md).

## Table of contents

### Properties

- [nonce](identity_wasm.IJwsVerificationOptions.md#nonce)
- [methodScope](identity_wasm.IJwsVerificationOptions.md#methodscope)
- [methodId](identity_wasm.IJwsVerificationOptions.md#methodid)

## Properties

### nonce

• `Optional` `Readonly` **nonce**: `string`

Verify that the `nonce` set in the protected header matches this.

[More Info](https://tools.ietf.org/html/rfc8555#section-6.5.2)

___

### methodScope

• `Optional` `Readonly` **methodScope**: [`MethodScope`](../classes/identity_wasm.MethodScope.md)

Verify the signing verification method relationship matches this.

___

### methodId

• `Optional` `Readonly` **methodId**: [`DIDUrl`](../classes/identity_wasm.DIDUrl.md)

The DID URL of the method, whose JWK should be used to verify the JWS.
If unset, the `kid` of the JWS is used as the DID Url.
