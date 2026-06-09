module ctf::recycle {
    use iota::transfer::Receiving;
    use iota::dynamic_field as df;
    use ctf::pizza::{Self, PizzaBox};

    const ENoPizzaBoxProvidedEver: u64 = 1;
    const ENotEnoughRecycledYet: u64 = 2;
    const ENoRecyclingNeededJustEatIt: u64 = 3;

    public struct PizzaBoxRecycler has key {
        id: object::UID
    }

    public struct PizzaPointBalance has store {
        amount: u64
    }
    
    public struct Flag has key, store {
        id: UID,
        user: address
    }

    fun init (ctx: &mut TxContext) {
        transfer::share_object(
            PizzaBoxRecycler {
                id: object::new(ctx)
            }
        );
    }

    public fun accept_box(recycler: &mut PizzaBoxRecycler, disposed: Receiving<PizzaBox>, ctx: &mut TxContext) {
        let sender = tx_context::sender(ctx);
        let box = transfer::public_receive(&mut recycler.id, disposed);

        assert!(pizza::has_pineapple_traces(&box), ENoRecyclingNeededJustEatIt);

        if (df::exists_(&recycler.id, sender)) {
            let balance: &mut PizzaPointBalance = df::borrow_mut(&mut recycler.id, sender);
            balance.amount = balance.amount + 1
        } else {
            df::add(&mut recycler.id, sender, PizzaPointBalance { amount: 1 });
        };
        
        pizza::recycle_box(box);
    }

    #[allow(lint(self_transfer))]
    public fun get_flag(recycler: &mut PizzaBoxRecycler, ctx: &mut TxContext) {
        let sender = tx_context::sender(ctx);
        // Make sure what we are withdrawing exists
        assert!(df::exists_(&recycler.id, sender), ENoPizzaBoxProvidedEver);
        let balance: &mut PizzaPointBalance = df::borrow_mut(&mut recycler.id, sender);
        assert!(balance.amount > 2, ENotEnoughRecycledYet);
        transfer::public_transfer(Flag {
            id: object::new(ctx),
            user: tx_context::sender(ctx)
        }, tx_context::sender(ctx));
    }
}
