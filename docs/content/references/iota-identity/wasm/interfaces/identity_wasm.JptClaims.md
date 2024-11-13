# Interface: JptClaims

[identity\_wasm](../modules/identity_wasm.md).JptClaims

JPT claims

## Table of contents

### Properties

- [iss](identity_wasm.JptClaims.md#iss)
- [sub](identity_wasm.JptClaims.md#sub)
- [exp](identity_wasm.JptClaims.md#exp)
- [iat](identity_wasm.JptClaims.md#iat)
- [nbf](identity_wasm.JptClaims.md#nbf)
- [jti](identity_wasm.JptClaims.md#jti)

## Properties

### iss

• `Optional` `Readonly` **iss**: `string`

Who issued the JWP

___

### sub

• `Optional` `Readonly` **sub**: `string`

Subject of the JPT.

___

### exp

• `Optional` `Readonly` **exp**: `number`

Expiration time.

___

### iat

• `Optional` `Readonly` **iat**: `number`

Issuance date.

___

### nbf

• `Optional` `Readonly` **nbf**: `number`

Time before which the JPT MUST NOT be accepted

___

### jti

• `Optional` `Readonly` **jti**: `string`

Unique ID for the JPT.
