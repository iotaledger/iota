module ctf::counter {

    const MaxCounter: u64 = 1000;
    const ENoAttemptLeft: u64 = 0;
    
    /// A shared counter.
    public struct Counter has key {
        id: UID,
        owner: address,
        value: u64
    }

    /// Create and share a Counter object.
    public(package) fun create_counter(ctx: &mut TxContext) {
        transfer::share_object(Counter {
            id: object::new(ctx),
            owner: tx_context::sender(ctx),
            value: 0
        })
    }

    public fun owner(counter: &Counter): address {
        counter.owner
    }

    public fun value(counter: &Counter): u64 {
        counter.value
    }

    /// Increment a counter by 1.
    public fun increment(counter: &mut Counter) {
        counter.value = counter.value + 1;
    }

    /// Set value (only runnable by the Counter owner)
    public entry fun set_value(counter: &mut Counter, value: u64, ctx: &TxContext) {
        assert!(counter.owner == tx_context::sender(ctx), 0);
        counter.value = value;
    }

    /// Check whether the counter has reached the limit.
    public fun is_within_limit(counter: &mut Counter) {
        assert!(counter.value <= MaxCounter, ENoAttemptLeft);
    }
}
