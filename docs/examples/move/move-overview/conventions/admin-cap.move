module conventions::social_network {

    use std::string::String;

    public struct Account has key {
        id: UID,
        name: String
    }

    public struct Admin has key {
        id: UID,
    }

    // ✅ Correct
    // cap.update(&mut account, b"jose");
    public fun update(_: &Admin, account: &mut Account, new_name: String) {
        // Implementation omitted.
        abort(0)
    }

    // ❌ Incorrect
    // account.update(&cap, b"jose");
    public fun set(account: &mut Account, _: &Admin, new_name: String) {
        // Implementation omitted.
        abort(0)
    }
}
