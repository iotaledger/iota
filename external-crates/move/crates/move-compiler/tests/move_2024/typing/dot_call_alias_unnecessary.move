// Test: import used only via dot-call should warn
module a::m {
    public struct X has drop { val: u64 }

    public fun new(): X { X { val: 0 } }

    public fun consume(_self: X) {}

    public fun borrow(_self: &X) {}
}

// Case 1: import used only with dot notation — should warn
module b::dot_only {
    use a::m::{new, consume, borrow};

    fun t() {
        let x = new();
        x.borrow();
        x.consume();
    }
}

// Case 2: import used as a normal function call — should NOT warn
module b::path_only {
    use a::m::{new, consume};

    fun t() {
        let x = new();
        consume(x);
    }
}

// Case 3: import used both ways — should NOT warn
module b::both_uses {
    use a::m::{new, consume, borrow};

    fun t() {
        let x = new();
        x.borrow();
        consume(x);
    }
}

// Case 4: aliased import used only via dot-call — should NOT warn
// (the alias name wouldn't be available via global scope)
module b::aliased {
    use a::m::{new, consume as eat};

    fun t() {
        let x = new();
        x.eat();
    }
}
