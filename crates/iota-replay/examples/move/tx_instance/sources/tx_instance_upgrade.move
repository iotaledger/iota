module tx_instance::tx_instance_upgrade {
    use iota::event;
    use iota::tx_context::{Self, TxContext};

    struct TxInstance has copy, drop {
        user: address,
        published: bool,
        name: vector<u8>
    }

    fun init(ctx: &mut TxContext) {
        event::emit(TxInstance {
            user: tx_context::sender(ctx),
            published: true,
            name: b"TxInstance"
        })
    }
}
