// invalid, objects need UID not ID
module a::m {
    use iota::object;
    struct S has key {
        id: object::ID
    }
}

module iota::object {
    struct ID has store {
        id: address,
    }
}
