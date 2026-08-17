// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_sdk_types::{ObjectId, ObjectVersion};
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
    let read_objects = ObjectSet::from_iter([child.clone()]);
    let modified_at = vec![ObjectVersion::new(child.id(), child.version())];

    let superseded = build_superseded(&modified_at, &input_objects, &read_objects);

    assert_eq!(superseded, vec![(child_key, child)]);
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
