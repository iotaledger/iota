// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

mod field_mask_tree;
mod field_mask_util;

pub use field_mask_tree::FieldMaskTree;
pub use field_mask_util::FieldMaskUtil;
pub use prost_types::FieldMask;

/// Separator between field paths when a FieldMask is encoded as a string
pub const FIELD_PATH_SEPARATOR: char = ',';

/// Separator between fields in a field path
pub const FIELD_SEPARATOR: char = '.';

pub const FIELD_PATH_WILDCARD: &str = "*";

fn is_valid_path(path: &str) -> bool {
    if path == FIELD_PATH_WILDCARD {
        return true;
    }

    path.split(FIELD_SEPARATOR).all(is_valid_path_component)
}

// A valid path component needs to be a valid protobuf identifier which is
// defined by the following:
//
// ```
// letter        = "A" … "Z" | "a" … "z" | "_" .
// decimal_digit = "0" … "9"
// identifier = letter { letter | decimal_digit }
// ```
fn is_valid_path_component(component: &str) -> bool {
    if component.is_empty() || component == "_" {
        return false;
    }

    let component = component.as_bytes();

    if !(component[0].is_ascii_alphabetic() || component[0] == b'_') {
        return false;
    }

    for &byte in &component[1..] {
        if !(byte.is_ascii_alphabetic() || byte.is_ascii_digit() || byte == b'_') {
            return false;
        }
    }

    true
}

pub trait MessageFields {
    const FIELDS: &'static [&'static MessageField];

    /// Oneof group names declared in this message.
    ///
    /// A oneof name acts as a virtual parent path for its variant fields
    /// during read mask validation.  For example, a message with
    /// `oneof execution_result { CommandResults command_results = 3; ... }`
    /// lists `"execution_result"` here so that paths like
    /// `"execution_result.command_results"` are accepted by `validate()`.
    const ONEOFS: &'static [&'static str] = &[];
}

pub struct MessageField {
    pub name: &'static str,
    pub json_name: &'static str,
    pub number: i32,
    pub is_optional: bool,
    pub is_map: bool,
    pub message_fields: Option<&'static [&'static MessageField]>,
}

impl MessageField {
    // Returns the field name and all nested field names
    pub fn get_optional_message_field_names(&self) -> Vec<String> {
        let mut names = Vec::new();

        // Recursively get nested field names and prefix them with the parent field name
        if let Some(nested_fields) = self.message_fields {
            for field in nested_fields {
                for nested_name in field.get_optional_message_field_names() {
                    names.push(format!("{}.{}", self.name, nested_name));
                }
            }
        } else if self.is_optional {
            // No nested fields, just return the field name if it's optional
            names.push(self.name.to_string());
        }

        names
    }
}

impl AsRef<str> for MessageField {
    fn as_ref(&self) -> &str {
        self.name
    }
}

#[doc(hidden)]
impl MessageField {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            json_name: "",
            number: 0,
            is_optional: false,
            is_map: false,
            message_fields: None,
        }
    }

    pub const fn with_message_fields(
        mut self,
        message_fields: &'static [&'static MessageField],
    ) -> Self {
        self.message_fields = Some(message_fields);
        self
    }

    pub const fn with_optional(mut self, is_optional: bool) -> Self {
        self.is_optional = is_optional;
        self
    }

    pub const fn with_is_map(mut self, is_map: bool) -> Self {
        self.is_map = is_map;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_path_component() {
        let cases = [
            ("foo", true),
            ("_", false),
            ("", false),
            ("_abc", true),
            ("BAR", true),
            ("foo.bar", false),
        ];

        for (case, expected) in cases {
            assert_eq!(is_valid_path_component(case), expected);
        }
    }

    #[test]
    fn test_valid_path() {
        let cases = [
            ("*", true),
            ("**", false),
            ("foo.bar", true),
            ("foo.bar.baz", true),
            ("_", false),
            (".", false),
            ("", false),
            ("_abc", true),
            ("BAR", true),
        ];

        for (case, expected) in cases {
            assert_eq!(is_valid_path(case), expected);
        }
    }
}
