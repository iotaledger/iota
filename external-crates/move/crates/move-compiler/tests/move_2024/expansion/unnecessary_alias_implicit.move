module a::m {
    public struct S()
    public fun foo() {
        use a::m::S; // unused and duplicate
        use a::m::foo; // unused and duplicate
    }
}
