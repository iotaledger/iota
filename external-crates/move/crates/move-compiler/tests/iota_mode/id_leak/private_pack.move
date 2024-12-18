// not allowed since C is not packed with a fresh UID
module a::a {
    use iota::object::UID;

    struct A has key {
        id: UID
    }
}

module b::b {
    use iota::object::UID;
    use a::a::A;

    struct B has key {
        id: UID
    }
    public fun no(b: B): A {
        let B { id } = b;
        A { id }
    }
}

module iota::object {
    struct UID has store {
        id: address,
    }
}
