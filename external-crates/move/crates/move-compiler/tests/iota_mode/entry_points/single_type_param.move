module a::m {
    use iota::tx_context;

    public entry fun foo<T>(_: T, _: &mut tx_context::TxContext) {
        abort 0
    }

}

module iota::tx_context {
    struct TxContext has drop {}
}
