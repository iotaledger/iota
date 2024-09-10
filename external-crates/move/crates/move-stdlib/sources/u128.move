// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[defines_primitive(u128)]
module std::u128 {
    /// Return the larger of `x` and `y`
    public fun max(x: u128, y: u128): u128 {
        std::macros::num_max!(x, y)
    }

    /// Return the smaller of `x` and `y`
    public fun min(x: u128, y: u128): u128 {
        std::macros::num_min!(x, y)
    }

    /// Return the absolute value of x - y
    public fun diff(x: u128, y: u128): u128 {
        std::macros::num_diff!(x, y)
    }

    /// Calculate x / y, but round up the result.
    public fun divide_and_round_up(x: u128, y: u128): u128 {
        std::macros::num_divide_and_round_up!(x, y)
    }

    /// Return the value of a base raised to a power
    public fun pow(base: u128, exponent: u8): u128 {
        std::macros::num_pow!(base, exponent)
    }

    /// Get a nearest lower integer Square Root for `x`. Given that this
    /// function can only operate with integers, it is impossible
    /// to get perfect (or precise) integer square root for some numbers.
    ///
    /// Example:
    /// ```
    /// math::sqrt(9) => 3
    /// math::sqrt(8) => 2 // the nearest lower square root is 4;
    /// ```
    ///
    /// In integer math, one of the possible ways to get results with more
    /// precision is to use higher values or temporarily multiply the
    /// value by some bigger number. Ideally if this is a square of 10 or 100.
    ///
    /// Example:
    /// ```
    /// math::sqrt(8) => 2;
    /// math::sqrt(8 * 10000) => 282;
    /// // now we can use this value as if it was 2.82;
    /// // but to get the actual result, this value needs
    /// // to be divided by 100 (because sqrt(10000)).
    ///
    ///
    /// math::sqrt(8 * 1000000) => 2828; // same as above, 2828 / 1000 (2.828)
    /// ```
    public fun sqrt(x: u128): u128 {
        std::macros::num_sqrt!<u128, u256>(x, 128)
    }

    /// Loops applying `$f` to each number from `$start` to `$stop` (exclusive)
    public macro fun range_do($start: u128, $stop: u128, $f: |u128|) {
        std::macros::range_do!($start, $stop, $f)
    }

    /// Loops applying `$f` to each number from `$start` to `$stop` (inclusive)
    public macro fun range_do_eq($start: u128, $stop: u128, $f: |u128|) {
        std::macros::range_do_eq!($start, $stop, $f)
    }

    /// Loops applying `$f` to each number from `0` to `$stop` (exclusive)
    public macro fun do($stop: u128, $f: |u128|) {
        std::macros::do!($stop, $f)
    }

    /// Loops applying `$f` to each number from `0` to `$stop` (inclusive)
    public macro fun do_eq($stop: u128, $f: |u128|) {
        std::macros::do_eq!($stop, $f)
    }
}
