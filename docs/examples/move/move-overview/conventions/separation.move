module conventions::wallet {
    public struct Wallet has key, store {
        id: UID,
        amount: u64
    }
}

module conventions::claw_back_wallet {
    public struct Wallet has key {
        id: UID,
        amount: u64
    }
}
