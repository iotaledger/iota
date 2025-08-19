[**@iota/hierarchies API documentation**](../../api_ref.md)

***

# Class: FederationProperty

Represents a property that can be granted to an account. A property
consists of a set of properties that must be satisfied by the account
in order to be granted the property.

The evaluation order: allow_any => shape => allowed_values
The evaluation order is determined by the possible size of the set of values
that match the shape.

## Properties

### propertyName

> **propertyName**: `PropertyName`

Retrieves the property name.

# Returns
The property name.

***

### allowedValues

> **allowedValues**: `PropertyValue`[]

Retrieves the allowed values for this property.

# Returns
An array of allowed property values.

***

### allowAny

> **allowAny**: `boolean`

Checks if any value is allowed for this property.

# Returns
A boolean indicating if any value is allowed.

***

### timespan

> **timespan**: [`Timespan`](Timespan.md)

Retrieves the timespan for this property.

# Returns
The timespan object.

## Accessors

### condition

#### Get Signature

> **get** **condition**(): `undefined` \| `PropertyShape`

Retrieves the condition for this property.

# Returns
The property value condition if present.

##### Returns

`undefined` \| `PropertyShape`

#### Set Signature

> **set** **condition**(`value`): `void`

Sets the condition for this property.

##### Parameters

###### value

`PropertyShape`

##### Returns

`void`

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
