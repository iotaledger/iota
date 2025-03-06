// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module iota::test_label_one;

use iota::labeler;

/// Name of the label.
public struct TEST_LABEL_ONE has drop {}

/// Create and transfer a `LabelerCap` object to an authority address.
public fun assign_labeler_cap(to: address, ctx: &mut TxContext) {
    // Test one time witness.
    let witness = TEST_LABEL_ONE {};

    // Create a new capability.
    let cap = labeler::create_labeler_cap(witness, ctx);

    // Transfer the capability to the specified address.
    transfer::public_transfer(cap, to);
}
