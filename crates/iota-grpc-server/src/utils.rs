// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::{Identifier, MAX_IDENTIFIER_LENGTH, StructTag, TypeParseError, TypeTag};
use iota_types::transaction::{
    move_core_identifier_validity_check, type_tags_identifiers_validity_check,
};

use crate::GrpcReader;

/// Parses the optional module or function name of a filter. `field` names it
/// in the error.
pub(crate) fn parse_filter_identifier(
    name: Option<String>,
    field: &str,
) -> Result<Option<Identifier>, String> {
    name.map(|name| {
        // The move-core check rejects `<SELF>`, which the SDK parser accepts,
        // and keeps invalid names away from the SDK parser, whose errors copy
        // the rest of the input.
        if name.len() <= MAX_IDENTIFIER_LENGTH
            && move_core_types::identifier::Identifier::is_valid(&name)
        {
            Ok(Identifier::new_unchecked(name))
        } else {
            Err(format!(
                "invalid {field} name: {}",
                quote_filter_input(&name)
            ))
        }
    })
    .transpose()
}

/// Parses the struct tag of an event filter. For a syntax error, the error
/// gives the byte offset and the input where parsing stopped.
pub(crate) fn parse_filter_struct_tag(struct_tag: &str) -> Result<StructTag, String> {
    let tag: StructTag = struct_tag.parse().map_err(|error| match error {
        // `input` is the rest of the struct tag from where parsing stopped.
        TypeParseError::Parse { input, .. } => format!(
            "invalid struct tag: unexpected input at byte {}: {}",
            struct_tag.len().saturating_sub(input.len()),
            quote_filter_input(&input)
        ),
        error => format!("invalid struct tag: {error}"),
    })?;
    // The SDK parser accepts `<SELF>`, which never names a module or type.
    move_core_identifier_validity_check(tag.module())
        .and_then(|()| move_core_identifier_validity_check(tag.name()))
        .and_then(|()| type_tags_identifiers_validity_check(tag.type_params()))
        .map_err(|_| format!("invalid struct tag: {}", quote_filter_input(struct_tag)))?;
    Ok(tag)
}

/// Quotes a filter value for an error message. Values longer than
/// [`MAX_IDENTIFIER_LENGTH`] bytes are cut to that length before quoting,
/// which escapes control characters.
pub(crate) fn quote_filter_input(value: &str) -> String {
    if value.len() <= MAX_IDENTIFIER_LENGTH {
        return format!("{value:?}");
    }
    let end = value.floor_char_boundary(MAX_IDENTIFIER_LENGTH);
    format!("{:?}... ({} bytes)", &value[..end], value.len())
}

/// Longest parser error, in bytes, that an error message repeats.
const MAX_PARSE_ERROR_LENGTH: usize = 256;

/// Formats a parser error for an error message, cut to
/// [`MAX_PARSE_ERROR_LENGTH`] bytes. The move-core parser's errors include the
/// rest of the input, which can be as long as the request.
pub(crate) fn shorten_parse_error(error: impl std::fmt::Display) -> String {
    let message = error.to_string();
    if message.len() <= MAX_PARSE_ERROR_LENGTH {
        return message;
    }
    let end = message.floor_char_boundary(MAX_PARSE_ERROR_LENGTH);
    format!("{}... ({} bytes)", &message[..end], message.len())
}

/// Per-item overhead when an item is placed in a protobuf `repeated`
/// length-delimited field (field number 1-15):
///   field_tag (1 byte) + length_delimiter_varint
pub(crate) fn repeated_field_item_overhead(item_encoded_len: usize) -> usize {
    1 + prost::length_delimiter_len(item_encoded_len)
}

/// Wrapper overhead for a `CheckpointData` oneof message wrapping an inner
/// message (e.g. `ExecutedTransactions` or `Events`).
///
/// Encoding: field_tag (1 byte) + length_delimiter(inner_encoded_len).
pub(crate) fn checkpoint_data_wrapper_overhead(inner_encoded_len: usize) -> usize {
    1 + prost::length_delimiter_len(inner_encoded_len)
}

/// Exact overhead for `has_next: true` in `GetObjectsResponse` /
/// `GetTransactionsResponse`. A proto3 bool field set to `true` encodes as
/// field_tag (1 byte) + varint(1) (1 byte) = 2 bytes.
pub(crate) const HAS_NEXT_TRUE_OVERHEAD: usize = 2;

