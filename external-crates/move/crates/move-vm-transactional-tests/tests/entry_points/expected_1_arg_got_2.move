//# run --args 1 2
// should fail, extra arg
module 0x42::m {
fun main(_x: u64) {}
}
