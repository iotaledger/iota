[**@iota/hierarchies API documentation**](../../api_ref.md)

---

# Class: HierarchiesClient

A client to interact with Hierarchies objects on the IOTA ledger.

This client is used for read and write operations. For read-only capabilities,
you can use [HierarchiesClientReadOnly](HierarchiesClientReadOnly.md), which does not require an account or signing capabilities.

## Constructors

### Constructor

> **new HierarchiesClient**(`client`, `signer`): `HierarchiesClient`

Creates a new client with signing capabilities.

# Arguments

- `client` - A read-only client for blockchain interaction.
- `signer` - A signer for transaction authorization.

# Errors

Returns an error if the signer's public key cannot be retrieved.

```
#### Parameters

##### client

[`HierarchiesClientReadOnly`](HierarchiesClientReadOnly.md)

##### signer

`TransactionSigner`

#### Returns

`HierarchiesClient`

## Methods

### createNewFederation()

> **createNewFederation**(): `any`

Creates a new [`WasmTransactionBuilder`] for creating a new federation.

See [`HierarchiesClient::create_new_federation`] for more details.

#### Returns

`any`

***

### addRootAuthority()

> **addRootAuthority**(`federation_id`, `account_id`): `any`

Creates a [`WasmTransactionBuilder`] for adding a root authority to a federation.

# Arguments

* `federation_id` - The [`WasmObjectID`] of the federation.
* `account_id` - The [`WasmObjectID`] of the account to add as a root authority.

#### Parameters

##### federation\_id

`string`

##### account\_id

`string`

#### Returns

`any`

***

### revokeRootAuthority()

> **revokeRootAuthority**(`federation_id`, `account_id`): `any`

Creates a [`WasmTransactionBuilder`] for revoking a root authority from a federation.

Only existing root authorities can revoke other root authorities.
Cannot revoke the last root authority to prevent lockout.

# Arguments

* `federation_id` - The [`WasmObjectID`] of the federation.
* `account_id` - The [`WasmObjectID`] of the account to revoke as a root authority.

#### Parameters

##### federation\_id

`string`

##### account\_id

`string`

#### Returns

`any`

***

### reinstateRootAuthority()

> **reinstateRootAuthority**(`federation_id`, `account_id`): `any`

Creates a [`WasmTransactionBuilder`] for reinstating a revoked root authority to a federation.

Only existing root authorities can reinstate revoked root authorities.
The target account must be in the revoked list to be reinstated.

# Arguments

* `federation_id` - The [`WasmObjectID`] of the federation.
* `account_id` - The [`WasmObjectID`] of the account to reinstate as a root authority.

#### Parameters

##### federation\_id

`string`

##### account\_id

`string`

#### Returns

`any`

***

### addProperty()

> **addProperty**(`federation_id`, `property`): `any`

Creates a new [`WasmTransactionBuilder`] for adding a property to a federation.

# Arguments

* `federation_id` - The [`WasmObjectID`] of the federation.
* `property` - The property to add.

#### Parameters

##### federation\_id

`string`

##### property

[`FederationProperty`](FederationProperty.md)

#### Returns

`any`

***

### revoke\_property()

> **revoke\_property**(`federation_id`, `property_name`, `valid_to_ms?`): `any`

Creates a new [`WasmTransactionBuilder`] for revoking a property from a federation.

# Arguments

* `federation_id` - The [`WasmObjectID`] of the federation.
* `property_name` - The name of the property to revoke.
* `valid_to_ms` - The timestamp in milliseconds until which the property is valid.

#### Parameters

##### federation\_id

`string`

##### property\_name

`PropertyName`

##### valid\_to\_ms?

`null` | `bigint`

#### Returns

`any`

***

### createAccreditationToAttest()

> **createAccreditationToAttest**(`federation_id`, `receiver`, `want_properties`): `any`

Creates a new [`WasmTransactionBuilder`] for creating an accreditation to attest.

# Arguments

* `federation_id` - The [`WasmObjectID`] of the federation.
* `receiver` - The [`WasmObjectID`] of the receiver of the accreditation.
* `want_properties` - The properties for which permissions are being granted.

#### Parameters

##### federation\_id

`string`

##### receiver

`string`

##### want\_properties

[`FederationProperty`](FederationProperty.md)[]

#### Returns

`any`

***

### revokeAccreditationToAttest()

> **revokeAccreditationToAttest**(`federation_id`, `user_id`, `permission_id`): `any`

Creates a new [`WasmTransactionBuilder`] for revoking an accreditation to attest.

# Arguments

* `federation_id` - The [`WasmObjectID`] of the federation.
* `user_id` - The [`WasmObjectID`] of the user whose accreditation is being revoked.
* `permission_id` - The [`WasmObjectID`] of the permission to revoke.

#### Parameters

##### federation\_id

`string`

##### user\_id

`string`

##### permission\_id

`string`

#### Returns

`any`

***

### createAccreditationToAccredit()

> **createAccreditationToAccredit**(`federation_id`, `receiver`, `want_properties`): `any`

Creates a new [`WasmTransactionBuilder`] for creating an accreditation to accredit.

# Arguments

* `federation_id` - The [`WasmObjectID`] of the federation.
* `receiver` - The [`WasmObjectID`] of the receiver of the accreditation.
* `want_properties` - The properties for which permissions are being granted.

#### Parameters

##### federation\_id

`string`

##### receiver

`string`

##### want\_properties

[`FederationProperty`](FederationProperty.md)[]

#### Returns

`any`

***

### revokeAccreditationToAccredit()

> **revokeAccreditationToAccredit**(`federation_id`, `user_id`, `accreditation_id`): `any`

Creates a new [`WasmTransactionBuilder`] for revoking an accreditation to accredit.

# Arguments

* `federation_id` - The [`WasmObjectID`] of the federation.
* `user_id` - The [`WasmObjectID`] of the user whose accreditation is being revoked.
* `accreditation_id` - The [`WasmObjectID`] of the accreditation to revoke.

#### Parameters

##### federation\_id

`string`

##### user\_id

`string`

##### accreditation\_id

`string`

#### Returns

`any`

***

### senderPublicKey()

> **senderPublicKey**(): `PublicKey`

Retrieves the sender's public key.

#### Returns

`PublicKey`

***

### senderAddress()

> **senderAddress**(): `string`

Retrieves the sender's address.

#### Returns

`string`

***

### network()

> **network**(): `string`

Retrieves the network identifier.

#### Returns

`string`

***

### packageId()

> **packageId**(): `string`

Retrieves the package ID.

#### Returns

`string`

***

### packageHistory()

> **packageHistory**(): `string`[]

Retrieves the package history.

#### Returns

`string`[]

***

### iotaClient()

> **iotaClient**(): `IotaClient`

Retrieves the IOTA client instance.

#### Returns

`IotaClient`

***

### signer()

> **signer**(): `TransactionSigner`

Retrieves the transaction signer.

#### Returns

`TransactionSigner`

***

### readOnly()

> **readOnly**(): [`HierarchiesClientReadOnly`](HierarchiesClientReadOnly.md)

Retrieves a read-only version of the hierarchies client.

#### Returns

[`HierarchiesClientReadOnly`](HierarchiesClientReadOnly.md)
```
