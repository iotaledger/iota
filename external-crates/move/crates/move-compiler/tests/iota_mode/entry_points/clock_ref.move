// valid, Clock by immutable reference

module a::m {
    public entry fun yes_clock_ref(_: &iota::clock::Clock) {
        abort 0
    }
}

module iota::clock {
    struct Clock has key {
        id: iota::object::UID,
    }
}

module iota::object {
    struct UID has store {
        id: address,
    }
}
