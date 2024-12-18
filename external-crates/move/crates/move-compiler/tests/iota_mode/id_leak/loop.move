// allowed, even though a bit pointless
module a::m {
    use iota::object::{Self, UID};
    use iota::tx_context::{Self, TxContext};
    use iota::transfer::transfer;

    struct Obj has key {
        id: UID
    }

    public entry fun transmute(ctx: &mut TxContext) {
        let i = 0;
        let id = object::new(ctx);
        while (i <= 10) {
            object::delete(id);
            id = object::new(ctx);
            i = i + 1;
        };
        let obj = Obj { id };
        transfer(obj, tx_context::sender(ctx))
    }

}

module iota::object {
    struct UID has store {
        id: address,
    }
    public fun new(_: &mut iota::tx_context::TxContext): UID {
        abort 0
    }
    public fun delete(_: UID) {
        abort 0
    }
}

module iota::tx_context {
    struct TxContext has drop {}
    public fun sender(_: &TxContext): address {
        @0
    }
}

module iota::transfer {
    public fun transfer<T: key>(_: T, _: address) {
        abort 0
    }
}
