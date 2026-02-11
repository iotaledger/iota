// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::collections::HashMap;

use bitflags::bitflags;
use prost_types::FieldDescriptorProto;

bitflags! {
    /// Flags for different types of accessor methods to generate
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct AccessorTypes: u8 {
        /// Generate `field()` getter returning value or default
        const GETTER = 0b0000_0001;
        /// Generate `field_opt()` getter returning `Option<&T>`
        const GETTER_OPT = 0b0000_0010;
        /// Generate `set_field()` setter method
        const SET = 0b0000_0100;
        /// Generate `with_field()` builder-pattern setter
        const WITH = 0b0000_1000;
        /// Generate `field_mut()` returning `&mut T`
        const MUT = 0b0001_0000;
        /// Generate `field_opt_mut()` returning `Option<&mut T>`
        const MUT_OPT = 0b0010_0000;
        /// Generate `const_default()` and `default_instance()` helper functions
        const DEFAULT = 0b0100_0000;
    }
}

impl AccessorTypes {
    /// Parse a comma-separated string of accessor types
    /// Example: "set,with" -> AccessorTypes::SET | AccessorTypes::WITH
    /// Special values:
    /// - "all" -> generates all accessor types (getter, getter_opt, set, with,
    ///   mut, mut_opt) Note: "all" cannot be combined with other accessor types
    ///   Note: "all" includes getter, which automatically generates default
    ///   helpers
    /// - "default" -> generates const_default() and default_instance() helpers
    ///   Note: Only use "default" with non-getter accessors (e.g.,
    ///   "set,with,default") Note: "getter" and "all" already include default
    ///   helpers, so don't combine
    ///
    /// Panics if:
    /// - Unknown accessor type is encountered
    /// - "all" is combined with other accessor types
    /// - "default" is combined with "getter" or "all" (redundant, since getter
    ///   includes defaults)
    pub fn parse(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }

        let mut result = AccessorTypes::empty();
        let mut has_all = false;
        let mut has_other_accessors = false;

        for part in s.split(',') {
            let part = part.trim();
            match part {
                "all" => {
                    has_all = true;
                    result |= AccessorTypes::GETTER
                        | AccessorTypes::GETTER_OPT
                        | AccessorTypes::SET
                        | AccessorTypes::WITH
                        | AccessorTypes::MUT
                        | AccessorTypes::MUT_OPT;
                }
                "default" => result |= AccessorTypes::DEFAULT,
                "getter" => {
                    has_other_accessors = true;
                    result |= AccessorTypes::GETTER;
                }
                "getter_opt" => {
                    has_other_accessors = true;
                    result |= AccessorTypes::GETTER_OPT;
                }
                "set" => {
                    has_other_accessors = true;
                    result |= AccessorTypes::SET;
                }
                "with" => {
                    has_other_accessors = true;
                    result |= AccessorTypes::WITH;
                }
                "mut" => {
                    has_other_accessors = true;
                    result |= AccessorTypes::MUT;
                }
                "mut_opt" => {
                    has_other_accessors = true;
                    result |= AccessorTypes::MUT_OPT;
                }
                _ => {
                    panic!(
                        "Unknown accessor type '{}'. Valid types are: getter, getter_opt, set, with, mut, mut_opt, all, default",
                        part
                    );
                }
            }
        }

        if has_all && has_other_accessors {
            panic!(
                "Cannot combine 'all' with other accessor types in '{}'. Use 'all' alone, or list specific types.",
                s
            );
        }

