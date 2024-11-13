# Interface: IJwsSignatureOptions

[identity\_wasm](../modules/identity_wasm.md).IJwsSignatureOptions

Holds options to create [JwsSignatureOptions](../classes/identity_wasm.JwsSignatureOptions.md).

## Table of contents

### Properties

- [attachJwk](identity_wasm.IJwsSignatureOptions.md#attachjwk)
- [b64](identity_wasm.IJwsSignatureOptions.md#b64)
- [typ](identity_wasm.IJwsSignatureOptions.md#typ)
- [cty](identity_wasm.IJwsSignatureOptions.md#cty)
- [url](identity_wasm.IJwsSignatureOptions.md#url)
- [nonce](identity_wasm.IJwsSignatureOptions.md#nonce)
- [kid](identity_wasm.IJwsSignatureOptions.md#kid)
- [detachedPayload](identity_wasm.IJwsSignatureOptions.md#detachedpayload)
- [customHeaderParameters](identity_wasm.IJwsSignatureOptions.md#customheaderparameters)

## Properties

### attachJwk

• `Optional` `Readonly` **attachJwk**: `boolean`

Whether to attach the public key in the corresponding method
to the JWS header.

Default: false

___

### b64

• `Optional` `Readonly` **b64**: `boolean`

Whether to Base64url encode the payload or not.

[More Info](https://tools.ietf.org/html/rfc7797#section-3)

___

### typ

• `Optional` `Readonly` **typ**: `string`

The Type value to be placed in the protected header.

[More Info](https://tools.ietf.org/html/rfc7515#section-4.1.9)

___

### cty

• `Optional` `Readonly` **cty**: `string`

Content Type to be placed in the protected header.

[More Info](https://tools.ietf.org/html/rfc7515#section-4.1.10)

___

### url

• `Optional` `Readonly` **url**: `string`

The URL to be placed in the protected header.

[More Info](https://tools.ietf.org/html/rfc8555#section-6.4.1)

___

### nonce

• `Optional` `Readonly` **nonce**: `string`

The nonce to be placed in the protected header.

[More Info](https://tools.ietf.org/html/rfc8555#section-6.5.2)

___

### kid

• `Optional` `Readonly` **kid**: `string`

The kid to set in the protected header.
If unset, the kid of the JWK with which the JWS is produced is used.

[More Info](https://www.rfc-editor.org/rfc/rfc7515#section-4.1.4)

___

### detachedPayload

• `Optional` `Readonly` **detachedPayload**: `boolean`

/// Whether the payload should be detached from the JWS.

[More Info](https://www.rfc-editor.org/rfc/rfc7515#appendix-F).

___

### customHeaderParameters

• `Optional` `Readonly` **customHeaderParameters**: `Record`\<`string`, `any`\>

Additional header parameters.
