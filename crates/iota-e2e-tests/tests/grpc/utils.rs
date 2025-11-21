// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Trait for types that can provide field presence information
pub(crate) trait FieldPresenceChecker {
    /// Check if a field with the given name is present (not None)
    fn is_field_present(&self, field_name: &str) -> Option<bool>;
}

/// Macro to automatically implement FieldPresenceChecker for protobuf response
/// types.
///
/// Example: To add support for another protobuf response type, just add:
/// impl_field_presence_checker!(AnotherResponse, {
///     "field1" => field1,
///     "field2" => field2,
///     // ... other fields
/// });
#[macro_export]
macro_rules! impl_field_presence_checker {
    ($type:ty, { $( $field_name:literal => $field_ident:ident ),* $(,)? }) => {
        impl $crate::utils::FieldPresenceChecker for $type {
            fn is_field_present(&self, field_name: &str) -> Option<bool> {
                match field_name {
                    $( $field_name => Some(self.$field_ident.is_some()), )*
                    _ => None, // Unknown field
                }
            }
        }
    };
}

/// Assert field presence for any type implementing MessageFields +
/// FieldPresenceChecker
pub(crate) fn assert_field_presence<T>(response: &T, expected_fields: &[&str], scenario: &str)
where
    T: iota_grpc_types::field::MessageFields + FieldPresenceChecker,
{
    let expected_set: std::collections::HashSet<_> = expected_fields.iter().copied().collect();

    for field in T::FIELDS {
        let field_name = field.name;
        let should_be_present = expected_set.contains(field_name);

        match response.is_field_present(field_name) {
            Some(is_present) => {
                assert_eq!(
                    is_present, should_be_present,
                    "{field_name} presence mismatch in {scenario}: expected {should_be_present}, got {is_present}",
                );
            }
            None => panic!(
                "Unknown field '{field_name}' in {}, scenario {scenario}",
                std::any::type_name::<T>(),
            ),
        }
    }
}
