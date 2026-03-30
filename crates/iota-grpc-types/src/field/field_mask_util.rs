// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use prost_types::FieldMask;

use super::{
    FIELD_PATH_SEPARATOR, FIELD_PATH_WILDCARD, FIELD_SEPARATOR, FieldMaskTree, MessageField,
    MessageFields,
};

pub trait FieldMaskUtil: sealed::Sealed {
    fn normalize(self) -> FieldMask;

    fn from_str(s: &str) -> FieldMask;

    fn from_paths<I: AsRef<str>, T: IntoIterator<Item = I>>(paths: T) -> FieldMask;

    fn display(&self) -> impl std::fmt::Display + '_;

    fn validate<M: MessageFields>(&self) -> Result<(), &str>;
}

impl FieldMaskUtil for FieldMask {
    fn normalize(self) -> FieldMask {
        FieldMaskTree::from(self).to_field_mask()
    }

    fn from_str(s: &str) -> FieldMask {
        Self::from_paths(s.split(FIELD_PATH_SEPARATOR))
    }

    fn from_paths<I: AsRef<str>, T: IntoIterator<Item = I>>(paths: T) -> FieldMask {
        FieldMask {
            paths: paths
                .into_iter()
                .filter_map(|path| {
                    let path = path.as_ref();
                    if path.is_empty() {
                        None
                    } else {
                        Some(path.to_owned())
                    }
                })
                .collect(),
        }
    }

    fn display(&self) -> impl std::fmt::Display + '_ {
        FieldMaskDisplay(self)
    }

    fn validate<M: MessageFields>(&self) -> Result<(), &str> {
        // Determine if the provided path matches one of the provided fields. If a path
        // matches a field and that field is a message type (which can have its
        // own set of fields), attempt to match the remainder of the path
        // against a field in the sub_message.
        //
        // `oneofs` lists oneof group names for the current message level.
        // A oneof name acts as a virtual parent: `"execution_result"` alone is
        // valid (selects all variants), and `"execution_result.command_results"`
        // is validated by recursing into the same `fields` array.
        fn is_valid_path(
            mut fields: &[&MessageField],
            mut oneofs: &[&str],
            mut path: &str,
        ) -> bool {
            loop {
                let (field_name, remainder) = path
                    .split_once(FIELD_SEPARATOR)
                    .map(|(field, remainder)| (field, (!remainder.is_empty()).then_some(remainder)))
                    .unwrap_or((path, None));

                if let Some(field) = fields.iter().find(|field| field.name == field_name) {
                    match (field.message_fields, remainder) {
                        (None, None) | (Some(_), None) => return true,
                        // If the matched field is a map, any key name after it is valid
                        (None, Some(_)) if field.is_map => return true,
                        (None, Some(_)) => return false,
                        (Some(sub_message_fields), Some(remainder)) => {
                            fields = sub_message_fields;
                            // Nested messages have their own oneofs (from their
                            // own MessageFields impl), but we don't have access
                            // to those here.  Oneof-as-parent only applies at
                            // the top level of each message, and nested oneofs
                            // in read masks are uncommon, so this is acceptable.
                            oneofs = &[];
                            path = remainder;
                        }
                    }
                } else if oneofs.contains(&field_name) {
                    // The path segment matches a oneof group name.
                    // Bare oneof name (no remainder) means "all variants".
                    // With a remainder, validate the rest against the same
                    // fields (oneof variants are top-level fields).
                    match remainder {
                        None => return true,
                        Some(remainder) => {
                            // Don't recurse through oneofs again at the same
                            // level — the remainder should match a real field.
                            oneofs = &[];
                            path = remainder;
                        }
                    }
                } else {
                    return false;
                }
            }
        }

        for path in &self.paths {
            if path == FIELD_PATH_WILDCARD {
                continue;
            }
            if !is_valid_path(M::FIELDS, M::ONEOFS, path) {
                return Err(path);
            }
        }

        Ok(())
    }
}

struct FieldMaskDisplay<'a>(&'a FieldMask);

