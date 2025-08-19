[**@iota/hierarchies API documentation**](../../api_ref.md)

***

# Class: Federation

Represents a federation. A federation is a group of entities that have agreed to work together

## Properties

### id

> `readonly` **id**: `string`

Retrieves the ID of the federation.

# Returns
A string representing the federation ID.

***

### governance

> `readonly` **governance**: [`Governance`](Governance.md)

Retrieves the governance of the federation.

# Returns
The governance object for the federation.

***

### rootAuthorities

> `readonly` **rootAuthorities**: [`RootAuthority`](RootAuthority.md)[]

Retrieves the root authorities of the federation.

# Returns
An array of root authorities.

***

### revokedRootAuthorities

> `readonly` **revokedRootAuthorities**: `string`[]

Retrieves the revoked root authorities of the federation.

# Returns
An array of revoked root authorities.

## Methods

### toJSON()

> **toJSON**(): `Object`

* Return copy of self without private attributes.

#### Returns

`Object`

***

### toString()

> **toString**(): `string`

Return stringified version of self.

#### Returns

`string`
