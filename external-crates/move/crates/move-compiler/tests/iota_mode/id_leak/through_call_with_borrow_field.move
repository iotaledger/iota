// not allowed, the call tries to make a new UID
module a::m {
    use iota::object::UID;
    use iota::transfer::transfer;

    struct S has copy, drop { f: u64 }

    struct Object has key { id: UID }

    public fun new(id: UID): Object {
        Object { id }
    }

    public entry fun foo(obj: Object) {
        let s = S { f: 0 };
        let Object { id } = obj;
        _ = &s.f;
        transfer(new(id), @42);
    }

}

module iota::object {
    struct UID has store {
        id: address,
    }
}

module iota::transfer {
    public fun transfer<T: key>(_: T, _: address) {
        abort 0
    }
}
