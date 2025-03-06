// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Defines a LabelerCap used for creating labels in a ``iota::timelock::Timelock`` object.
/// The LabelerCap can be created only be consuming an OTW, making then labels unique for each cap.
module iota::labeler;

/// Error code for when a type passed to the `create_labeler_cap` function is not a one-time witness.
const ENotOneTimeWitness: u64 = 0;

/// `LabelerCap` allows to create labels of the specific type `L`.
/// Can be publicly transferred like any other object.
public struct LabelerCap<phantom L> has key, store {
    id: UID,
}

/// Create a `LabelerCap` instance.
/// Can be created only by consuming a one time witness.
public fun create_labeler_cap<L: drop>(witness: L, ctx: &mut TxContext): LabelerCap<L> {
    assert!(iota::types::is_one_time_witness(&witness), ENotOneTimeWitness);

    LabelerCap<L> {
        id: object::new(ctx),
    }
}

/// Delete a `LabelerCap` instance.
/// If a capability is destroyed, it is impossible to add the related labels.
public fun destroy_labeler_cap<L>(cap: LabelerCap<L>) {
    let LabelerCap<L> {
        id,
    } = cap;

    object::delete(id);
}
