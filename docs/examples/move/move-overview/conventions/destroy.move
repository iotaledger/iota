
module conventions::wallet {

    use iota::balance::{Self, Balance};
    use iota::iota::IOTA;

    public struct Wallet<Value> has key, store {
        id: UID,
        value: Value
    }

    // Value has drop
    public fun drop<Value: drop>(self: Wallet<Value>) {
        let Wallet { id, value: _ } = self;
        object::delete(id);
    }

    // Value doesn't have drop
    // Throws if the `wallet.value` is not empty.
    public fun destroy_empty(self: Wallet<Balance<IOTA>>) {
        let Wallet { id, value } = self;
        object::delete(id);
        balance::destroy_zero(value);
    }
}
