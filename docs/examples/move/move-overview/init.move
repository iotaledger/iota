module examples::one_timer {
    /// The unique capability created during the module's initialization.
    public struct CreatorCapability has key {
        id: UID
    }

    /// This function runs only once upon the module's publication.
    /// It ensures that certain actions, like granting the module's author a unique
    /// `CreatorCapability`, happen just once.
    fun init(ctx: &mut TxContext) {
        transfer::transfer(CreatorCapability {
            id: object::new(ctx),
        }, tx_context::sender(ctx))
    }
}