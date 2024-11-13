# Class: Storage

[identity\_wasm](../modules/identity_wasm.md).Storage

A type wrapping a `JwkStorage` and `KeyIdStorage` that should always be used together when
working with storage backed DID documents.

## Table of contents

### Constructors

- [constructor](identity_wasm.Storage.md#constructor)

### Methods

- [keyIdStorage](identity_wasm.Storage.md#keyidstorage)
- [keyStorage](identity_wasm.Storage.md#keystorage)

## Constructors

### constructor

• **new Storage**(`jwkStorage`, `keyIdStorage`)

Constructs a new `Storage`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `jwkStorage` | [`JwkStorage`](../interfaces/identity_wasm.JwkStorage.md) |
| `keyIdStorage` | [`KeyIdStorage`](../interfaces/identity_wasm.KeyIdStorage.md) |

## Methods

### keyIdStorage

▸ **keyIdStorage**(): [`KeyIdStorage`](../interfaces/identity_wasm.KeyIdStorage.md)

Obtain the wrapped `KeyIdStorage`.

#### Returns

[`KeyIdStorage`](../interfaces/identity_wasm.KeyIdStorage.md)

___

### keyStorage

▸ **keyStorage**(): [`JwkStorage`](../interfaces/identity_wasm.JwkStorage.md)

Obtain the wrapped `JwkStorage`.

#### Returns

[`JwkStorage`](../interfaces/identity_wasm.JwkStorage.md)
