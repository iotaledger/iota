# Class: StatusList2021CredentialBuilder

[identity\_wasm](../modules/identity_wasm.md).StatusList2021CredentialBuilder

Builder type to construct valid [StatusList2021Credential](identity_wasm.StatusList2021Credential.md) istances.

## Table of contents

### Constructors

- [constructor](identity_wasm.StatusList2021CredentialBuilder.md#constructor)

### Methods

- [purpose](identity_wasm.StatusList2021CredentialBuilder.md#purpose)
- [subjectId](identity_wasm.StatusList2021CredentialBuilder.md#subjectid)
- [expirationDate](identity_wasm.StatusList2021CredentialBuilder.md#expirationdate)
- [issuer](identity_wasm.StatusList2021CredentialBuilder.md#issuer)
- [context](identity_wasm.StatusList2021CredentialBuilder.md#context)
- [type](identity_wasm.StatusList2021CredentialBuilder.md#type)
- [proof](identity_wasm.StatusList2021CredentialBuilder.md#proof)
- [build](identity_wasm.StatusList2021CredentialBuilder.md#build)

## Constructors

### constructor

• **new StatusList2021CredentialBuilder**(`status_list?`)

Creates a new [StatusList2021CredentialBuilder](identity_wasm.StatusList2021CredentialBuilder.md).

#### Parameters

| Name | Type |
| :------ | :------ |
| `status_list?` | [`StatusList2021`](identity_wasm.StatusList2021.md) |

## Methods

### purpose

▸ **purpose**(`purpose`): [`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

Sets the purpose of the [StatusList2021Credential](identity_wasm.StatusList2021Credential.md) that is being created.

#### Parameters

| Name | Type |
| :------ | :------ |
| `purpose` | [`StatusPurpose`](../enums/identity_wasm.StatusPurpose.md) |

#### Returns

[`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

___

### subjectId

▸ **subjectId**(`id`): [`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

Sets `credentialSubject.id`.

#### Parameters

| Name | Type |
| :------ | :------ |
| `id` | `string` |

#### Returns

[`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

___

### expirationDate

▸ **expirationDate**(`time`): [`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

Sets the expiration date of the credential.

#### Parameters

| Name | Type |
| :------ | :------ |
| `time` | [`Timestamp`](identity_wasm.Timestamp.md) |

#### Returns

[`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

___

### issuer

▸ **issuer**(`issuer`): [`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

Sets the issuer of the credential.

#### Parameters

| Name | Type |
| :------ | :------ |
| `issuer` | `string` |

#### Returns

[`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

___

### context

▸ **context**(`context`): [`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

Sets the context of the credential.

#### Parameters

| Name | Type |
| :------ | :------ |
| `context` | `string` |

#### Returns

[`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

___

### type

▸ **type**(`t`): [`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

Adds a credential type.

#### Parameters

| Name | Type |
| :------ | :------ |
| `t` | `string` |

#### Returns

[`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

___

### proof

▸ **proof**(`proof`): [`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

Adds a credential's proof.

#### Parameters

| Name | Type |
| :------ | :------ |
| `proof` | [`Proof`](identity_wasm.Proof.md) |

#### Returns

[`StatusList2021CredentialBuilder`](identity_wasm.StatusList2021CredentialBuilder.md)

___

### build

▸ **build**(): [`StatusList2021Credential`](identity_wasm.StatusList2021Credential.md)

Attempts to build a valid [StatusList2021Credential](identity_wasm.StatusList2021Credential.md) with the previously provided data.

#### Returns

[`StatusList2021Credential`](identity_wasm.StatusList2021Credential.md)
