// valid
module a::m {
    use iota::object;
    struct S has key {
        id: object::UID
    }
}

module iota::object {
    struct UID has store {
        id: address,
    }
}
