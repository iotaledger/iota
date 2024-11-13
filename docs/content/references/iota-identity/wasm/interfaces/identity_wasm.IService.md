# Interface: IService

[identity\_wasm](../modules/identity_wasm.md).IService

Base [Service](../classes/identity_wasm.Service.md) properties.

## Table of contents

### Properties

- [id](identity_wasm.IService.md#id)
- [type](identity_wasm.IService.md#type)
- [serviceEndpoint](identity_wasm.IService.md#serviceendpoint)
- [properties](identity_wasm.IService.md#properties)

## Properties

### id

• `Readonly` **id**: `string` \| [`DIDUrl`](../classes/identity_wasm.DIDUrl.md)

Identifier of the service.

Must be a valid DIDUrl with a fragment.

___

### type

• `Readonly` **type**: `string` \| `string`[]

Type of service.

E.g. "LinkedDomains".

___

### serviceEndpoint

• `Readonly` **serviceEndpoint**: `string` \| `string`[] \| `Map`\<`string`, `string`[]\> \| `Record`\<`string`, `string`[]\>

A URL, set of URLs, or map of URL sets.

NOTE: throws an error if any entry is not a valid URL string. List entries must be unique.

___

### properties

• `Optional` `Readonly` **properties**: `Record`\<`string`, `any`\> \| `Map`\<`string`, `any`\>

Additional custom properties to embed in the service.

WARNING: entries may overwrite existing fields and result in invalid documents.
