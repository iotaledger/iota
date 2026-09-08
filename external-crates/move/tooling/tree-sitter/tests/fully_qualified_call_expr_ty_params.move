module a::b;

fun f() {
    let x = iota::dynamic_field::borrow<vector<u8>, u64>(&parent, b"");
    let x = ::iota::dynamic_field::borrow<vector<u8>, u64>(&parent, b"");
}
