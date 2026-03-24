// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::tx_context_tests;

#[test]
fun test_id_generation() {
    let mut ctx = tx_context::dummy();
    assert!(ctx.get_ids_created() == 0);

    let id1 = object::new(&mut ctx);
    let id2 = object::new(&mut ctx);

    // new_id should always produce fresh ID's
    assert!(&id1 != &id2);
    assert!(ctx.get_ids_created() == 2);
    id1.delete();
    id2.delete();
}

#[test]
fun test_tx_context_returned_refs_consistency() {
    use std::unit_test::assert_eq;

    let digest1 = b"11111111111111111111111111111111";
    let digest2 = b"22222222222222222222222222222222";

    let ctx1 = tx_context::new(@0x0, digest1, 0, 0, 0);

    // check the digest
    let digest_ref1 = ctx1.digest();
    assert_eq!(*digest_ref1, digest1);

    let ctx2 = tx_context::new(@0x0, digest2, 0, 0, 0);

    // the digest should be updated for the new instance
    assert_eq!(*ctx2.digest(), digest2);
    // and for the previous instance as well, since the digest is stored in a global variable
    assert_eq!(*ctx1.digest(), digest2);

    // the created reference to the first digest should still be valid and unchanged
    assert_eq!(*digest_ref1, digest1);
}
