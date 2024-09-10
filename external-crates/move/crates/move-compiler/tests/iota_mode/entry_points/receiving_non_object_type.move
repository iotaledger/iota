// invalid, Receiving type with non-object type param

module a::m {
    use iota::transfer::Receiving;

    public entry fun no(_: Receiving<u64>) { abort 0 }
}

module iota::transfer {
    struct Receiving<phantom T: key> has drop {
        id: address
    }
}
