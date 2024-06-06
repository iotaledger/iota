// valid, Receiving type by immut ref with object type param

module a::m {
    use iota::object;
    use iota::transfer::Receiving;

    struct S has key { id: object::UID }

    public entry fun yes(_: &Receiving<S>) { }
}

module iota::object {
    struct UID has store {
        id: address,
    }
}

module iota::transfer {
    struct Receiving<phantom T: key> has drop {
        id: address
    }
}
