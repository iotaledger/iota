module ctf::checkin {

    public struct Flag has key, store {
        id: UID,
        user: address
    }

    public entry fun get_flag(ctx: &mut TxContext) {
        transfer::public_transfer(Flag {
            id: object::new(ctx),
            user: tx_context::sender(ctx)
        }, tx_context::sender(ctx));
    }
}
