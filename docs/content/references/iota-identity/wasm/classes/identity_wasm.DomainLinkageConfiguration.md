# Class: DomainLinkageConfiguration

[identity\_wasm](../modules/identity_wasm.md).DomainLinkageConfiguration

DID Configuration Resource which contains Domain Linkage Credentials.
It can be placed in an origin's `.well-known` directory to prove linkage between the origin and a DID.
See: <https://identity.foundation/.well-known/resources/did-configuration/#did-configuration-resource>

Note:
- Only the [JSON Web Token Proof Format](https://identity.foundation/.well-known/resources/did-configuration/#json-web-token-proof-format)

## Table of contents

### Constructors

- [constructor](identity_wasm.DomainLinkageConfiguration.md#constructor)

### Methods

- [toJSON](identity_wasm.DomainLinkageConfiguration.md#tojson)
- [toString](identity_wasm.DomainLinkageConfiguration.md#tostring)
- [linkedDids](identity_wasm.DomainLinkageConfiguration.md#linkeddids)
- [issuers](identity_wasm.DomainLinkageConfiguration.md#issuers)
- [fromJSON](identity_wasm.DomainLinkageConfiguration.md#fromjson)
- [clone](identity_wasm.DomainLinkageConfiguration.md#clone)

## Constructors

### constructor

• **new DomainLinkageConfiguration**(`linkedDids`)

Constructs a new [DomainLinkageConfiguration](identity_wasm.DomainLinkageConfiguration.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `linkedDids` | [`Jwt`](identity_wasm.Jwt.md)[] |

## Methods

### toJSON

▸ **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

▸ **toJSON**(): `any`

Serializes this to a JSON object.

#### Returns

`any`

___

### toString

▸ **toString**(): `string`

Return stringified version of self.

#### Returns

`string`

___

### linkedDids

▸ **linkedDids**(): [`Jwt`](identity_wasm.Jwt.md)[]

List of the Domain Linkage Credentials.

#### Returns

[`Jwt`](identity_wasm.Jwt.md)[]

___

### issuers

▸ **issuers**(): [`CoreDID`](identity_wasm.CoreDID.md)[]

List of the issuers of the Domain Linkage Credentials.

#### Returns

[`CoreDID`](identity_wasm.CoreDID.md)[]

___

### fromJSON

▸ `Static` **fromJSON**(`json`): [`DomainLinkageConfiguration`](identity_wasm.DomainLinkageConfiguration.md)

Deserializes an instance from a JSON object.

#### Parameters

| Name | Type |
| :------ | :------ |
| `json` | `any` |

#### Returns

[`DomainLinkageConfiguration`](identity_wasm.DomainLinkageConfiguration.md)

___

### clone

▸ **clone**(): [`DomainLinkageConfiguration`](identity_wasm.DomainLinkageConfiguration.md)

Deep clones the object.

#### Returns

[`DomainLinkageConfiguration`](identity_wasm.DomainLinkageConfiguration.md)
