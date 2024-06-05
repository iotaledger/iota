module a::beep {
    struct BEEP has drop {
        f0: u64,
        f1: bool,
    }
    fun init(_ctx: &mut iota::tx_context::TxContext) {
    }
}

module iota::tx_context {
    struct TxContext has drop {}
}
