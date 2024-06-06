// init cannot have return values
module a::m {
    use iota::tx_context;
    fun init(_: &mut tx_context::TxContext): u64 {
        0
    }
}

module iota::tx_context {
    struct TxContext has drop {}
}
