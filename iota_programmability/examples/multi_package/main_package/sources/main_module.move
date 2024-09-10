// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module main_package::main_module {
    use dep_package::dep_module;

    fun foo(): u64 {
        dep_module::foo()
    }

}
