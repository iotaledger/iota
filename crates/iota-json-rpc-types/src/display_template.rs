// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rendering of on-chain Display templates, shared by the JSON-RPC and GraphQL
//! servers.
//!
//! A Display is a set of `key = template` entries published for a Move type,
//! for example `name = "{name}"` or `image_url = "https://x.io/{id}.png"`.
//! A `{field.path}` placeholder is replaced by the value of that field of the
//! object and `\` makes the next character literal.

use iota_types::base_types::RESOLVED_STD_OPTION;
use move_core_types::annotated_value::{MoveStruct, MoveValue};

use crate::{IotaMoveStruct, IotaMoveValue, IotaMoveVariant};

/// Maximum total size in bytes of the rendered Display fields of one object.
pub const MAX_DISPLAY_OUTPUT_SIZE: usize = 1024 * 1024;
/// Maximum nesting depth of a placeholder path, counted in field names.
///
/// e.g. `{a.b.c}` has a depth of 3.
pub const MAX_DISPLAY_NESTED_LEVEL: usize = 10;

#[derive(thiserror::Error, Debug)]
pub enum DisplayRenderError {
    #[error("Display template value cannot be empty")]
    TemplateValueEmpty,
    #[error("Display template value of {0} exceeds maximum depth of {1}")]
    ExceedsLookupDepth(usize, usize),
    #[error("Vector is not supported as a Display value {0}")]
    Vector(String),
    #[error("Field value {0} cannot be found in struct")]
    FieldNotFound(String),
    #[error("Field value {path} cannot be found in variant {variant}")]
    FieldNotFoundInVariant { path: String, variant: String },
    #[error("Unexpected move value type for field {0}")]
    UnexpectedMoveValue(String),
    #[error("Rendered Display fields exceed the maximum size of {0} bytes")]
    OutputTooLarge(usize),
}

/// Field lookup for rendering on-chain Display templates.
///
/// Resolves a dot-separated path (e.g. `owner.address` from `{owner.address}`)
/// to the string that replaces the placeholder.
pub trait DisplayFieldLookup {
    /// Looks up a field by its path and returns its string representation.
    ///
    /// # Errors
    /// Returns a [`DisplayRenderError`] if the path is empty, cannot be found,
    /// exceeds nesting limits, or references an unsupported type (such as a vector).
    fn get_display_value(&self, path: &str) -> Result<String, DisplayRenderError>;
}

/// Renders one Display template against `object`.
///
/// The template is read once, left to right. Plain text is copied, a
/// `{path}` placeholder is replaced by `object.get_display_value(path)`, and
/// `\` makes the next character literal. Substituted values are never read
/// again, so a value that contains `{...}` stays as it is.
///
/// The rendered size is subtracted from `output_budget`, which is shared by
/// all fields of one object. If the result would not fit, an error is
/// returned and the budget is set to zero, so the remaining fields of the
/// object fail without being rendered.
pub fn render_display_template(
    template: &str,
    object: &impl DisplayFieldLookup,
    output_budget: &mut usize,
) -> Result<String, DisplayRenderError> {
    if *output_budget == 0 {
        return Err(DisplayRenderError::OutputTooLarge(MAX_DISPLAY_OUTPUT_SIZE));
    }

    let mut output = String::new();
    // `{` and the text read after it, written out as it is if the placeholder
    // is never closed.
    let mut placeholder = String::new();
    let mut var_name = String::new();
    let mut in_braces = false;
    let mut escaped = false;

    for ch in template.chars() {
        match ch {
            '\\' if !escaped => {
                escaped = true;
                continue;
            }
            '{' if !escaped => {
                if in_braces {
                    // The earlier `{` was never closed, so it is plain text.
                    output.push_str(&placeholder);
                }
                placeholder.clear();
                placeholder.push('{');
                in_braces = true;
                var_name.clear();
            }
            '}' if !escaped && in_braces => {
                in_braces = false;
                placeholder.clear();
                let value = object.get_display_value(&var_name)?;
                if output.len() + value.len() > *output_budget {
                    *output_budget = 0;
                    return Err(DisplayRenderError::OutputTooLarge(MAX_DISPLAY_OUTPUT_SIZE));
                }
                output.push_str(&value);
            }
            _ if in_braces => {
                if !escaped {
                    var_name.push(ch);
                }
                placeholder.push(ch);
            }
            _ => output.push(ch),
        }
        escaped = false;
    }
    output.push_str(&placeholder);

    if output.len() > *output_budget {
        *output_budget = 0;
        return Err(DisplayRenderError::OutputTooLarge(MAX_DISPLAY_OUTPUT_SIZE));
    }
    *output_budget -= output.len();
    Ok(output)
}

