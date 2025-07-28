// invalid, first field of an object must be iota::object::UID
module a::m {
    struct S has key {
        flag: bool
    }
}
