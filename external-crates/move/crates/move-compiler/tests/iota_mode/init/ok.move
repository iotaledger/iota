// valid init function
module a::m {
    use iota::tx_context;
    fun init(_: &mut tx_context::TxContext) {
    }
}

module iota::tx_context {
    struct TxContext has drop {}
}
