module conventions::object {

    public struct Object has key, store {
        id: UID
    }

    public fun new(ctx:&mut TxContext): Object {
        Object {
            id: object::new(ctx)
        }
    }
}
