module conventions::wallet {

    use iota::object::UID;

    public struct Wallet has key, store {
        id: UID,
        amount: u64
    }
}

module conventions::claw_back_wallet {

    use iota::object::UID;

    public struct Wallet has key {
        id: UID,
        amount: u64
    }
}
