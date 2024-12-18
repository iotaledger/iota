// invalid, no one-time witness type parameter in init

module a::m {
    use iota::tx_context;

    struct M has drop { dummy: bool }

    fun init(_ctx: &mut tx_context::TxContext) {
    }
}

module iota::tx_context {
    struct TxContext has drop {}
}
