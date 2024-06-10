// invalid, one-time witness type candidate used in a different module

module a::n {
    use iota::iota;
    use iota::tx_context;

    fun init(_otw: iota::IOTA, _ctx: &mut tx_context::TxContext) {
    }

}


module iota::tx_context {
    struct TxContext has drop {}
}

module iota::iota {
    struct IOTA has drop {}
}
