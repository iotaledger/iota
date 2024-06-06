// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module timelock::labeler {

    use std::string::{Self, String};

    use iota::types;

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
        assert!(types::is_one_time_witness(&witness), ENotOneTimeWitness);

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

    /// Return a fully qualified type name with the original package IDs
    /// that is used as type related a label value.
    public(package) fun type_name<L>(): String {
        string::from_ascii(std::type_name::get_with_original_ids<L>().into_string())
    }
}
