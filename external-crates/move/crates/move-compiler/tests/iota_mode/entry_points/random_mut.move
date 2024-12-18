// invalid Random by mutable reference

module a::m {
    public entry fun no_random_mut(_: &mut iota::random::Random) {
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
