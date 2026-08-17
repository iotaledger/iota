// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_sdk_types::{Address, ObjectId, ObjectVersion};
use iota_types::{
    object::{Object, ObjectSet},
    storage::ObjectKey,
};

use super::{build_superseded, build_superseded_counting};

/// A version superseded by a mutation of a runtime-loaded object — a
/// dynamic field — is captured from the tracked read objects. Such an
/// object never appears in `input_objects`, which is why sourcing the
/// pre-images from inputs alone silently dropped it.
#[test]
fn test_superseded_captures_runtime_loaded_objects() {
    let child = Object::immutable_with_id_for_testing(ObjectId::random());
    let child_key = ObjectKey(child.id(), child.version());

    // The tracked set holds it; the declared inputs do not.
    let input_objects = BTreeMap::new();
    let mut read_objects = ObjectSet::default();
    read_objects.insert(child.clone());
    let modified_at = vec![ObjectVersion::new(child.id(), child.version())];

    let superseded = build_superseded(&modified_at, &input_objects, &read_objects);

    assert_eq!(superseded, vec![(child_key, child)]);
}

/// When a key is present in both sources, the pre-image comes from
/// `input_objects` rather than `read_objects` — the declared input is
/// checked first, and the tracked read objects are only a fallback for
/// what the inputs don't cover.
#[test]
fn test_superseded_prefers_input_objects_over_read_objects() {
    let id = ObjectId::random();
    let from_input = Object::with_id_owner_gas_for_testing(id, Address::ZERO, 1);
    let from_read = Object::with_id_owner_gas_for_testing(id, Address::ZERO, 2);
    let key = ObjectKey(id, from_input.version());

    let mut input_objects = BTreeMap::new();
    input_objects.insert(id, from_input.clone());
    let mut read_objects = ObjectSet::default();
    read_objects.insert(from_read);
    let modified_at = vec![ObjectVersion::new(id, from_input.version())];

    let superseded = build_superseded(&modified_at, &input_objects, &read_objects);

    assert_eq!(superseded, vec![(key, from_input)]);
}

/// A modified version present in neither source is dropped rather than
/// guessed at, and counted so the gap is visible.
#[test]
fn test_superseded_counts_what_it_cannot_capture() {
    let missing = ObjectId::random();
    let mut misses = 0;

    let superseded = build_superseded_counting(
        &[ObjectVersion::new(missing, 7.into())],
        &BTreeMap::new(),
        &ObjectSet::default(),
        &mut misses,
    );

    assert!(superseded.is_empty());
    assert_eq!(misses, 1);
}