/// Splits a dotted lookup path (e.g. `"owner.address"`) into its first part
/// and the remaining parts.
///
/// # Errors
/// - [`DisplayRenderError::TemplateValueEmpty`] if the path is empty or starts with a dot.
/// - [`DisplayRenderError::ExceedsLookupDepth`] if the number of parts exceeds
///   [`MAX_DISPLAY_NESTED_LEVEL`].
fn split_path(path: &str) -> Result<(&str, Vec<&str>), DisplayRenderError> {
    let mut parts = path.split('.');
    let first = match parts.next() {
        Some(first) if !first.is_empty() => first,
        _ => return Err(DisplayRenderError::TemplateValueEmpty),
    };
    let rest: Vec<&str> = parts.collect();
    if rest.len() + 1 > MAX_DISPLAY_NESTED_LEVEL {
        return Err(DisplayRenderError::ExceedsLookupDepth(
            rest.len() + 1,
            MAX_DISPLAY_NESTED_LEVEL,
        ));
    }
    Ok((first, rest))
}

/// Converts a leaf value into its string representation for template substitution.
///
/// - **Options:** Renders the `Some` value by these same rules, or returns an empty string (`""`)
///   for `None`.
/// - **Vectors:** Explicitly rejected as unsupported display values (returning
///   [`DisplayRenderError::Vector`]).
/// - **Primitives/Others:** Converted to string via `to_string()`.
fn render_leaf(value: &IotaMoveValue, path: &str) -> Result<String, DisplayRenderError> {
    match value {
        IotaMoveValue::Option(move_option) => match move_option.as_ref() {
            Some(inner) => render_leaf(inner, path),
            None => Ok("".to_string()),
        },
        IotaMoveValue::Vector(_) => Err(DisplayRenderError::Vector(path.to_string())),
        _ => Ok(value.to_string()),
    }
}

/// Field lookup over the JSON-RPC representation of an object, used by the
/// full node and the indexer.
///
/// Unlike the raw Move representation, framework types such as `String` and
/// `UID` are already flattened to plain values, so a path like `id.id` fails
/// here. A path may walk through nested structs and enum variants. Only the
/// final value is converted for display, and it must be a scalar.
impl DisplayFieldLookup for IotaMoveStruct {
    fn get_display_value(&self, path: &str) -> Result<String, DisplayRenderError> {
        let (first, rest) = split_path(path)?;
        let mut current = get_iota_struct_field(self, first, path)?;
        for part in rest {
            current =
                match current {
                    IotaMoveValue::Struct(s) => get_iota_struct_field(s, part, path)?,
                    IotaMoveValue::Variant(IotaMoveVariant {
                        fields, variant, ..
                    }) => fields.get(part).ok_or_else(|| {
                        DisplayRenderError::FieldNotFoundInVariant {
                            path: path.to_string(),
                            variant: variant.clone(),
                        }
                    })?,
                    _ => return Err(DisplayRenderError::UnexpectedMoveValue(path.to_string())),
                };
        }
        render_leaf(current, path)
    }
}

/// Returns the field named `part` of `move_struct`. `path` is the full
/// placeholder path, used only in error messages.
///
/// # Errors
/// - [`DisplayRenderError::FieldNotFound`] if the struct has no such field.
/// - [`DisplayRenderError::UnexpectedMoveValue`] for a `Runtime` struct, whose fields are
///   positional and cannot be looked up by name.
fn get_iota_struct_field<'a>(
    move_struct: &'a IotaMoveStruct,
    part: &str,
    path: &str,
) -> Result<&'a IotaMoveValue, DisplayRenderError> {
    match move_struct {
        IotaMoveStruct::WithTypes { fields, .. } | IotaMoveStruct::WithFields(fields) => fields
            .get(part)
            .ok_or_else(|| DisplayRenderError::FieldNotFound(path.to_string())),
        IotaMoveStruct::Runtime(_) => {
            Err(DisplayRenderError::UnexpectedMoveValue(path.to_string()))
        }
    }
}

/// Field lookup over the raw Move representation of an object, used by
/// GraphQL.
///
/// Unlike the JSON-RPC representation, the raw struct keeps framework types
/// such as `String` and `UID` as nested structs, so a path may walk into
/// them (e.g. `id.id`). Only the final value is converted for display, and it
/// must still be a scalar.
impl DisplayFieldLookup for MoveStruct {
    fn get_display_value(&self, path: &str) -> Result<String, DisplayRenderError> {
        let (first, rest) = split_path(path)?;
        let mut current = get_move_struct_field(self, first)?;
        for part in rest {
            current = match current {
                MoveValue::Struct(s) => get_move_struct_field(s, part)?,
                _ => return Err(DisplayRenderError::UnexpectedMoveValue(path.to_string())),
            };
        }
        // An `Option` is a struct holding a vector of zero or one element.
        // Unwrap it here so that a vector inside is rejected before the value
        // is copied.
        while let MoveValue::Struct(s) = current {
            let ty = &s.type_;
            if (&ty.address, ty.module.as_ref(), ty.name.as_ref()) != RESOLVED_STD_OPTION {
                break;
            }
            let Some((_, MoveValue::Vector(elements))) = s.fields.first() else {
                break;
            };
            match elements.first() {
                Some(inner) => current = inner,
                None => return Ok("".to_string()),
            }
        }
        if let MoveValue::Vector(_) = current {
            return Err(DisplayRenderError::Vector(path.to_string()));
        }
        render_leaf(&current.clone().into(), path)
    }
}

