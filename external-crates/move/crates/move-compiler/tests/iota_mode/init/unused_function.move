// init is unused but does not error because we are in IOTA mode
module a::m {
    fun init(_: &mut iota::tx_context::TxContext) {}
}

module iota::tx_context {
    struct TxContext has drop {}
}
