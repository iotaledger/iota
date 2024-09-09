# Iota Objects

For Iota, `key` is used to signify an _object_. Objects the only way to store data in Iota--allowing
the data to persist between transactions.

For more details, see the Iota documentation on

- [The Object Model](https://wiki.iota.org/concepts/object-model)
- [Move Rules for Objects](https://wiki.iota.org/concepts/iota-move-concepts#global-unique)
- [Transferring Objects](https://wiki.iota.org/concepts/transfers)

## Object Rules

An object is a [`struct`](../structs.md) with the [`key`](../abilities.md#key) ability. The first
field of the struct must be `id: iota::object::UID`. This 32-byte field (a strongly typed wrapper
around an [`address`](../primitive-types/address.md)) is then used to uniquely identify the object.

Note that since `iota::object::UID` has only the `store` ability (it does not have `copy` or `drop`),
no object has `copy` or `drop`.

## Transfer Rules

Objects can be have their ownership changed and transferred in the `iota::transfer` module. Many
functions in the module have "public" and "private" variant, where the "private" variant can only be
called inside of the module that defines the object's type. The "public" variants can be called only
if the object has `store`.

For example if we had two objects `A` and `B` defined in the module `my_module`:

```
module a::my_module {
    public struct A has key {
        id: iota::object::UID,
    }
    public struct B has key, store {
        id: iota::object::UID,
    }
}
```

`A` can only be transferred using the `iota::transfer::transfer` inside of `a::my_module`, while `B`
can be transferred anywhere using `iota::transfer::public_transfer`. These rules are enforced by a
custom type system (bytecode verifier) rule in Iota.