/// Returns the field named `part` of `move_struct`.
///
/// # Errors
/// - [`DisplayRenderError::FieldNotFound`] if the struct has no such field.
fn get_move_struct_field<'a>(
    move_struct: &'a MoveStruct,
    part: &str,
) -> Result<&'a MoveValue, DisplayRenderError> {
    move_struct
        .fields
        .iter()
        .find_map(|(id, value)| (id.as_str() == part).then_some(value))
        .ok_or_else(|| DisplayRenderError::FieldNotFound(part.to_string()))
}

#[cfg(test)]
mod tests {
    use move_core_types::{
        account_address::AccountAddress, identifier::Identifier, language_storage::StructTag,
    };

    use super::*;

    /// A `std::string::String` as it is stored on chain.
    fn move_string(value: &str) -> MoveValue {
        MoveValue::Struct(MoveStruct {
            type_: StructTag {
                address: AccountAddress::ONE,
                module: Identifier::new("string").unwrap(),
                name: Identifier::new("String").unwrap(),
                type_params: vec![],
            },
            fields: vec![(
                Identifier::new("bytes").unwrap(),
                MoveValue::Vector(value.bytes().map(MoveValue::U8).collect()),
            )],
        })
    }

    /// The object as GraphQL sees it: raw Move values.
    fn move_struct(fields: &[(&str, &str)]) -> MoveStruct {
        MoveStruct {
            type_: StructTag {
                address: AccountAddress::TWO,
                module: Identifier::new("test").unwrap(),
                name: Identifier::new("Test").unwrap(),
                type_params: vec![],
            },
            fields: fields
                .iter()
                .map(|(name, value)| (Identifier::new(*name).unwrap(), move_string(value)))
                .collect(),
        }
    }

    /// The same object as JSON-RPC sees it: converted, strings flattened.
    fn iota_struct(fields: &[(&str, &str)]) -> IotaMoveStruct {
        IotaMoveStruct::WithFields(
            fields
                .iter()
                .map(|(name, value)| (name.to_string(), IotaMoveValue::String(value.to_string())))
                .collect(),
        )
    }

    const FIELDS: &[(&str, &str)] = &[
        ("name", "Boar"),
        ("id", "0x1"),
        ("url", "https://x.io/{id}.png"), // value that itself contains a placeholder
        ("path", "C:\\nft"),              // value with a backslash
        ("i", "{i}{i}"),                  // each substitution adds two placeholders
    ];

    /// Renders through both lookups and checks they agree.
    fn render(template: &str) -> Result<String, DisplayRenderError> {
        let mut budget = MAX_DISPLAY_OUTPUT_SIZE;
        let a = render_display_template(template, &move_struct(FIELDS), &mut budget);
        let mut budget = MAX_DISPLAY_OUTPUT_SIZE;
        let b = render_display_template(template, &iota_struct(FIELDS), &mut budget);
        assert_eq!(
            a.as_ref().map_err(ToString::to_string),
            b.as_ref().map_err(ToString::to_string),
            "MoveStruct and IotaMoveStruct lookups differ for {template:?}"
        );
        a
    }

    #[test]
    fn test_render_display_template() {
        let cases = [
            ("no template", "no template"),
            ("{name}", "Boar"),
            ("A {name} with {id}", "A Boar with 0x1"),
            ("{name}{name}", "BoarBoar"),
            // A value is appended as text, never read as a template.
            ("{url} {id}", "https://x.io/{id}.png 0x1"),
            // A backslash inside a value is kept.
            ("{path}", "C:\\nft"),
            // Escapes.
            ("\\{name\\}", "{name}"),
            ("\\{name\\} is {name}", "{name} is Boar"),
            ("{name} \\{name}", "Boar {name}"),
            ("\\\\{name}", "\\Boar"),
            ("a\\\\b", "a\\b"),
            // Broken braces are plain text.
            ("a } b", "a } b"),
            ("unclosed {name", "unclosed {name"),
            ("{a{name}", "{aBoar"),
        ];
        for (template, expected) in cases {
            assert_eq!(render(template).unwrap(), expected, "template {template:?}");
        }
        assert!(matches!(
            render("{missing}"),
            Err(DisplayRenderError::FieldNotFound(_))
        ));
        assert!(matches!(
            render("{}"),
            Err(DisplayRenderError::TemplateValueEmpty)
        ));
        assert!(matches!(
            render(&format!("{{{}}}", ["a"; 11].join("."))),
            Err(DisplayRenderError::ExceedsLookupDepth(11, 10))
        ));
    }

