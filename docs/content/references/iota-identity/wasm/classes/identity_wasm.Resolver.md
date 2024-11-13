# Class: Resolver

[identity\_wasm](../modules/identity_wasm.md).Resolver

Convenience type for resolving DID documents from different DID methods.   
 
Also provides methods for resolving DID Documents associated with
verifiable [Credential](identity_wasm.Credential.md)s and [Presentation](identity_wasm.Presentation.md)s.

# Configuration

The resolver will only be able to resolve DID documents for methods it has been configured for in the constructor.

## Table of contents

### Constructors

- [constructor](identity_wasm.Resolver.md#constructor)

### Methods

- [resolve](identity_wasm.Resolver.md#resolve)
- [resolveMultiple](identity_wasm.Resolver.md#resolvemultiple)

## Constructors

### constructor

• **new Resolver**(`config`)

Constructs a new [Resolver](identity_wasm.Resolver.md).

# Errors
If both a `client` is given and the `handlers` map contains the "iota" key the construction process
will throw an error because the handler for the "iota" method then becomes ambiguous.

#### Parameters

| Name | Type |
| :------ | :------ |
| `config` | [`ResolverConfig`](../modules/identity_wasm.md#resolverconfig) |

## Methods

### resolve

▸ **resolve**(`did`): `Promise`\<`IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md)\>

Fetches the DID Document of the given DID.

### Errors

Errors if the resolver has not been configured to handle the method
corresponding to the given DID or the resolution process itself fails.

#### Parameters

| Name | Type |
| :------ | :------ |
| `did` | `string` |

#### Returns

`Promise`\<`IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md)\>

___

### resolveMultiple

▸ **resolveMultiple**(`dids`): `Promise`\<(`IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md))[]\>

Concurrently fetches the DID Documents of the multiple given DIDs.

# Errors
* If the resolver has not been configured to handle the method of any of the given DIDs.
* If the resolution process of any DID fails.

## Note
* The order of the documents in the returned array matches that in `dids`.
* If `dids` contains duplicates, these will be resolved only once and the resolved document
is copied into the returned array to match the order of `dids`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `dids` | `string`[] |

#### Returns

`Promise`\<(`IToCoreDocument` \| [`CoreDocument`](identity_wasm.CoreDocument.md))[]\>
