// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

/// Trait for checking field presence/absence
pub(crate) trait FieldPresenceChecker {
    /// Returns a list of all top-level field names for this type.
    fn top_level_fields(&self) -> &[&'static str];

    /// Check if a specific top-level field is present (no dots allowed).
    ///
    /// Returns:
    ///   - None: field name is invalid (doesn't exist on this type)
    ///   - Some((true, Some(checker))): field is present and has nested fields
    ///   - Some((true, None)): field is present without nested fields
    ///   - Some((false, _)): field exists but is absent (None)
    fn check_field_presence(
        &self,
        field: &str,
    ) -> Option<(bool, Option<&dyn FieldPresenceChecker>)>;
}

/// Macro to automatically implement FieldPresenceChecker for a protobuf message
/// type
///
/// This macro generates code that can check which fields are present/absent.
///
/// # Usage
/// ```ignore
/// impl_field_presence_checker!(MyMessage {
///     field1,               // simple field (string, int, etc.)
///     field2,               // another simple field
///     nested: NestedType,   // nested message that can be recursed into
/// });
/// ```
#[macro_export]
macro_rules! impl_field_presence_checker {
    // Main rule: matches the syntax `Type { field1, field2: NestedType, ... }`
    ($type:ty { $( $field:ident $( : $nested_type:ty )? ),* $(,)? }) => {
        // Generate the trait implementation for the given type
        impl $crate::utils::FieldPresenceChecker for $type {
            fn top_level_fields(&self) -> &[&'static str] {
                &[ $( stringify!($field) ),* ]  // stringify! turns `field1` into "field1"
            }

            fn check_field_presence(&self, field: &str) -> Option<(bool, Option<&dyn $crate::utils::FieldPresenceChecker>)> {
                match field {
                    // For each field in the macro input, generate a match arm
                    $(
                        stringify!($field) => {
                            // Call the helper rule to check this field
                            // If $nested_type is present, it passes it; otherwise doesn't
                            $crate::impl_field_presence_checker!(@field_check self, $field $(, $nested_type)?)
                        }
                    )*
                    // Field name doesn't match any known field
                    _ => None,
                }
            }
        }
    };

    // Helper rule for nested fields (when `: Type` is specified)
    (@field_check $self:ident, $field:ident, $nested_type:ty) => {{
        // Check if the field is Some (present) or None (absent)
        let present = $self.$field.is_some();

        // If nested field is present, provide a reference to it as a FieldPresenceChecker
        let nested = $self.$field.as_ref().map(|f| f as &dyn $crate::utils::FieldPresenceChecker);

        Some((present, nested))
    }};

    // Helper rule for simple fields (when no `: Type` is specified)
    (@field_check $self:ident, $field:ident) => {
        // Just check if the field is present; no nested checker needed
        Some(($self.$field.is_some(), None))
    };
}

/// Assert field presence/absence for any type implementing
/// FieldPresenceChecker. This function validates that an object contains
/// exactly the fields specified (or their absence). It also supports nested
/// field paths using dot notation (e.g., "reference.object_id").
///
/// # Example
/// ```ignore
/// assert_field_presence(
///     &object,
///     &["reference.object_id", "reference.version", "bcs"],
///     "test scenario"
/// );
/// ```
/// This checks:
/// - `reference` is present (inferred because reference.* are listed)
/// - `reference.object_id` is present
/// - `reference.version` is present
/// - `bcs` is present
/// - All other fields at the top level are absent
/// - All other fields inside `reference` are absent (like `reference.digest`)
pub(crate) fn assert_field_presence(
    checker: &dyn FieldPresenceChecker,
    expected_field_paths: &[&str],
    scenario: &str,
) {
    let mut expected_nested_field_paths: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut expected_non_nested_field_paths: HashSet<&str> = HashSet::new();
    let mut expected_top_level_fields: HashSet<&str> = HashSet::new();

    for expected_field_path in expected_field_paths {
        if let Some((top_level_field, remaining_path)) = expected_field_path.split_once('.') {
            expected_nested_field_paths
                .entry(top_level_field)
                .or_default()
                .push(remaining_path);
            expected_top_level_fields.insert(top_level_field);
        } else {
            expected_non_nested_field_paths.insert(expected_field_path);
            expected_top_level_fields.insert(expected_field_path);
        }
    }

    let actual_top_level_fields: HashSet<&str> =
        checker.top_level_fields().iter().copied().collect();

    // Validate that all expected fields exist on this type
    for expected_top_level_field in &expected_top_level_fields {
        assert!(
            actual_top_level_fields.contains(expected_top_level_field),
            "Invalid field '{}' in {scenario}: field does not exist on this type",
            expected_top_level_field
        );
    }

    // Check each field at this level for correct presence/absence
    for top_level_field in actual_top_level_fields.clone() {
        let should_be_present = expected_top_level_fields.contains(top_level_field);

        let (is_present, _) = checker
            .check_field_presence(top_level_field)
            .unwrap_or_else(|| panic!("Invalid field '{top_level_field}' in {scenario}"));

        assert_eq!(
            is_present, should_be_present,
            "Field '{top_level_field}' in {scenario}: expected {should_be_present}, got {is_present}"
        );
    }

    // Check that no field is specified both as nested and non-nested
    for non_nested_field in &expected_non_nested_field_paths {
        if expected_nested_field_paths.contains_key(non_nested_field) {
            panic!(
                "Contradictory field paths in {scenario}: '{non_nested_field}' specified both as non-nested (implying no nested fields) and with nested fields ({})",
                expected_nested_field_paths[non_nested_field]
                    .iter()
                    .map(|s| format!("{}.{}", non_nested_field, s))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    // Recurse into nested fields
    for top_level_field in &actual_top_level_fields {
        // Recurse only if there is a nested checker for this field
        if let Some((_, Some(nested_checker))) = checker.check_field_presence(top_level_field) {
            let expected_field_paths_nested = expected_nested_field_paths
                .get(top_level_field)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);

            // Recurse into this nested field
            assert_field_presence(
                nested_checker,
                expected_field_paths_nested,
                &format!("{scenario}.{top_level_field}"),
            );
        }
    }
}
