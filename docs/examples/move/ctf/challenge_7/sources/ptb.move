module ctf::ptb {

    public struct Flour has copy, drop {}
    public struct Water has copy, drop {}
    public struct Yeast has copy, drop {}
    public struct Salt has copy, drop {}
    public struct Dough has copy, drop {}

    public struct Flag has key, store {
        id: UID,
        user: address
    }

    public fun get_ingredients(): (Flour, Water, Yeast, Salt) {
        (Flour{}, Water{}, Yeast{}, Salt{})
    }

    public fun make_dough(_: Flour, _: Water, _: Yeast, _: Salt): Dough {
        Dough{}
    }

    #[allow(lint(self_transfer))]
    public fun get_flag(_: Dough,  ctx: &mut TxContext) {
        transfer::public_transfer(Flag {
            id: object::new(ctx),
            user: tx_context::sender(ctx)
        }, tx_context::sender(ctx));
    }
}
