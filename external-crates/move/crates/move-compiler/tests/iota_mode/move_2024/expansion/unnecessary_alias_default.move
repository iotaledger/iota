module iota::object {
    public struct ID()
    public struct UID()
}
module iota::transfer {}
module iota::tx_context {
    public struct TxContext()
}

module a::m {
    use iota::object::{Self, ID, UID};
    use iota::transfer;
    use iota::tx_context::{Self, TxContext};
}
