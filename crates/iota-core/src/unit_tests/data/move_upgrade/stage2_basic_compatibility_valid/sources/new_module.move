// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module base_addr::new_module {

    struct MyObject has key, store {
        id: iota::object::UID,
        data: u64
    }

    public fun this_is_a_new_module() { }

    public fun i_can_call_funs_in_other_modules_that_already_existed(): u64 {
        base_addr::friend_module::friend_call()
    }
}
