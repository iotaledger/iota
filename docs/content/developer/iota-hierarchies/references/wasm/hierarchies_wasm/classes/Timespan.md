[**@iota/hierarchies API documentation**](../../api_ref.md)

***

# Class: Timespan

Represents the time span of validity for a property

## Constructors

### Constructor

> **new Timespan**(): `Timespan`

Creates a new `WasmTimespan` with default values.

#### Returns

`Timespan`

## Properties

### validFromMs

> `readonly` **validFromMs**: `undefined` \| `bigint`

Retrieves the start timestamp.

# Returns
The start timestamp in milliseconds.

## Accessors

### setValidFromMs

#### Set Signature

> **set** **setValidFromMs**(`value`): `void`

Sets the start and end timestamps for the timespan.

##### Parameters

###### value

`bigint`

##### Returns

`void`

***

### validUntilMs

#### Get Signature

> **get** **validUntilMs**(): `undefined` \| `bigint`

Retrieves the end timestamp.

# Returns
The end timestamp in milliseconds.

##### Returns

`undefined` \| `bigint`

#### Set Signature

> **set** **validUntilMs**(`value`): `void`

Sets the end timestamp for the timespan.

##### Parameters

###### value

`bigint`

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
