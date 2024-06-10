// invalid, otw type packed

module a::m {
    use iota::tx_context;

    struct M has drop { dummy: bool }

    fun init(_otw: M, _ctx: &mut tx_context::TxContext) {
    }

    fun pack(): M {
        M { dummy: false }
    }
}

module iota::tx_context {
    struct TxContext has drop {}
}
