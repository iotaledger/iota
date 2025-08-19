[**@iota/hierarchies API documentation**](../../api_ref.md)

***

# Class: HierarchiesClientReadOnly

A client to interact with Hierarchies objects on the IOTA ledger.

This client is used for read-only operations, meaning it does not require an account
or signing capabilities. For write operations, use [HierarchiesClient](HierarchiesClient.md).

## Methods

### create()

> `static` **create**(`iota_client`): `Promise`\<`HierarchiesClientReadOnly`\>

Creates a new instance of `HierarchiesClientReadOnly`.

# Arguments
* `iota_client` - The IOTA client used for interacting with the ledger.

# Returns
A new `HierarchiesClientReadOnly` instance.

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with `WasmHierarchiesClientReadOnly`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const client = await HierarchiesClientReadOnly.create(iotaClient);
  console.log("Client created:", client);
} catch (error) {
  console.error("Failed to create client:", error);
}
```

#### Parameters

##### iota\_client

`IotaClient`

#### Returns

`Promise`\<`HierarchiesClientReadOnly`\>

***

### createWithPkgId()

> `static` **createWithPkgId**(`iota_client`, `iota_hierarchies_pkg_id`): `Promise`\<`HierarchiesClientReadOnly`\>

Creates a new instance of `HierarchiesClientReadOnly` using a specific package ID.

# Arguments
* `iota_client` - The IOTA client used for interacting with the ledger.
* `iota_hierarchies_pkg_id` - The hierarchies package ID.

# Returns
A new `HierarchiesClientReadOnly` instance.

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with `WasmHierarchiesClientReadOnly`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const client = await HierarchiesClientReadOnly.createWithPkgId(iotaClient, pkgId);
  console.log("Client created:", client);
} catch (error) {
  console.error("Failed to create client:", error);
}
```

#### Parameters

##### iota\_client

`IotaClient`

##### iota\_hierarchies\_pkg\_id

`string`

#### Returns

`Promise`\<`HierarchiesClientReadOnly`\>

***

### packageId()

> **packageId**(): `string`

Retrieves the package ID of the used hierarchies package.

# Returns
A string representing the package ID.

#### Returns

`string`

***

### packageHistory()

> **packageHistory**(): `string`[]

Retrieves the history of hierarchies package IDs.

# Returns
An array of strings representing the package history.

#### Returns

`string`[]

***

### iotaClient()

> **iotaClient**(): `IotaClient`

Retrieves the underlying IOTA client used by this client.

# Returns
The `IotaClient` instance.

#### Returns

`IotaClient`

***

### network()

> **network**(): `string`

Retrieves the network identifier associated with this client.

# Returns
A string representing the network identifier.

#### Returns

`string`

***

### chainId()

> **chainId**(): `string`

Retrieves the chain ID associated with this client.

# Returns
A string representing the chain ID.

#### Returns

`string`

***

### getFederationById()

> **getFederationById**(`federation_id`): `Promise`\<[`Federation`](Federation.md)\>

Retrieves a federation by its ID.

# Arguments

* `federation_id`: The [`ObjectID`] of the federation.

# Returns
A `Result` containing the [`Federation`] object or an [`Error`].

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with `WasmFederation`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const federation = await client.getFederationById(federationId);
  console.log("Federation:", federation);
} catch (error) {
  console.error("Failed to get federation:", error);
}
```

#### Parameters

##### federation\_id

`string`

#### Returns

`Promise`\<[`Federation`](Federation.md)\>

***

### isRootAuthority()

> **isRootAuthority**(`federation_id`, `user_id`): `Promise`\<`boolean`\>

Check if root authority is in the federation.
# Arguments

* `federation_id`: The [`ObjectID`] of the federation.
* `user_id`: The [`ObjectID`] of the user.

# Returns
A `Result` containing a boolean indicating if the user is a root authority or an [`Error`].

#### Parameters

##### federation\_id

`string`

##### user\_id

`string`

#### Returns

`Promise`\<`boolean`\>

***

### getProperties()

> **getProperties**(`federation_id`): `Promise`\<`PropertyName`[]\>

Retrieves all property names registered in the federation.

# Arguments

* `federation_id`: The [`ObjectID`] of the federation.

# Returns
A `Result` containing the list of property names or an [`Error`].

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with `WasmProperty[]`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const properties = await client.getProperties(federationId);
  console.log("Properties:", properties);
} catch (error) {
  console.error("Failed to get properties:", error);
}
```

#### Parameters

##### federation\_id

`string`

#### Returns

`Promise`\<`PropertyName`[]\>

***

### isPropertyInFederation()

> **isPropertyInFederation**(`federation_id`, `property_name`): `Promise`\<`boolean`\>

Checks if a property is registered in the federation.

# Arguments

* `federation_id`: The [`ObjectID`] of the federation.
* `property_name`: The name of the property to check.

