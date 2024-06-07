// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::language_storage::StructTag;

#[cfg(test)]
#[path = "../unit_tests/timelock/label_tests.rs"]
mod label_tests;

/// Get the string label representation.
pub fn label_struct_tag_to_string(label_tag: StructTag) -> String {
    let with_prefix = false;

    label_tag.to_canonical_string(with_prefix)
}
