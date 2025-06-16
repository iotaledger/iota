// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module 0x42::m {
    fun test_04<T: drop>(x: Maybe<T>, other: Maybe<T>): Maybe<T> {
        match (x) {
            (x @ Maybe::Nothing) | (x @ Maybe::Just(_)) => other,
        }
    }
}