impl std::fmt::Display for FieldMaskDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use std::fmt::Write;

        let mut first = true;

        for path in &self.0.paths {
            // Ignore empty paths
            if path.is_empty() {
                continue;
            }

            // If this isn't the first path we've printed,
            // we need to print a FIELD_PATH_SEPARATOR character
            if first {
                first = false;
            } else {
                f.write_char(FIELD_PATH_SEPARATOR)?;
            }
            f.write_str(path)?;
        }

        Ok(())
    }
}

mod sealed {
    pub trait Sealed {}

    impl Sealed for prost_types::FieldMask {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_string() {
        assert!(
            FieldMask::display(&FieldMask::default())
                .to_string()
                .is_empty()
        );

        let mask = FieldMask::from_paths(["foo"]);
        assert_eq!(FieldMask::display(&mask).to_string(), "foo");
        assert_eq!(mask.display().to_string(), "foo");
        let mask = FieldMask::from_paths(["foo", "bar"]);
        assert_eq!(FieldMask::display(&mask).to_string(), "foo,bar");

        // empty paths are ignored
        let mask = FieldMask::from_paths(["", "foo", "", "bar", ""]);
        assert_eq!(FieldMask::display(&mask).to_string(), "foo,bar");
    }

    #[test]
    fn test_from_str() {
        let mask = FieldMask::from_str("");
        assert!(mask.paths.is_empty());

        let mask = FieldMask::from_str("foo");
        assert_eq!(mask.paths.len(), 1);
        assert_eq!(mask.paths[0], "foo");

        let mask = FieldMask::from_str("foo,bar.baz");
        assert_eq!(mask.paths.len(), 2);
        assert_eq!(mask.paths[0], "foo");
        assert_eq!(mask.paths[1], "bar.baz");

        // empty field paths are ignored
        let mask = FieldMask::from_str(",foo,,bar,");
        assert_eq!(mask.paths.len(), 2);
        assert_eq!(mask.paths[0], "foo");
        assert_eq!(mask.paths[1], "bar");
    }

    #[test]
    fn test_validate() {
        struct Foo;
        impl MessageFields for Foo {
            const FIELDS: &'static [&'static MessageField] = &[
                &MessageField::new("bar").with_message_fields(Bar::FIELDS),
                &MessageField::new("baz"),
            ];
        }
        struct Bar;

        impl MessageFields for Bar {
            const FIELDS: &'static [&'static MessageField] = &[
                &MessageField {
                    name: "a",
                    json_name: "a",
                    number: 1,
                    is_optional: false,
                    is_map: false,
                    message_fields: None,
                },
                &MessageField {
                    name: "b",
                    json_name: "b",
                    number: 2,
                    is_optional: false,
                    is_map: false,
                    message_fields: None,
                },
            ];
        }