pub(crate) fn render_json(
    grpc_reader: Arc<GrpcReader>,
    max_json_move_value_size: usize,
    type_tag: &TypeTag,
    contents: &[u8],
) -> Option<prost_types::Value> {
    // JSON rendering is best-effort - log errors but don't fail the request
    let layout = grpc_reader
        .get_type_layout(type_tag)
        .map_err(|e| tracing::debug!("unable to get type layout for JSON rendering: {e}"))
        .ok()
        .flatten()?;

    render_json_with_layout(max_json_move_value_size, &layout, contents)
}

/// Renders Move `contents` to JSON against an already-resolved `layout`, for
/// callers that hold the layout and need not resolve it from a type tag.
pub(crate) fn render_json_with_layout(
    max_json_move_value_size: usize,
    layout: &move_core_types::annotated_value::MoveTypeLayout,
    contents: &[u8],
) -> Option<prost_types::Value> {
    // JSON rendering is best-effort - log errors but don't fail the request
    iota_types::proto_value::ProtoVisitorBuilder::new(max_json_move_value_size)
        .deserialize_value(contents, layout)
        .map_err(|e| tracing::debug!("unable to convert move value to JSON: {e}"))
        .ok()
}

/// Returns `true` only when `field` is named explicitly in `mask`, rather than
/// merely covered by a `*` wildcard or a bare parent.
///
/// Fields rendered from Move type layouts (an object's `json`, a dynamic
/// field's `json_name`, an event's `json_contents`, a command output's `json`)
/// are gated on this, so the heavy layout resolution runs only when the client
/// asks for the field by name.
pub(crate) fn mask_names_field(mask: &iota_grpc_types::field::FieldMaskTree, field: &str) -> bool {
    // `explicit_children`, not `map_keys`: a node may be a wildcard for its
    // ordinary fields yet still carry an explicit-only leaf (a `json` requested
    // alongside a bare parent), which `map_keys` reports as "no filter".
    mask.explicit_children().any(|name| name == field)
}

/// Traffic-control cost of one returned item under `mask`: a base of 1 plus the
/// weight of each `(field, weight)` in `heavy_fields` that `mask` names
/// explicitly.
///
/// These fields each cost extra server work — Move type-layout resolution, and
/// for `display` a display-index lookup — so a read that asks for them draws
/// down the caller's rendered-field budget. A read that names none renders
/// nothing extra and weighs 0.
pub(crate) fn read_mask_rendered_field_weight(
    mask: &iota_grpc_types::field::FieldMaskTree,
    heavy_fields: &[(&str, usize)],
) -> usize {
    heavy_fields
        .iter()
        .filter(|(field, _)| mask_names_field(mask, field))
        .map(|(_, weight)| weight)
        .sum::<usize>()
}

/// Per-item rendered-field weight of an object read under `read_mask`: a weight
/// for each layout-resolved field it names — `json` (Move type-layout
/// resolution) and `display` (layout resolution plus a display-index lookup). A
/// read that names neither renders nothing and weighs 0.
pub(crate) fn object_rendered_field_weight(read_mask: Option<&prost_types::FieldMask>) -> usize {
    use iota_grpc_types::{
        field::{FieldMaskTree, SERVER_RENDERED_FIELD_NAMES},
        v1::object::Object,
    };

    read_mask.map_or(0, |mask| {
        read_mask_rendered_field_weight(
            &FieldMaskTree::from_field_mask_with_exceptions(mask, SERVER_RENDERED_FIELD_NAMES),
            &[
                (Object::JSON_FIELD.name, 1),
                (Object::DISPLAY_FIELD.name, 2),
            ],
        )
    })
}

/// Per-item rendered-field weight of a transaction read under `read_mask`: a
/// weight when the mask names the per-event `json_contents`, which the server
/// renders by resolving each event's Move type layout. A read that does not
/// name it renders nothing extra and weighs 0.
pub(crate) fn transaction_rendered_field_weight(
    read_mask: Option<&prost_types::FieldMask>,
) -> usize {
    use iota_grpc_types::{
        field::{FieldMaskTree, SERVER_RENDERED_FIELD_NAMES},
        v1::{
            event::Event,
            transaction::{ExecutedTransaction, TransactionEvents},
        },
    };

    read_mask.map_or(0, |mask| {
        let tree =
            FieldMaskTree::from_field_mask_with_exceptions(mask, SERVER_RENDERED_FIELD_NAMES);
        let renders_json_contents = tree
            .subtree(ExecutedTransaction::EVENTS_FIELD.name)
            .and_then(|events| events.subtree(TransactionEvents::EVENTS_FIELD.name))
            .is_some_and(|event| mask_names_field(&event, Event::JSON_CONTENTS_FIELD.name));
        usize::from(renders_json_contents)
    })
}

