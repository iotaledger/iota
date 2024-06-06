// invalid, Clock by mutable reference

module a::m {
    public entry fun no_clock_mut(_: &mut iota::clock::Clock) {
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
