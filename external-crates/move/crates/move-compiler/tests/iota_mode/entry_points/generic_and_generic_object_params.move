module a::m {
    use iota::object;
    struct Obj<T> has key {
        id: object::UID,
        value: T,
    }

    public entry fun foo<T0: key, T1: store>(_: T0, _: Obj<T1>) {
        abort 0
    }

}

module iota::object {
    struct UID has store {
        id: address,
    }
}