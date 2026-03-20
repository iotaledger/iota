// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

use test_cluster::{TestCluster, TestClusterBuilder};

/// Set up a gRPC test cluster and high-level client with default settings
///
/// # Parameters
/// * `wait_for_checkpoint` - Optional checkpoint to wait for before returning
/// * `client_max_message_size_bytes` - Optional max message size for the client
pub async fn setup_grpc_test(
    wait_for_checkpoint: Option<u64>,
    client_max_message_size_bytes: Option<u32>,
) -> (TestCluster, iota_grpc_client::Client) {
    setup_grpc_test_with_builder(
        |builder| builder,
        wait_for_checkpoint,
        client_max_message_size_bytes,
    )
    .await
}

/// Set up a gRPC test cluster and high-level client with custom builder
///
/// # Parameters
/// * `builder_fn` - Function to customize the TestClusterBuilder
/// * `wait_for_checkpoint` - Optional checkpoint to wait for before returning
/// * `client_max_message_size_bytes` - Optional max message size for the client
pub async fn setup_grpc_test_with_builder<F>(
    builder_fn: F,
    wait_for_checkpoint: Option<u64>,
    client_max_message_size_bytes: Option<u32>,
) -> (TestCluster, iota_grpc_client::Client)
where
    F: FnOnce(TestClusterBuilder) -> TestClusterBuilder,
{
    let builder = TestClusterBuilder::new()
        .with_fullnode_enable_grpc_api(true)
        .disable_fullnode_pruning()
        .with_num_validators(1);

    let test_cluster = builder_fn(builder).build().await;

    if let Some(checkpoint) = wait_for_checkpoint {
        test_cluster.wait_for_checkpoint(checkpoint, None).await;
    }

    let mut client = iota_grpc_client::Client::connect(test_cluster.grpc_url())
        .await
        .expect("Failed to connect to gRPC service");

    if let Some(max_size) = client_max_message_size_bytes {
        client = client.with_max_decoding_message_size(max_size as usize);
    }

    (test_cluster, client)
}

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
///     items: [ItemType],    // repeated field (Vec) that can be recursed into
/// });
/// ```
#[macro_export]
macro_rules! impl_field_presence_checker {
    // Main rule: matches the syntax `Type { field1, field2: NestedType, field3: [Type], ... }`
    ($type:ty { $( $field:ident $( : $nested_type:tt )? ),* $(,)? }) => {
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

    // Transparent type rule:
    //
    // Use this when the proto message has `field_mask_transparent = true`.  In
    // that case the read-mask paths address the *inner* message's fields
    // directly (no wrapper prefix), so the checker must expose those inner
    // fields at the top level and delegate all lookups to the inner instance.
    //
    // Syntax:
    //   impl_field_presence_checker!(OuterType, transparent(inner_field) {
    //       inner_field1,
    //       inner_field2,
    //       ...
    //   });
    //
    // `inner_field` is the name of the `Option<InnerType>` field on `OuterType`.
    // The body lists the *field names* (no type annotations needed) of
    // `InnerType` so the macro can build the static field list and produce the
    // correct absent-field answers when the inner field is `None`.
    ($type:ty, transparent($inner_field:ident) { $( $field:ident ),* $(,)? }) => {
        impl $crate::utils::FieldPresenceChecker for $type {
            fn top_level_fields(&self) -> &[&'static str] {
                // Static list — independent of whether the inner field is present.
                &[ $( stringify!($field) ),* ]
            }

            fn check_field_presence(&self, field: &str) -> Option<(bool, Option<&dyn $crate::utils::FieldPresenceChecker>)> {
                match &self.$inner_field {
                    // Inner present: delegate fully to its checker.
                    Some(inner) => inner.check_field_presence(field),
                    // Inner absent: all its fields are also absent.
                    // Return Some((false, None)) for valid field names, None for
                    // unknown ones (mirrors the inner type's own behaviour).
                    None => match field {
                        $( stringify!($field) => Some((false, None)), )*
                        _ => None,
                    },
                }
            }
        }
    };

    // Transparent-repeated type rule:
    //
    // Like `transparent`, but for when the inner field is a `Vec<T>` (repeated
    // proto field) instead of `Option<T>`.  Delegates to the first element when
    // the vec is non-empty; reports every field as absent when it is empty.
    //
    // Syntax:
    //   impl_field_presence_checker!(OuterType, transparent_repeated(vec_field) {
    //       inner_field1,
    //       inner_field2,
    //       ...
    //   });
    ($type:ty, transparent_repeated($inner_field:ident) { $( $field:ident ),* $(,)? }) => {
        impl $crate::utils::FieldPresenceChecker for $type {
            fn top_level_fields(&self) -> &[&'static str] {
                &[ $( stringify!($field) ),* ]
            }

            fn check_field_presence(&self, field: &str) -> Option<(bool, Option<&dyn $crate::utils::FieldPresenceChecker>)> {
                if let Some(first) = self.$inner_field.first() {
                    first.check_field_presence(field)
                } else {
                    // Vec is empty — all inner fields are absent.
                    match field {
                        $( stringify!($field) => Some((false, None)), )*
                        _ => None,
                    }
                }
            }
        }
    };

    // Helper rule for repeated fields (when `: [Type]` is specified)
    (@field_check $self:ident, $field:ident, [ $nested_type:ty ]) => {{
        // Repeated fields are always present, check if non-empty
        let present = !$self.$field.is_empty();

        // If the vec is non-empty, provide a reference to the first element as a checker
        let nested = $self.$field.first().map(|f| f as &dyn $crate::utils::FieldPresenceChecker);

        Some((present, nested))
    }};

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

/// Utility function to convert a comma-separated field mask string into a
/// vector of field paths For example,
/// "transaction.digest,transaction.bcs,signatures" becomes ["transaction.
/// digest", "transaction.bcs", "signatures"]
pub fn comma_separated_field_mask_to_paths(mask_str: &str) -> Vec<&str> {
    mask_str
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Assert field presence/absence for any type implementing
/// [`FieldPresenceChecker`].
///
/// Each path in `expected_field_paths` is either:
/// - A **bare** name (no dot), e.g. `"bcs"` or `"effects"`:
///   - If the field has a nested [`FieldPresenceChecker`], **all** of its
///     sub-fields are asserted present recursively (mirrors server wildcard
///     semantics where a parent path returns every sub-field).
///   - If the field has no nested checker (a leaf), only presence is asserted.
/// - A **dotted** path, e.g. `"reference.object_id"` — the top-level field must
///   be present, and exactly the listed sub-paths must be present inside it
///   (all other sub-fields are asserted absent).
///
/// Every top-level field that is *not* listed in `expected_field_paths` (either
/// bare or as the prefix of a dotted path) is asserted **absent**.
///
/// Paths listed in `ignored_field_paths` are skipped entirely — no presence or
/// absence is asserted for them.  Use this for fields that are optionally
/// populated depending on server state (e.g. `checkpoint` / `timestamp` that
/// are always `None` for just-executed transactions).  The format mirrors
/// `expected_field_paths`: a bare name ignores that field at the current level;
/// a dotted path (`"executed_transaction.checkpoint"`) ignores the named
/// sub-field when recursing into `executed_transaction`.
///
/// This design lets tests pass read-mask paths directly: a wildcard mask entry
/// like `"effects"` (all sub-fields) maps to bare `"effects"`, while a specific
/// entry like `"effects.digest"` maps to the dotted path.
///
/// # Example
/// ```ignore
/// assert_field_presence(
///     &object,
///     &["reference.object_id", "reference.version", "bcs"],
///     &[],
///     "test scenario"
/// );
/// ```
/// This checks:
/// - `reference` is present (inferred because reference.* are listed)
/// - `reference.object_id` is present
/// - `reference.version` is present
/// - `bcs` is present (leaf — presence only, no nested inspection)
/// - All other top-level fields are absent
/// - Inside `reference`: only `object_id` and `version` are present
///   (`reference.digest` is absent)
pub(crate) fn assert_field_presence(
    checker: &dyn FieldPresenceChecker,
    expected_field_paths: &[&str],
    ignored_field_paths: &[&str],
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

    // Parse ignored paths: bare names are ignored at this level; dotted paths
    // are threaded down into the corresponding nested recursion.
    let mut ignored_nested_field_paths: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut ignored_top_level_fields: HashSet<&str> = HashSet::new();

    for ignored_field_path in ignored_field_paths {
        if let Some((top_level_field, remaining_path)) = ignored_field_path.split_once('.') {
            ignored_nested_field_paths
                .entry(top_level_field)
                .or_default()
                .push(remaining_path);
        } else {
            ignored_top_level_fields.insert(ignored_field_path);
        }
    }

    let actual_top_level_fields: HashSet<&str> =
        checker.top_level_fields().iter().copied().collect();

    // Validate that all expected fields exist on this type
    for expected_top_level_field in &expected_top_level_fields {
        assert!(
            actual_top_level_fields.contains(expected_top_level_field),
            "Invalid field '{}' in '{scenario}': field does not exist on this type",
            expected_top_level_field
        );
    }

    // Check each field at this level for correct presence/absence.
    // Fields listed in ignored_top_level_fields are skipped entirely.
    for top_level_field in actual_top_level_fields.clone() {
        if ignored_top_level_fields.contains(top_level_field) {
            continue;
        }

        let should_be_present = expected_top_level_fields.contains(top_level_field);

        let (is_present, _) = checker
            .check_field_presence(top_level_field)
            .unwrap_or_else(|| panic!("Invalid field '{top_level_field}' in '{scenario}'"));

        assert_eq!(
            is_present, should_be_present,
            "Field '{top_level_field}' in '{scenario}': expected '{should_be_present}', got '{is_present}'"
        );
    }

    // Check that no field is specified both as nested and non-nested
    for non_nested_field in &expected_non_nested_field_paths {
        if expected_nested_field_paths.contains_key(non_nested_field) {
            panic!(
                "Contradictory field paths in '{scenario}': '{non_nested_field}' specified both as non-nested (implying no nested fields) and with nested fields ({})",
                expected_nested_field_paths[non_nested_field]
                    .iter()
                    .map(|s| format!("{}.{}", non_nested_field, s))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    // Recurse for fields with explicit dotted sub-paths, threading down any
    // ignored sub-paths that were specified for this field.
    for (top_level_field, sub_paths) in &expected_nested_field_paths {
        if let Some((_, Some(nested_checker))) = checker.check_field_presence(top_level_field) {
            let ignored_sub: &[&str] = ignored_nested_field_paths
                .get(top_level_field)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            assert_field_presence(
                nested_checker,
                sub_paths,
                ignored_sub,
                &format!("{scenario}.{top_level_field}"),
            );
        }
    }

    // For bare paths that have a nested checker, auto-recurse and assert that
    // ALL sub-fields are present (minus any ignored ones).  This mirrors the
    // server's wildcard behaviour: a mask entry like "effects" returns every
    // sub-field of effects, so the test expectation "effects" should verify all
    // of them.
    // Bare paths that are themselves ignored, or that have no nested checker
    // (leaf fields), are skipped — their presence was already handled above.
    for bare_field in &expected_non_nested_field_paths {
        if ignored_top_level_fields.contains(*bare_field) {
            continue;
        }
        if let Some((true, Some(nested_checker))) = checker.check_field_presence(bare_field) {
            let all_sub_fields: Vec<&str> = nested_checker.top_level_fields().to_vec();
            let ignored_sub: &[&str] = ignored_nested_field_paths
                .get(bare_field)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            assert_field_presence(
                nested_checker,
                &all_sub_fields,
                ignored_sub,
                &format!("{scenario}.{bare_field}"),
            );
        }
    }
}
