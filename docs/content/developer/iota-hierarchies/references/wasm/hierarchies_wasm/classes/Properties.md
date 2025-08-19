[**@iota/hierarchies API documentation**](../../api_ref.md)

***

# Class: Properties

Properties is a struct that contains a map of PropertyName to Property

## Properties

### data

> `readonly` **data**: [`FederationProperty`](FederationProperty.md)[]

Retrieves all property names and their corresponding property data as a JavaScript Map.

# Returns
A list of Property objects.

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

***

### add\_property()

> **add\_property**(`property`): `void`

Adds a new property to the properties list

#### Parameters

##### property

[`FederationProperty`](FederationProperty.md)

#### Returns

`void`
