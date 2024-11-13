# Class: LinkedDomainService

[identity\_wasm](../modules/identity_wasm.md).LinkedDomainService

## Table of contents

### Constructors

- [constructor](identity_wasm.LinkedDomainService.md#constructor)

### Methods

- [toJSON](identity_wasm.LinkedDomainService.md#tojson)
- [toString](identity_wasm.LinkedDomainService.md#tostring)
- [domains](identity_wasm.LinkedDomainService.md#domains)
- [toService](identity_wasm.LinkedDomainService.md#toservice)
- [fromService](identity_wasm.LinkedDomainService.md#fromservice)
- [isValid](identity_wasm.LinkedDomainService.md#isvalid)
- [clone](identity_wasm.LinkedDomainService.md#clone)

## Constructors

### constructor

• **new LinkedDomainService**(`options`)

Constructs a new [LinkedDomainService](identity_wasm.LinkedDomainService.md) that wraps a spec compliant [Linked Domain Service Endpoint](https://identity.foundation/.well-known/resources/did-configuration/#linked-domain-service-endpoint).

Domain URLs must include the `https` scheme in order to pass the domain linkage validation.

#### Parameters

| Name | Type |
| :------ | :------ |
| `options` | [`ILinkedDomainService`](../interfaces/identity_wasm.ILinkedDomainService.md) |

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

___

### domains

▸ **domains**(): `string`[]

Returns the domains contained in the Linked Domain Service.

#### Returns

`string`[]

___

### toService

▸ **toService**(): [`Service`](identity_wasm.Service.md)

Returns the inner service which can be added to a DID Document.

#### Returns

[`Service`](identity_wasm.Service.md)

___

### fromService

▸ `Static` **fromService**(`service`): [`LinkedDomainService`](identity_wasm.LinkedDomainService.md)

Creates a new [LinkedDomainService](identity_wasm.LinkedDomainService.md) from a [Service](identity_wasm.Service.md).

# Error

Errors if `service` is not a valid Linked Domain Service.

#### Parameters

| Name | Type |
| :------ | :------ |
| `service` | [`Service`](identity_wasm.Service.md) |

#### Returns

[`LinkedDomainService`](identity_wasm.LinkedDomainService.md)

___

### isValid

▸ `Static` **isValid**(`service`): `boolean`

Returns `true` if a [Service](identity_wasm.Service.md) is a valid Linked Domain Service.

#### Parameters

| Name | Type |
| :------ | :------ |
| `service` | [`Service`](identity_wasm.Service.md) |

#### Returns

`boolean`

___

### clone

▸ **clone**(): [`LinkedDomainService`](identity_wasm.LinkedDomainService.md)

Deep clones the object.

#### Returns

[`LinkedDomainService`](identity_wasm.LinkedDomainService.md)
