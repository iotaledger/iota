# Enumeration: StatusCheck

[identity\_wasm](../modules/identity_wasm.md).StatusCheck

Controls validation behaviour when checking whether or not a credential has been revoked by its
[`credentialStatus`](https://www.w3.org/TR/vc-data-model/#status).

## Table of contents

### Enumeration Members

- [Strict](identity_wasm.StatusCheck.md#strict)
- [SkipUnsupported](identity_wasm.StatusCheck.md#skipunsupported)
- [SkipAll](identity_wasm.StatusCheck.md#skipall)

## Enumeration Members

### Strict

• **Strict** = ``0``

Validate the status if supported, reject any unsupported
[`credentialStatus`](https://www.w3.org/TR/vc-data-model/#status) types.

Only `RevocationBitmap2022` is currently supported.

This is the default.

___

### SkipUnsupported

• **SkipUnsupported** = ``1``

Validate the status if supported, skip any unsupported
[`credentialStatus`](https://www.w3.org/TR/vc-data-model/#status) types.

___

### SkipAll

• **SkipAll** = ``2``

Skip all status checks.
