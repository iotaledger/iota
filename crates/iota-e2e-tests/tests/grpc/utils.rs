// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Trait for checking if fields are present in protobuf messages
///
/// This lets us check which optional fields have values (Some) vs are empty
/// (None)
pub(crate) trait FieldPresenceChecker {
    /// Returns a list of all possible field names for this type
    /// Example: For a message with fields "id" and "name", returns ["id",
    /// "name"]
    fn all_fields(&self) -> &[&'static str];

    /// Check if a single field is present
    ///
    /// Input: field name like "reference" (no dots allowed)
    /// Returns:
    ///   - None: field name is invalid (doesn't exist on this type)
    ///   - Some((true, Some(checker))): field is present and has nested fields
    ///     we can check
    ///   - Some((true, None)): field is present but is a simple value (no
    ///     nesting)
    ///   - Some((false, _)): field is absent (None)
    fn check_field(&self, field: &str) -> Option<(bool, Option<&dyn FieldPresenceChecker>)>;
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
///
/// # What it generates
/// For each field, it checks if `self.field.is_some()` (protobuf optional
/// fields are Option<T>) For nested fields with `: Type`, it also provides the
/// nested checker so you can recurse
#[macro_export]
macro_rules! impl_field_presence_checker {
    // Main rule: matches the syntax `Type { field1, field2: NestedType, ... }`
    ($type:ty { $( $field:ident $( : $nested_type:ty )? ),* $(,)? }) => {
        // Generate the trait implementation for the given type
        impl $crate::utils::FieldPresenceChecker for $type {
            // Return all field names as a static array
            fn all_fields(&self) -> &[&'static str] {
                &[ $( stringify!($field) ),* ]  // stringify! turns `field1` into "field1"
            }

            // Check a single field by name
            fn check_field(&self, field: &str) -> Option<(bool, Option<&dyn $crate::utils::FieldPresenceChecker>)> {
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
    // This rule matches when $nested_type is present
    (@field_check $self:ident, $field:ident, $nested_type:ty) => {{
        // Check if the field is Some (present) or None (absent)
        let present = $self.$field.is_some();

        // If present, provide a reference to it as a FieldPresenceChecker
        // This allows recursion into nested fields
        let nested = $self.$field.as_ref().map(|f| f as &dyn $crate::utils::FieldPresenceChecker);

        Some((present, nested))
    }};

    // Helper rule for simple fields (when no `: Type` is specified)
    // This rule matches when $nested_type is NOT present
    (@field_check $self:ident, $field:ident) => {
        // Just check if the field is present; no nested checker needed
        Some(($self.$field.is_some(), None))
    };
}

/// Assert nested field masks - validate presence and absence of nested fields
///
/// This validates field masks that can include nested paths like
/// "reference.object_id"
///
/// # Arguments
/// * `object` - The protobuf message to check
/// * `field_paths` - List of field paths that should be present (can include
///   dots)
/// * `scenario` - Test scenario name (for error messages)
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
pub(crate) fn assert_field_presence<T>(object: &T, field_paths: &[&str], scenario: &str)
where
    T: iota_grpc_types::field::MessageFields + FieldPresenceChecker,
{
    // Start checking from the top level
    check_level(object, object.all_fields(), field_paths, scenario);
}

/// Internal recursive function to check field presence at each nesting level
///
/// This is called recursively for each level of nesting.
///
/// # How it works
/// 1. Extract field names expected at THIS level (before the first dot)
/// 2. Check that all fields at this level are present/absent as expected
/// 3. Group remaining paths by their parent field
/// 4. Recursively check nested levels
///
/// # Example
/// If field_paths is ["reference.object_id", "reference.version", "bcs"]:
/// 1. At top level: expects "reference" and "bcs" present, all others absent
/// 2. Recurses into "reference" with paths ["object_id", "version"]
/// 3. Inside "reference": expects "object_id" and "version" present, all others
///    absent
fn check_level(
    checker: &dyn FieldPresenceChecker,
    all_fields: &[&'static str],
    paths: &[&str],
    scenario: &str,
) {
    use std::collections::{HashMap, HashSet};

    // Extract field names expected at THIS level (before first '.')
    // Example: ["reference.object_id", "bcs"] -> {"reference", "bcs"}
    let expected_this_level: HashSet<&str> = paths
        .iter()
        .map(|path| path.split('.').next().unwrap())
        .collect();

    // Validate that all expected fields are valid (exist in all_fields)
    let valid_fields: HashSet<&str> = all_fields.iter().copied().collect();
    for expected_field in &expected_this_level {
        assert!(
            valid_fields.contains(expected_field),
            "Invalid field '{}' in {scenario}: field does not exist on this type",
            expected_field
        );
    }

    // Check each field at this level for correct presence/absence
    for field in all_fields {
        // Should this field be present?
        let should_be_present = expected_this_level.contains(field);

        // Is this field actually present?
        let (is_present, _) = checker
            .check_field(field)
            .unwrap_or_else(|| panic!("Invalid field '{field}' in {scenario}"));

        // Verify expectation matches reality
        assert_eq!(
            is_present, should_be_present,
            "Field '{field}' in {scenario}: expected {should_be_present}, got {is_present}"
        );
    }

    // Group nested paths by their parent field
    // Example: ["reference.object_id", "reference.version"]
    //       -> {"reference": ["object_id", "version"]}
    let mut nested: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut standalone_fields: HashSet<&str> = HashSet::new();

    for path in paths {
        if let Some((field, rest)) = path.split_once('.') {
            // This path has nesting - add the remaining part to the group
            nested.entry(field).or_default().push(rest);
        } else {
            // This is a standalone field (no dot) - track it
            standalone_fields.insert(path);
        }
    }

    // Validate no contradictory paths
    // A field cannot be specified both standalone AND with nested paths
    // Example: ["reference", "reference.object_id"] is contradictory because:
    //   - "reference" alone means: all nested fields should be absent
    //   - "reference.object_id" means: object_id should be present
    for field in &standalone_fields {
        if nested.contains_key(field) {
            panic!(
                "Contradictory field paths in {scenario}: '{}' specified both standalone (implying no nested fields) and with nested paths ({})",
                field,
                nested[field]
                    .iter()
                    .map(|s| format!("{}.{}", field, s))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    // Recursively check nested fields
    // IMPORTANT: We must recurse into ALL present fields (not just those with
    // nested paths) to verify their nested children are absent when not
    // specified
    for field in &expected_this_level {
        // Get the nested checker for this field
        if let Some((_, Some(nested_checker))) = checker.check_field(field) {
            // Get sub-paths for this field, or empty slice if none specified
            // Empty slice means ALL nested fields should be absent
            let sub_paths = nested.get(field).map(|v| v.as_slice()).unwrap_or(&[]);

            // Recurse into this nested field
            check_level(
                nested_checker,
                nested_checker.all_fields(),
                sub_paths,
                &format!("{scenario}.{field}"), // Update scenario for better error messages
            );
        }
    }
}