/// Per-item rendered-field weight of a dynamic-field read under `read_mask`: the
/// weight of each layout-resolved field it names — the field's own `json_name`,
/// and `json`/`display` on the nested `field_object` and `child_object`. A read
/// that names none renders nothing extra and weighs 0.
pub(crate) fn dynamic_field_rendered_field_weight(
    read_mask: Option<&prost_types::FieldMask>,
) -> usize {
    use iota_grpc_types::{
        field::{FieldMaskTree, SERVER_RENDERED_FIELD_NAMES},
        v1::{dynamic_field::DynamicField, object::Object},
    };

    read_mask.map_or(0, |mask| {
        let tree =
            FieldMaskTree::from_field_mask_with_exceptions(mask, SERVER_RENDERED_FIELD_NAMES);
        let object_fields = [
            (Object::JSON_FIELD.name, 1),
            (Object::DISPLAY_FIELD.name, 2),
        ];
        let nested: usize = [
            DynamicField::FIELD_OBJECT_FIELD.name,
            DynamicField::CHILD_OBJECT_FIELD.name,
        ]
        .into_iter()
        .filter_map(|field| tree.subtree(field))
        .map(|object| {
            object_fields
                .iter()
                .filter(|(field, _)| mask_names_field(&object, field))
                .map(|(_, weight)| *weight)
                .sum::<usize>()
        })
        .sum();
        read_mask_rendered_field_weight(&tree, &[(DynamicField::JSON_NAME_FIELD.name, 1)]) + nested
    })
}

#[cfg(test)]
mod tests {
    use iota_grpc_types::field::{FieldMaskTree, FieldMaskUtil};
    use prost_types::FieldMask;

    use super::{
        dynamic_field_rendered_field_weight, mask_names_field, object_rendered_field_weight,
        transaction_rendered_field_weight,
    };

    fn tree(paths: &[&str]) -> FieldMaskTree {
        FieldMaskTree::from(FieldMask::from_paths(paths))
    }

    #[test]
    fn mask_names_field_requires_explicit_entry() {
        // Named explicitly → included.
        assert!(mask_names_field(&tree(&["json"]), "json"));
        assert!(mask_names_field(&tree(&["bcs", "json"]), "json"));

        // A sibling field is named, but not this one → excluded.
        assert!(!mask_names_field(&tree(&["bcs"]), "json"));

        // Empty mask → excluded.
        assert!(!mask_names_field(&tree(&[]), "json"));
    }

    #[test]
    fn mask_names_field_excludes_wildcard() {
        // A `*` wildcard covers every field but names none, so a heavy field
        // is not pulled in.
        assert!(!mask_names_field(&FieldMaskTree::new_wildcard(), "json"));
        assert!(!mask_names_field(&tree(&["*"]), "json"));
    }

    #[test]
    fn mask_names_field_excludes_bare_parent_subtree() {
        // A bare parent (`events`) yields a wildcard subtree, which must not
        // pull in the heavy child (`json_contents`); only `events.json_contents`
        // does.
        let bare = tree(&["events"]);
        let bare_subtree = bare.subtree("events").unwrap();
        assert!(!mask_names_field(&bare_subtree, "json_contents"));

        let explicit = tree(&["events.json_contents"]);
        let explicit_subtree = explicit.subtree("events").unwrap();
        assert!(mask_names_field(&explicit_subtree, "json_contents"));
    }

    #[test]
    fn object_rendered_field_weight_counts_heavy_fields() {
        // No mask (defaults) and plain reads render nothing extra: weight 0.
        assert_eq!(object_rendered_field_weight(None), 0);
        assert_eq!(
            object_rendered_field_weight(Some(&FieldMask::from_paths([
                "reference.object_id",
                "bcs"
            ]))),
            0
        );
        // A `*` wildcard renders neither heavy field.
        assert_eq!(
            object_rendered_field_weight(Some(&FieldMask::from_paths(["*"]))),
            0
        );

        // `json` adds 1, `display` adds 2, and they add together.
        assert_eq!(
            object_rendered_field_weight(Some(&FieldMask::from_paths(["json"]))),
            1
        );
        assert_eq!(
            object_rendered_field_weight(Some(&FieldMask::from_paths(["display"]))),
            2
        );
        assert_eq!(
            object_rendered_field_weight(Some(&FieldMask::from_paths(["bcs", "json", "display"]))),
            3
        );

        // `*` combined with an explicit-only field is charged for the field.
        assert_eq!(
            object_rendered_field_weight(Some(&FieldMask::from_paths(["*", "json"]))),
            1
        );
        assert_eq!(
            object_rendered_field_weight(Some(&FieldMask::from_paths(["*", "json", "display"]))),
            3
        );
    }

