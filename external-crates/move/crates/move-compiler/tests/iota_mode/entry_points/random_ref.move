// valid Random by immutable reference

module a::m {
    public entry fun yes_random_ref(_: &iota::random::Random) {
        abort 0
    }
}

module iota::random {
    struct Random has key {
        id: iota::object::UID,
    }
}

module iota::object {
    struct UID has store {
        id: address,
    }
}
