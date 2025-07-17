/*
/// Module: viewing
module viewing::viewing;
*/

// For Move coding conventions, see
// https://docs.iota.org/developer/iota-101/move-overview/conventions

module viewing::view_fns;

public struct EmptyVoid {

}

// Should be OK
#[view]
public fun ok(): u8 {
    3
}

// Should fail, because of missing return
// #[view]
// public fun no_return(l : u8) {
//     let _r = 3 + l;
// }

// Should fail, because of mutable reference input
// #[view]
// public fun mutator(_void: &mut EmptyVoid): u8 {
//     3
// }