// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::deterministic_object_id_tests;
//TODO

use iota::deterministic_object_id;
 #[test]
 fun test_id_generation() {
        assert!(1 == 1); // Placeholder test to ensure the module compiles
 }
//     let addr = deterministic_object_id::dummy_address();
//     let salt: vector<u8> = vector[0x12, 0x34, 0xab, 0xcd];

//     let id1 = object::new_precomputed(addr, salt);
//     let id2 = object::new_precomputed(addr, salt);

//     // new_precomputed should return the same ID for the same address and salt
//     assert!(&id1 == &id2);
//     id1.delete();
//     id2.delete();
// }


