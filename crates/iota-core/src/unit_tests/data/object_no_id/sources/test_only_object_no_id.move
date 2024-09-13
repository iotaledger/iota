// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module object_no_id::test_only_object_no_id {
    #[test_only]
    public struct NotObject has key {f: u64}

    #[test]
    fun bad_share() {
        iota::transfer::share_object(NotObject{f: 42});
    }
}
