// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// `#[view]` functions that return their input unchanged, used to exercise the
/// argument-resolution and value-serialization layers of the
/// `ViewFunctionCalls` endpoint across a range of Move types, plus a couple of
/// functions that always fail (an abort and a non-`#[view]` function).
module view_demo::echo {
    /// A non-object struct with a nested `vector<u8>`, to exercise struct
    /// serialization to and from both BCS and JSON.
    public struct Pair has copy, drop, store {
        u: u128,
        b: vector<u8>,
    }

    #[view]
    public fun echo_u8(x: u8): u8 { x }

    #[view]
    public fun echo_u32(x: u32): u32 { x }

    #[view]
    public fun echo_u64(x: u64): u64 { x }

    #[view]
    public fun echo_u128(x: u128): u128 { x }

    #[view]
    public fun echo_u256(x: u256): u256 { x }

    #[view]
    public fun echo_bytes(x: vector<u8>): vector<u8> { x }

    #[view]
    public fun echo_pair(x: Pair): Pair { x }

    /// Always aborts: exercises the execution-error output path.
    #[view]
    public fun always_aborts(): u64 { abort 7 }

    /// A public function without the `#[view]` attribute: exercises the
    /// server-side rejection path.
    public fun not_view(): u64 { 0 }
}