        let mask = FieldMask::from_str("");
        assert_eq!(mask.validate::<Foo>(), Ok(()));
        let mask = FieldMask::from_str("bar");
        assert_eq!(mask.validate::<Foo>(), Ok(()));
        let mask = FieldMask::from_str("bar.a");
        assert_eq!(mask.validate::<Foo>(), Ok(()));
        let mask = FieldMask::from_str("bar.a,bar.b");
        assert_eq!(mask.validate::<Foo>(), Ok(()));
        let mask = FieldMask::from_str("bar.a,bar.b,bar.c");
        assert_eq!(mask.validate::<Foo>(), Err("bar.c"));
        let mask = FieldMask::from_str("baz");
        assert_eq!(mask.validate::<Foo>(), Ok(()));
        let mask = FieldMask::from_str("baz.a");
        assert_eq!(mask.validate::<Foo>(), Err("baz.a"));
        let mask = FieldMask::from_str("foobar");
        assert_eq!(mask.validate::<Foo>(), Err("foobar"));
    }

    #[test]
    fn test_validate_map_key() {
        // Simulates ProtocolConfig-like structure:
        //   Outer.map_wrapper (message) -> MapWrapper.entries (map<string, string>)
        struct Outer;
        struct MapWrapper;

        // MapWrapper has a single map field
        const MAP_FIELD: &MessageField = &MessageField {
            name: "entries",
            json_name: "entries",
            number: 1,
            is_optional: false,
            is_map: true,
            message_fields: None,
        };

        impl MessageFields for MapWrapper {
            const FIELDS: &'static [&'static MessageField] = &[MAP_FIELD];
        }

        impl MessageFields for Outer {
            const FIELDS: &'static [&'static MessageField] = &[
                &MessageField::new("scalar"),
                &MessageField::new("map_wrapper").with_message_fields(MapWrapper::FIELDS),
            ];
        }

        // Exact field name is valid
        let mask = FieldMask::from_str("map_wrapper");
        assert_eq!(mask.validate::<Outer>(), Ok(()));

        // Exact map field name is valid
        let mask = FieldMask::from_str("map_wrapper.entries");
        assert_eq!(mask.validate::<Outer>(), Ok(()));

        // Any key after the map field is valid
        let mask = FieldMask::from_str("map_wrapper.entries.my_key");
        assert_eq!(mask.validate::<Outer>(), Ok(()));

        // Multiple keys
        let mask = FieldMask::from_str("map_wrapper.entries.key1,map_wrapper.entries.key2");
        assert_eq!(mask.validate::<Outer>(), Ok(()));

        // Non-map scalar field still rejects sub-paths
        let mask = FieldMask::from_str("scalar.bad");
        assert_eq!(mask.validate::<Outer>(), Err("scalar.bad"));

        // Non-existent top-level field is rejected
        let mask = FieldMask::from_str("no_such_field");
        assert_eq!(mask.validate::<Outer>(), Err("no_such_field"));
    }

    #[test]
    fn test_validate_oneof() {
        // Simulates SimulatedTransaction-like structure:
        //   oneof execution_result { CommandResults command_results = 3; ExecutionError
        // execution_error = 4; }
        struct Msg;
        struct CmdResults;
        struct ExecError;

        impl MessageFields for CmdResults {
            const FIELDS: &'static [&'static MessageField] = &[&MessageField::new("bcs")];
        }

        impl MessageFields for ExecError {
            const FIELDS: &'static [&'static MessageField] = &[
                &MessageField::new("source"),
                &MessageField::new("command_index"),
            ];
        }

        impl MessageFields for Msg {
            const FIELDS: &'static [&'static MessageField] = &[
                &MessageField::new("executed_transaction"),
                &MessageField::new("command_results").with_message_fields(CmdResults::FIELDS),
                &MessageField::new("execution_error").with_message_fields(ExecError::FIELDS),
            ];
            const ONEOFS: &'static [&'static str] = &["execution_result"];
        }

        // Bare oneof name is valid (selects all variants)
        let mask = FieldMask::from_str("execution_result");
        assert_eq!(mask.validate::<Msg>(), Ok(()));

        // Oneof name + variant field is valid
        let mask = FieldMask::from_str("execution_result.command_results");
        assert_eq!(mask.validate::<Msg>(), Ok(()));

        // Oneof name + variant field + nested field is valid
        let mask = FieldMask::from_str("execution_result.command_results.bcs");
        assert_eq!(mask.validate::<Msg>(), Ok(()));

        let mask = FieldMask::from_str("execution_result.execution_error.source");
        assert_eq!(mask.validate::<Msg>(), Ok(()));

        // Oneof name + non-existent field is invalid
        let mask = FieldMask::from_str("execution_result.no_such_field");
        assert_eq!(
            mask.validate::<Msg>(),
            Err("execution_result.no_such_field")
        );

        // Oneof name + variant + invalid nested field is invalid
        let mask = FieldMask::from_str("execution_result.execution_error.bad");
        assert_eq!(
            mask.validate::<Msg>(),
            Err("execution_result.execution_error.bad")
        );

        // Direct variant field (without oneof prefix) is still valid
        let mask = FieldMask::from_str("command_results");
        assert_eq!(mask.validate::<Msg>(), Ok(()));

        // Non-existent top-level field is still invalid
        let mask = FieldMask::from_str("no_such_field");
        assert_eq!(mask.validate::<Msg>(), Err("no_such_field"));

        // Regular field is still valid
        let mask = FieldMask::from_str("executed_transaction");
        assert_eq!(mask.validate::<Msg>(), Ok(()));
    }
}
