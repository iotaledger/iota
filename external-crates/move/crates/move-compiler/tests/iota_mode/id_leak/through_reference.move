// cannot assign to UID reference
module a::m {
    use iota::object::UID;

    struct Foo has key {
        id: UID,
    }

    public fun foo(f: Foo, ref: &mut UID) {
        let Foo { id } = f;
        *ref = id;
    }

}

module iota::object {
    struct UID has store {
        id: address,
    }
}