    #[test]
    fn test_render_display_template_does_not_render_placeholders_in_values() {
        // Each substituted value holds two placeholders. Expanding them again
        // would double the output at every placeholder of the template.
        let output = render(&"{i}".repeat(64)).unwrap();
        assert_eq!(output, "{i}{i}".repeat(64));
    }

    #[test]
    fn test_render_display_template_output_budget() {
        let value = "x".repeat(1024);
        let object = iota_struct(&[("v", &value)]);
        let mut budget = 4 * 1024;

        // Fits exactly in the budget.
        let output = render_display_template(&"{v}".repeat(4), &object, &mut budget).unwrap();
        assert_eq!(output.len(), 4 * 1024);
        assert_eq!(budget, 0);

        // Nothing is left, so even a template without placeholders fails.
        assert!(matches!(
            render_display_template("text", &object, &mut budget),
            Err(DisplayRenderError::OutputTooLarge(_)),
        ));

        // A field that does not fit uses up the budget for the next fields.
        let mut budget = 4 * 1024;
        assert!(matches!(
            render_display_template(&"{v}".repeat(5), &object, &mut budget),
            Err(DisplayRenderError::OutputTooLarge(_)),
        ));
        assert_eq!(budget, 0);
        assert!(matches!(
            render_display_template("{v}", &object, &mut budget),
            Err(DisplayRenderError::OutputTooLarge(_)),
        ));
    }

    #[test]
    fn test_nested_paths() {
        // `{owner.address}` walks into the `owner` struct on both
        // representations and renders its `address` field.
        let mut owner = move_struct(&[("address", "0xabc")]);
        owner.type_.name = Identifier::new("Owner").unwrap();
        let mut object = move_struct(&[("name", "Boar")]);
        object
            .fields
            .push((Identifier::new("owner").unwrap(), MoveValue::Struct(owner)));

        let iota_object = IotaMoveStruct::WithFields(
            [
                (
                    "name".to_string(),
                    IotaMoveValue::String("Boar".to_string()),
                ),
                (
                    "owner".to_string(),
                    IotaMoveValue::Struct(iota_struct(&[("address", "0xabc")])),
                ),
            ]
            .into_iter()
            .collect(),
        );

        for template in ["{owner.address}", "{name} of {owner.address}"] {
            let mut budget = MAX_DISPLAY_OUTPUT_SIZE;
            let a = render_display_template(template, &object, &mut budget).unwrap();
            let mut budget = MAX_DISPLAY_OUTPUT_SIZE;
            let b = render_display_template(template, &iota_object, &mut budget).unwrap();
            assert_eq!(a, b, "template {template:?}");
            assert!(a.ends_with("0xabc"), "template {template:?} -> {a:?}");
        }

        // GraphQL walks the raw struct, so `name.bytes` resolves to the vector
        // and is rejected as such; JSON-RPC has `name` flattened to a string.
        let mut budget = MAX_DISPLAY_OUTPUT_SIZE;
        assert!(matches!(
            render_display_template("{name.bytes}", &move_struct(FIELDS), &mut budget),
            Err(DisplayRenderError::Vector(_))
        ));
        assert!(matches!(
            render_display_template("{name.bytes}", &iota_struct(FIELDS), &mut budget),
            Err(DisplayRenderError::UnexpectedMoveValue(_))
        ));
    }

    #[test]
    fn test_option_value() {
        let option = |value: Option<IotaMoveValue>| IotaMoveValue::Option(Box::new(value));
        let object = IotaMoveStruct::WithFields(
            [
                (
                    "some".to_string(),
                    option(Some(IotaMoveValue::String("Boar".to_string()))),
                ),
                ("none".to_string(), option(None)),
                (
                    "vector".to_string(),
                    option(Some(IotaMoveValue::Vector(vec![]))),
                ),
            ]
            .into_iter()
            .collect(),
        );

        let mut budget = MAX_DISPLAY_OUTPUT_SIZE;
        assert_eq!(
            render_display_template("{some}|{none}", &object, &mut budget).unwrap(),
            "Boar|"
        );
        assert!(matches!(
            render_display_template("{vector}", &object, &mut budget),
            Err(DisplayRenderError::Vector(_))
        ));
    }
}
