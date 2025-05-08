// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module a::b {
    fun f() {
        a < *b && !c || (*&d == true);
    }
}
