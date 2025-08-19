[**@iota/hierarchies API documentation**](../../api_ref.md)

***

# Class: Governance

Represents the governance of a federation

## Properties

### id

> `readonly` **id**: `string`

Retrieves the ID of the governance.

# Returns
A string representing the governance ID.

***

### properties

> `readonly` **properties**: [`Properties`](Properties.md)

Retrieves the properties in the governance.

# Returns
The properties object.

***

### accreditationsToAccredit

> `readonly` **accreditationsToAccredit**: `Map`\<`any`, `any`\>

Retrieves the accreditations to accredit mapping.

# Returns
A JavaScript Map object containing accreditations to accredit.

***

### accreditationsToAttest

> `readonly` **accreditationsToAttest**: `Map`\<`any`, `any`\>

Retrieves the accreditations to attest mapping.

# Returns
A JavaScript Map object containing accreditations to attest.

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