    #[test]
    fn transaction_rendered_field_weight_counts_json_contents() {
        // No mask (defaults) and plain reads render nothing extra: weight 0.
        assert_eq!(transaction_rendered_field_weight(None), 0);
        assert_eq!(
            transaction_rendered_field_weight(Some(&FieldMask::from_paths([
                "transaction",
                "effects"
            ]))),
            0
        );
        // A bare `events` parent or a `*` wildcard does not pull in the
        // per-event `json_contents`.
        assert_eq!(
            transaction_rendered_field_weight(Some(&FieldMask::from_paths(["events"]))),
            0
        );
        assert_eq!(
            transaction_rendered_field_weight(Some(&FieldMask::from_paths(["*"]))),
            0
        );
        // Naming `events.events.json_contents` explicitly adds the weight, even
        // alongside a wildcard.
        assert_eq!(
            transaction_rendered_field_weight(Some(&FieldMask::from_paths([
                "events.events.json_contents"
            ]))),
            1
        );
        assert_eq!(
            transaction_rendered_field_weight(Some(&FieldMask::from_paths([
                "*",
                "events.events.json_contents"
            ]))),
            1
        );
    }

    #[test]
    fn dynamic_field_rendered_field_weight_counts_json_name_and_nested_objects() {
        // No mask (defaults) and plain reads render nothing extra: weight 0.
        assert_eq!(dynamic_field_rendered_field_weight(None), 0);
        assert_eq!(
            dynamic_field_rendered_field_weight(Some(&FieldMask::from_paths([
                "parent", "field_id"
            ]))),
            0
        );
        // The field's own `json_name` adds 1.
        assert_eq!(
            dynamic_field_rendered_field_weight(Some(&FieldMask::from_paths(["json_name"]))),
            1
        );
        // A nested object's `json` adds 1 and `display` adds 2.
        assert_eq!(
            dynamic_field_rendered_field_weight(Some(&FieldMask::from_paths([
                "field_object.json"
            ]))),
            1
        );
        assert_eq!(
            dynamic_field_rendered_field_weight(Some(&FieldMask::from_paths([
                "child_object.display"
            ]))),
            2
        );
        // A bare nested parent or a `*` wildcard pulls in neither.
        assert_eq!(
            dynamic_field_rendered_field_weight(Some(&FieldMask::from_paths(["field_object"]))),
            0
        );
        assert_eq!(
            dynamic_field_rendered_field_weight(Some(&FieldMask::from_paths(["*"]))),
            0
        );
        // They all add together.
        assert_eq!(
            dynamic_field_rendered_field_weight(Some(&FieldMask::from_paths([
                "json_name",
                "field_object.json",
                "child_object.display"
            ]))),
            1 + 1 + 2
        );
    }

    #[test]
    fn gate_detects_explicit_only_leaf_with_parent_and_with_star() {
        use iota_grpc_types::field::SERVER_RENDERED_FIELD_NAMES;

        // Named parent + explicit leaf: `json` sits under the object subtree,
        // while the parent still selects its ordinary fields.
        let mask = FieldMask::from_paths(["output_objects", "output_objects.json"]);
        let tree =
            FieldMaskTree::from_field_mask_with_exceptions(&mask, SERVER_RENDERED_FIELD_NAMES);
        let objects = tree.subtree("output_objects").unwrap();
        assert!(mask_names_field(&objects, "json"));
        assert!(objects.contains("reference"));

        // `*` + explicit leaf at the object level: `json` is a direct child and
        // the wildcard still selects everything else.
        let mask = FieldMask::from_paths(["*", "json"]);
        let tree =
            FieldMaskTree::from_field_mask_with_exceptions(&mask, SERVER_RENDERED_FIELD_NAMES);
        assert!(mask_names_field(&tree, "json"));
        assert!(tree.contains("reference"));
    }
}
