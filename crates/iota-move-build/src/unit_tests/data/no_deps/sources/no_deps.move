module no_deps::no_deps;

public struct Foo has copy, drop {
    value: u64,
}

public enum Bar has copy, drop {
    Empty,
    Value(u64),
}

public fun make_foo(value: u64): Foo {
    Foo { value }
}

public fun make_bar(value: u64): Bar {
    Bar::Value(value)
}

public fun foo_value(foo: &Foo): u64 {
    foo.value
}
