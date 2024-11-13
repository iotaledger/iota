# Enumeration: SubjectHolderRelationship

[identity\_wasm](../modules/identity_wasm.md).SubjectHolderRelationship

Declares how credential subjects must relate to the presentation holder.

See also the [Subject-Holder Relationship](https://www.w3.org/TR/vc-data-model/#subject-holder-relationships) section of the specification.

## Table of contents

### Enumeration Members

- [AlwaysSubject](identity_wasm.SubjectHolderRelationship.md#alwayssubject)
- [SubjectOnNonTransferable](identity_wasm.SubjectHolderRelationship.md#subjectonnontransferable)
- [Any](identity_wasm.SubjectHolderRelationship.md#any)

## Enumeration Members

### AlwaysSubject

• **AlwaysSubject** = ``0``

The holder must always match the subject on all credentials, regardless of their [`nonTransferable`](https://www.w3.org/TR/vc-data-model/#nontransferable-property) property.
This variant is the default.

___

### SubjectOnNonTransferable

• **SubjectOnNonTransferable** = ``1``

The holder must match the subject only for credentials where the [`nonTransferable`](https://www.w3.org/TR/vc-data-model/#nontransferable-property) property is `true`.

___

### Any

• **Any** = ``2``

The holder is not required to have any kind of relationship to any credential subject.
