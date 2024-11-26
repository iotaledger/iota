module ctf::pizza {

    public struct Pizza has key, store {
        id: UID,
        olive_oils: u16,
        yeast: u16, 
        flour: u16,
        water: u16,
        salt: u16,
        tomato_sauce: u16,
        cheese: u16,
        pineapple: u16,
    }

    public struct PizzaBox has key, store {
        id: UID,
        pizza: Pizza,
    }

    #[allow(lint(self_transfer))]
    public fun cook(olive_oils: u16, yeast: u16, flour: u16, water: u16, salt: u16, tomato_sauce: u16, cheese: u16, pineapple: u16, ctx: &mut tx_context::TxContext) {
        let sender = tx_context::sender(ctx);

        let p = Pizza {
            id: object::new(ctx),
            olive_oils,
            yeast,
            flour,
            water,
            salt,
            tomato_sauce,
            cheese,
            pineapple,
        };
    
        transfer::public_transfer(PizzaBox { id: object::new(ctx), pizza: p }, sender);
    }

    public fun has_pineapple_traces(box: &PizzaBox):bool {
        box.pizza.pineapple > 0
    }

    public fun recycle_box(box: PizzaBox) {
        let PizzaBox {id, pizza} = box;
        let Pizza {id: pid, olive_oils: _, yeast: _, flour: _, water: _, salt: _, tomato_sauce: _, cheese: _, pineapple: _} = pizza;

        object::delete(pid);
        object::delete(id);
    }
}