# Returns
A `Result` containing a boolean indicating if the property is registered or an [`Error`].

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with a `boolean`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const isRegistered = await client.isPropertyInFederation(federationId, propertyName);
  console.log("Is property registered:", isRegistered);
} catch (error) {
  console.error("Failed to check property registration:", error);
}
```

#### Parameters

##### federation\_id

`string`

##### property\_name

`PropertyName`

#### Returns

`Promise`\<`boolean`\>

***

### getAccreditationsToAttest()

> **getAccreditationsToAttest**(`federation_id`, `user_id`): `Promise`\<[`Accreditations`](Accreditations.md)\>

Retrieves attestation accreditations for a specific user.

# Arguments

* `federation_id`: The [`ObjectID`] of the federation.
* `user_id`: The [`ObjectID`] of the user.

# Returns
A `Result` containing the attestation accreditations or an [`Error`].

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with `WasmAccreditations`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const accreditations = await client.getAccreditationsToAttest(federationId, userId);
  console.log("Accreditations:", accreditations);
} catch (error) {
  console.error("Failed to get accreditations:", error);
}
```

#### Parameters

##### federation\_id

`string`

##### user\_id

`string`

#### Returns

`Promise`\<[`Accreditations`](Accreditations.md)\>

***

### isAttester()

> **isAttester**(`federation_id`, `user_id`): `Promise`\<`boolean`\>

Checks if a user has attestation accreditation.

# Arguments

* `federation_id`: The [`ObjectID`] of the federation.
* `user_id`: The [`ObjectID`] of the user.

# Returns
A `Result` containing a boolean indicating if the user has attestation accreditation or an [`Error`].

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with a `boolean`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const isAttester = await client.isAttester(federationId, userId);
  console.log("Is attester:", isAttester);
} catch (error) {
  console.error("Failed to check attester status:", error);
}
```

#### Parameters

##### federation\_id

`string`

##### user\_id

`string`

#### Returns

`Promise`\<`boolean`\>

***

### getAccreditationsToAccredit()

> **getAccreditationsToAccredit**(`federation_id`, `user_id`): `Promise`\<[`Accreditations`](Accreditations.md)\>

Retrieves accreditations to accredit for a specific user.

# Arguments

* `federation_id`: The [`ObjectID`] of the federation.
* `user_id`: The [`ObjectID`] of the user.

# Returns
A `Result` containing the accreditations to accredit for the user or an [`Error`].

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with `WasmAccreditations`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const accreditations = await client.getAccreditationsToAccredit(federationId, userId);
  console.log("Accreditations:", accreditations);
} catch (error) {
  console.error("Failed to get accreditations:", error);
}
```

#### Parameters

##### federation\_id

`string`

##### user\_id

`string`

#### Returns

`Promise`\<[`Accreditations`](Accreditations.md)\>

***

### isAccreditor()

> **isAccreditor**(`federation_id`, `user_id`): `Promise`\<`boolean`\>

Checks if a user has accreditations to accredit.

# Arguments

* `federation_id`: The [`ObjectID`] of the federation.
* `user_id`: The [`ObjectID`] of the user.

# Returns
A `Result` containing a boolean indicating if the user has accreditations to accredit or an [`Error`].

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with a `boolean`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const isAccreditor = await client.isAccreditor(federationId, userId);
  console.log("Is accreditor:", isAccreditor);
} catch (error) {
  console.error("Failed to check accreditor status:", error);
}
```

#### Parameters

##### federation\_id

`string`

##### user\_id

`string`

#### Returns

`Promise`\<`boolean`\>

***

### validateProperty()

> **validateProperty**(`federation_id`, `user_id`, `property_name`, `property_value`): `Promise`\<`boolean`\>

Validates a property for a specific user.

# Arguments

* `federation_id`: The [`ObjectID`] of the federation.
* `user_id`: The [`ObjectID`] of the user.
* `property_name`: The name of the property to validate.
* `property_value`: The value of the property to validate.

# Returns
A `Result` containing a boolean indicating if the property is valid or an [`Error`].

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with a `boolean`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const isValid = await client.validateProperty(federationId, userId, propertyName, propertyValue);
  console.log("Is property valid:", isValid);
} catch (error) {
  console.error("Failed to validate property:", error);
}
```

#### Parameters

##### federation\_id

`string`

##### user\_id

`string`

##### property\_name

`PropertyName`

##### property\_value

`PropertyValue`

#### Returns

`Promise`\<`boolean`\>

***

### validateProperties()

> **validateProperties**(`federation_id`, `entity_id`, `properties`): `Promise`\<`boolean`\>

Validates multiple properties for a specific user.

# Arguments

* `federation_id`: The [`ObjectID`] of the federation.
* `user_id`: The [`ObjectID`] of the user.
* `properties`: The properties to validate.

# Returns
A `Result` containing a boolean indicating if the properties are valid or an [`Error`].

# TypeScript Usage
This method returns a `Promise` in TypeScript.
- On success, the promise resolves with a `boolean`.
- On failure, the promise rejects with an `Error`.

```typescript
try {
  const areValid = await client.validateProperties(federationId, userId, properties);
  console.log("Are properties valid:", areValid);
} catch (error) {
  console.error("Failed to validate properties:", error);
}
```

#### Parameters

##### federation\_id

`string`

##### entity\_id

`string`

##### properties

`Map`\<`any`, `any`\>

#### Returns

`Promise`\<`boolean`\>
