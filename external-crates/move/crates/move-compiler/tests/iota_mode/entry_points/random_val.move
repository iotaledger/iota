// invalid Random by value

module a::m {
    public entry fun no_random_val(_: iota::random::Random) {
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
