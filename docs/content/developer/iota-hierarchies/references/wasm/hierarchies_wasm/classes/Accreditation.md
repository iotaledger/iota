[**@iota/hierarchies API documentation**](../../api_ref.md)

---

# Class: Accreditation

Represents an accreditation, which is a collection of properties granted by an accreditor.

## Properties

### id

> `readonly` **id**: `string`

Returns the unique identifier of the accreditation.

---

### accreditedBy

> `readonly` **accreditedBy**: `string`

Returns the identifier of the entity that granted the accreditation.

---

### properties

> `readonly` **properties**: [`FederationProperty`](FederationProperty.md)[]

Returns the properties associated with this accreditation as a map.

## Methods

### toJSON()

> **toJSON**(): `Object`

- Return copy of self without private attributes.

#### Returns

`Object`

---

### toString()

> **toString**(): `string`

Return stringified version of self.

#### Returns

`string`
