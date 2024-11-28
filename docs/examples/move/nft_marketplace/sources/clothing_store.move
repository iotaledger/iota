/// Module provides `mock` items for using them in marketplace and rental extensions.
#[allow(lint(self_transfer))]
module nft_marketplace::clothing_store {
    use iota::package;
    /// One Time Witness.
    public struct CLOTHING_STORE has drop {}


    fun init(otw: CLOTHING_STORE, ctx: &mut TxContext) {
        package::claim_and_keep(otw, ctx)
    }

    public struct TShirt has key, store {
        id: UID,
    }

    public struct Jacket has key, store {
        id: UID,
    }

    public struct Shoes has key, store {
        id: UID,
    }

    public struct Jeans has key, store {
        id: UID,
    }

    public fun new_tshirt(ctx: &mut TxContext) {
        let tshirt = TShirt {
            id: object::new(ctx),
        };

        transfer::public_transfer(tshirt, ctx.sender());
    }

    public fun new_jeans(ctx: &mut TxContext) {
        let jeans = Jeans {
            id: object::new(ctx),
        };

        transfer::public_transfer(jeans, ctx.sender());
    }

    public fun new_shoes(ctx: &mut TxContext) {
        let shoes = Shoes {
            id: object::new(ctx),
        };

        transfer::public_transfer(shoes, ctx.sender());
    }

    public fun new_jacket(ctx: &mut TxContext) {
        let jacket = Jacket {
            id: object::new(ctx),
        };

        transfer::public_transfer(jacket, ctx.sender());
    }
}