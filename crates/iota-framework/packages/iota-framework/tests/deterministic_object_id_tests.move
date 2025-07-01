// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::deterministic_object_id_tests;

use iota::deterministic_object_id;
 #[test]
 fun test_id_generation() {
       let addr = deterministic_object_id::dummy_address();
       let salt = vector[0x12, 0x34, 0xab, 0xcd];
       let id1 = object::new_precomputed(addr, salt);
       let id2 = object::new_precomputed(addr, salt);

       assert!(&id1 == &id2);
       id1.delete();
       id2.delete();
 }

#[test]
 fun test_different_salt_id_generation() {
       let addr = deterministic_object_id::dummy_address();
       let salt1 = vector[0x12, 0x34, 0xab, 0xcd];
       let salt2 = vector[0x56, 0x78, 0xef, 0x90];
       let id1 = object::new_precomputed(addr, salt1);
       let id2 = object::new_precomputed(addr, salt2);

       assert!(&id1 != &id2);
       id1.delete();
       id2.delete();
 }

#[test]
 fun test_different_address_id_generation() {
       let addr1 = deterministic_object_id::dummy_address();
       let addr2 = @0x2;
       let salt = vector[0x12, 0x34, 0xab, 0xcd];
       let id1 = object::new_precomputed(addr1, salt);
       let id2 = object::new_precomputed(addr2, salt);

       assert!(&id1 != &id2);
       id1.delete();
       id2.delete();
 }

 #[test]
 fun test_mixed_salt_address_id_generation() {
       let addr1 = deterministic_object_id::dummy_address();
       let addr2 = @0x2;
       let salt1 = vector[0x12, 0x34, 0xab, 0xcd];
       let salt2 = vector[0x56, 0x78, 0xef, 0x90];
       let id1 = object::new_precomputed(addr1, salt1);
       let id2 = object::new_precomputed(addr2, salt2);

       assert!(&id1 != &id2);
       
       let id3 = object::new_precomputed(addr1, salt2);

       assert!(&id1 != &id3);

       id1.delete();
       id2.delete();
       id3.delete();
 }