        // Validate that 'default' is not combined with 'getter' or 'all' (since getter
        // already includes default)
        if result.contains(AccessorTypes::DEFAULT) && result.contains(AccessorTypes::GETTER) {
            panic!(
                "Cannot combine 'default' with 'getter' or 'all' in '{}'. The 'getter' accessor already generates default helpers. Use 'default' only with non-getter accessors like 'set,with,default'.",
                s
            );
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Extract accessor types from a protobuf field's custom options
    /// Returns None if the field doesn't have the generate_accessors option
    pub fn from_field(
        field: &FieldDescriptorProto,
        accessor_map: &AccessorMap,
        message_name: &str,
    ) -> Option<Self> {
        // Build the key as "message_name.field_name"
        let key = format!("{}.{}", message_name, field.name());

        // Try to find in the map
        accessor_map.get(&key).copied()
    }
}

/// Map of field names to their accessor configurations
/// Key is "message_name.field_name", value is the accessor types
pub type AccessorMap = HashMap<String, AccessorTypes>;

/// Parse proto files to extract generate_accessors annotations from the
/// descriptor pool Returns a map of "MessageName.field_name" -> AccessorTypes
pub fn parse_proto_accessors_from_pool(pool: &prost_reflect::DescriptorPool) -> AccessorMap {
    let mut map = HashMap::new();

    // Get the extension descriptor for iota.grpc.generate_accessors
    let ext = match pool.get_extension_by_name("iota.grpc.generate_accessors") {
        Some(ext) => ext,
        None => {
            panic!("Extension iota.grpc.generate_accessors not found in descriptor pool");
        }
    };

    // Iterate all messages (including nested ones)
    for message in pool.all_messages() {
        let message_name = message.name();

        // Iterate all fields in this message
        for field in message.fields() {
            let field_name = field.name();

            // Get field options
            let options = field.options();

            // Check if the extension is set
            if options.has_extension(&ext) {
                if let Some(accessor_str) = options.get_extension(&ext).as_str() {
                    let key = format!("{}.{}", message_name, field_name);

                    if let Some(accessor_types) = AccessorTypes::parse(accessor_str) {
                        map.insert(key, accessor_types);
                    }
                }
            }
        }
    }

    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single() {
        assert_eq!(AccessorTypes::parse("set"), Some(AccessorTypes::SET));
        assert_eq!(AccessorTypes::parse("with"), Some(AccessorTypes::WITH));
    }

    #[test]
    fn test_parse_multiple() {
        let result = AccessorTypes::parse("set,with");
        assert_eq!(result, Some(AccessorTypes::SET | AccessorTypes::WITH));
    }

    #[test]
    fn test_parse_all() {
        let result = AccessorTypes::parse("getter,getter_opt,set,with,mut,mut_opt");
        assert_eq!(
            result,
            Some(
                AccessorTypes::GETTER
                    | AccessorTypes::GETTER_OPT
                    | AccessorTypes::SET
                    | AccessorTypes::WITH
                    | AccessorTypes::MUT
                    | AccessorTypes::MUT_OPT
            )
        );
    }

    #[test]
    fn test_parse_whitespace() {
        let result = AccessorTypes::parse("set , with ");
        assert_eq!(result, Some(AccessorTypes::SET | AccessorTypes::WITH));
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(AccessorTypes::parse(""), None);
    }

    #[test]
    fn test_parse_all_keyword() {
        let result = AccessorTypes::parse("all");
        assert_eq!(
            result,
            Some(
                AccessorTypes::GETTER
                    | AccessorTypes::GETTER_OPT
                    | AccessorTypes::SET
                    | AccessorTypes::WITH
                    | AccessorTypes::MUT
                    | AccessorTypes::MUT_OPT
            )
        );
    }

    #[test]
    fn test_parse_default_keyword() {
        let result = AccessorTypes::parse("default");
        assert_eq!(result, Some(AccessorTypes::DEFAULT));
    }

    #[test]
    #[should_panic(expected = "Unknown accessor type")]
    fn test_parse_unknown_panics() {
        AccessorTypes::parse("invalid");
    }

    #[test]
    #[should_panic(expected = "Cannot combine 'all' with other accessor types")]
    fn test_parse_all_with_set_panics() {
        AccessorTypes::parse("all,set");
    }

    #[test]
    #[should_panic(expected = "Cannot combine 'default' with 'getter' or 'all'")]
    fn test_parse_getter_with_default_panics() {
        // This should panic - getter already generates defaults
        AccessorTypes::parse("getter,default");
    }
}
