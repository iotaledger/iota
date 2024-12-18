module ctf::luckynumber {
    use ctf::counter::{Self, Counter};

    fun init(ctx: &mut TxContext) {
        counter::create_counter(ctx);
    }

    public struct Flag has key, store {
        id: UID,
        user: address
    }

    public entry fun get_flag(user_counter: &mut Counter, lucky_num: u64, ctx: &mut TxContext) {
        counter::increment(user_counter);
        counter::is_within_limit(user_counter);

        let _ = lucky_num;
        transfer::public_transfer(Flag {
            id: object::new(ctx),
            user: tx_context::sender(ctx)
        }, tx_context::sender(ctx));
    }
}
