module examples::item {
    use std::string::String;

    /// Type representing the capability to create new `Item`s.
    public struct AdminCap has key { id: UID }

    /// Custom NFT-like type representing an item.
    public struct Item has key, store { id: UID, name: String }

    /// Module initializer, called once during the module's deployment.
    /// This function creates a single instance of `AdminCap` and assigns it to the publisher.
    fun init(ctx: &mut TxContext) {
        transfer::transfer(AdminCap {
            id: object::new(ctx)
        }, tx_context::sender(ctx))
    }

    /// Function to create a new `Item`. It requires `AdminCap` to authorize the action.
    public fun create_item(_: &AdminCap, name: String, ctx: &mut TxContext): Item {
        let item = Item {
            id: object::new(ctx),
            name,
        };
        item
    }
}