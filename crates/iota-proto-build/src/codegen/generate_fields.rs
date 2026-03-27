// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use proc_macro2::TokenStream;
use prost_types::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorSet, field_descriptor_proto::Type,
};
use quote::quote;

use crate::{
    dependency_graph::{DependencyGraph, build_dependency_graph},
    ident::to_snake,
};

/// Information about a transparent wrapper message (one that should be skipped
/// in read_mask paths / field path builders).
pub(crate) struct TransparentInfo {
    /// True when the single inner field is a map field.
    pub is_map: bool,
    /// Simple Rust type name of the inner message (e.g. "Event"), None for
    /// maps.
    pub inner_message_name: Option<String>,
    /// Full protobuf type name of the inner message (e.g.
    /// ".iota.grpc.v1.event.Event"), None for maps.
    pub inner_full_type_name: Option<String>,
}

/// Parses the descriptor pool to find all messages with the
/// `field_mask_transparent` option set to true. Returns a map keyed by full
/// protobuf type name (e.g. `.iota.grpc.v1.event.Events`) with information
/// about the inner field that should be surfaced through the transparent
/// wrapper.
pub fn parse_transparent_messages_from_pool(
    pool: &prost_reflect::DescriptorPool,
) -> HashMap<String, TransparentInfo> {
    let mut map = HashMap::new();

    let transparent_ext = match pool.get_extension_by_name("iota.grpc.field_mask_transparent") {
        Some(ext) => ext,
        None => return map,
    };

    for message in pool.all_messages() {
        let options = message.options();
        if !options.has_extension(&transparent_ext) {
            continue;
        }

        let is_transparent = options
            .get_extension(&transparent_ext)
            .as_bool()
            .unwrap_or(false);

        if !is_transparent {
            continue;
        }

        // The wrapper must contain exactly one field.
        let fields: Vec<_> = message.fields().collect();
        if fields.len() != 1 {
            eprintln!(
                "Warning: transparent message {} has {} fields, expected 1 — skipping",
                message.full_name(),
                fields.len()
            );
            continue;
        }

        let inner_field = fields[0].clone();

        // Detect whether the inner field is a map field (a repeated field whose
        // message type has map_entry = true).
        let is_map = match inner_field.kind() {
            prost_reflect::Kind::Message(ref msg) => msg.is_map_entry(),
            _ => false,
        };

        let (inner_message_name, inner_full_type_name) = if is_map {
            (None, None)
        } else if let prost_reflect::Kind::Message(ref msg) = inner_field.kind() {
            let name = msg.name().to_owned();
            let full_name = format!(".{}", msg.full_name());
            (Some(name), Some(full_name))
        } else {
            (None, None)
        };

        let full_type_name = format!(".{}", message.full_name());
        map.insert(
            full_type_name,
            TransparentInfo {
                is_map,
                inner_message_name,
                inner_full_type_name,
            },
        );
    }

    map
}

// Helper to search nested messages
fn find_type_in_nested_messages(
    package: &str,
    nested: &[DescriptorProto],
    type_name: &str,
) -> Option<String> {
    for message in nested {
        if message.name() == type_name {
            // we use the last part of the package name
            return Some(package.split('.').next_back().unwrap_or(package).to_owned());
        }
        // Recurse into nested types
        if let Some(pkg) = find_type_in_nested_messages(package, &message.nested_type, type_name) {
            return Some(pkg);
        }
    }
    None
}

// Find which package a message type belongs to
fn find_package_for_type(
    type_name: &str,
    all_packages: &HashMap<String, FileDescriptorSet>,
) -> Option<String> {
    for (package, fds) in all_packages {
        for file in &fds.file {
            // Check top-level messages
            for message in &file.message_type {
                if message.name() == type_name {
                    // Check if this is a map entry (shouldn't be imported)
                    if message.options.as_ref().is_some_and(|o| o.map_entry()) {
                        return None;
                    }
                    // Extract the last part of the package name (e.g., "types" from
                    // "iota.grpc.v1.types")
                    return Some(package.split('.').next_back().unwrap_or(package).to_owned());
                }
                // Check nested messages (including map entries)
                if let Some(pkg) =
                    find_type_in_nested_messages(package, &message.nested_type, type_name)
                {
                    return Some(pkg);
                }
            }
        }
    }
    None
}

// Collects types from other packages that need to be imported.
// This function identifies all external message types (and their
// FieldPathBuilders) and tracks which package they come from to generate
// the correct import paths like `use crate::v1::object::Object`.
// For transparent wrapper types, the inner type is imported instead of the
// wrapper, because the generated field_info code references the inner type
// directly (e.g. `Some(Event::FIELDS)` instead of `Some(Events::FIELDS)`).
fn collect_external_types(
    current_package: &str,
    messages: &[DescriptorProto],
    external_types: &mut HashMap<String, String>,
    all_packages: &HashMap<String, FileDescriptorSet>,
    transparent_messages: &HashMap<String, TransparentInfo>,
) {
    for message in messages {
        for field in &message.field {
            // we skip google types‚
            if matches!(field.r#type(), Type::Message) && !field.type_name().contains("google") {
                let full_type_name = field.type_name();
                let field_message_name = full_type_name.split('.').next_back().unwrap();

                // Check if this type is external (from a different package)
                let is_external = !full_type_name.starts_with(&format!(".{}", current_package));

                if is_external {
                    // Check if this external type is a transparent wrapper
                    if let Some(info) = transparent_messages.get(full_type_name) {
                        // For transparent non-map types, import the inner type instead of the
                        // wrapper. The field_info code will reference the inner type directly.
                        if !info.is_map {
                            if let Some(inner_full_name) = &info.inner_full_type_name {
                                let inner_msg_name =
                                    inner_full_name.split('.').next_back().unwrap();
                                if let Some(package) =
                                    find_package_for_type(inner_msg_name, all_packages)
                                {
                                    external_types.insert(inner_msg_name.to_owned(), package);
                                }
                            }
                        }
                        // For transparent map types, no inner message to import
                        // (the field becomes a leaf/map
                        // field and doesn't reference a message type)
                    } else {
                        // Normal (non-transparent) external type
                        if let Some(package) =
                            find_package_for_type(field_message_name, all_packages)
                        {
                            external_types.insert(field_message_name.to_owned(), package);
                        }
                    }
                }
            }
        }
        // Recurse into nested messages
        collect_external_types(
            current_package,
            &message.nested_type,
            external_types,
            all_packages,
            transparent_messages,
        );
    }
}

// Helper function to collect imports for nested message types (but not their
// field builders)
fn collect_nested_message_imports(
    package: &str,
    messages: &[DescriptorProto],
    imports: &mut TokenStream,
    version: &str,
) {
    for message in messages {
        // Skip map entry messages
        if message.options.as_ref().is_some_and(|o| o.map_entry()) {
            continue;
        }

        // For messages with nested types, we need to import the nested message types
        // but not their field builders (since those are defined in this same file)
        if !message.nested_type.is_empty() {
            let parent_module = to_snake(message.name());
            let package_ident =
                quote::format_ident!("{}", package.split('.').next_back().unwrap_or(package));
            let parent_ident = quote::format_ident!("{parent_module}");

            for nested in &message.nested_type {
                // Skip map entry messages
                if nested.options.as_ref().is_some_and(|o| o.map_entry()) {
                    continue;
                }

                let nested_ident = quote::format_ident!("{}", nested.name());
                let version_ident = quote::format_ident!("{}", version);
                imports.extend(quote! {
                    #[allow(unused_imports)]
                    use crate::#version_ident::#package_ident::#parent_ident::#nested_ident;
                });

                // Recursively handle deeper nesting
                collect_nested_message_imports(package, &nested.nested_type, imports, version);
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct FileDescriptorWithPackageVersion {
    pub fd_set: FileDescriptorSet,
    pub version: String,
}

pub(crate) fn generate_field_info(
    packages: &HashMap<String, FileDescriptorWithPackageVersion>,
    out_dir: &Path,
    boxed_types: &[String],
    transparent_messages: &HashMap<String, TransparentInfo>,
) {
    let mut package_fds: HashMap<String, FileDescriptorSet> = HashMap::new();
    for (package, FileDescriptorWithPackageVersion { fd_set, .. }) in packages {
        package_fds.insert(package.clone(), fd_set.clone());
    }

    for (package, FileDescriptorWithPackageVersion { fd_set, version }) in packages {
        if package.contains("google") {
            continue;
        }

        let mut buf = String::new();
        let mut stream = TokenStream::new();

        // Collect external message types that need to be imported (maps type name ->
        // package name)
        let mut external_types: HashMap<String, String> = HashMap::new();
        for file in &fd_set.file {
            collect_external_types(
                package,
                &file.message_type,
                &mut external_types,
                &package_fds,
                transparent_messages,
            );
        }

        for file in &fd_set.file {
            stream.extend(generate_field_info_for_all_messages(
                package,
                &file.message_type,
                boxed_types,
                transparent_messages,
            ));
        }

        // Only generate file if there's actual content in the stream
        if !stream.is_empty() {
            // Sort external types by package and name
            let mut external_types: Vec<(String, String)> = external_types.into_iter().collect();
            external_types.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

            // Generate imports for external types with correct package paths
            let mut imports = TokenStream::new();
            let version_ident = quote::format_ident!("{}", version);
            for (type_name, package_name) in &external_types {
                let type_ident = quote::format_ident!("{type_name}");
                let builder_ident = quote::format_ident!("{type_name}FieldPathBuilder");
                let package_ident = quote::format_ident!("{package_name}");
                imports.extend(quote! {
                    #[allow(unused_imports)]
                    use crate::#version_ident::#package_ident::#type_ident;
                    #[allow(unused_imports)]
                    use crate::#version_ident::#package_ident::#builder_ident;
                });
            }

            // Also collect and import nested message types from the same package
            // (but not their field builders, since those are defined in this file)
            let mut nested_message_imports = TokenStream::new();
            for file in &fd_set.file {
                collect_nested_message_imports(
                    package,
                    &file.message_type,
                    &mut nested_message_imports,
                    version,
                );
            }

            let code = quote! {
                mod _field_impls {
                    #![allow(clippy::wrong_self_convention)]

                    use super::*;

                    use crate::field::MessageFields;
                    use crate::field::MessageField;

                    #imports
                    #nested_message_imports

                    #stream
                }

                pub use _field_impls::*;
            };

            let ast: syn::File = syn::parse2(code).expect("not a valid tokenstream");
            let code = prettyplease::unparse(&ast);

            // Add IOTA license header
            buf.push_str("// Copyright (c) Mysten Labs, Inc.\n");
            buf.push_str("// Modifications Copyright (c) 2025 IOTA Stiftung\n");
            buf.push_str("// SPDX-License-Identifier: Apache-2.0\n");
            buf.push('\n');
            buf.push_str(&code);

            let file_name = format!("{package}.field_info.rs");
            std::fs::write(out_dir.join(file_name), &buf).unwrap();
        }
    }
}

// Helper function to build a map of nested message names to their parent module
// names
fn build_nested_messages_map(messages: &[DescriptorProto]) -> HashMap<String, String> {
    let mut nested_messages = HashMap::new();

    for message in messages {
        if !message.nested_type.is_empty() {
            let parent_module = to_snake(message.name());
            for nested in &message.nested_type {
                // Skip map entry messages
                if nested.options.as_ref().is_some_and(|o| o.map_entry()) {
                    continue;
                }
                nested_messages.insert(nested.name().to_string(), parent_module.clone());

                // Recursively handle deeper nesting
                let deeper_nested = build_nested_messages_map(&nested.nested_type);

                // Merge deeper nested messages
                for (nested_name, nested_parent) in deeper_nested {
                    nested_messages
                        .insert(nested_name, format!("{}::{}", parent_module, nested_parent));
                }
            }
        }
    }

    nested_messages
}

// Helper function to recursively generate field info for all messages including
// nested ones
fn generate_field_info_for_all_messages(
    package: &str,
    messages: &[DescriptorProto],
    boxed_types: &[String],
    transparent_messages: &HashMap<String, TransparentInfo>,
) -> TokenStream {
    let mut stream = TokenStream::new();

    // Build the nested messages map for the entire message hierarchy
    let nested_messages = build_nested_messages_map(messages);

    // Build dependency graph for circular reference detection
    let dependency_graph = build_dependency_graph(messages, package, "");

    // First pass: Generate nested modules first so they're defined before being
    // used
    for message in messages {
        // Skip map entry messages
        if message.options.as_ref().is_some_and(|o| o.map_entry()) {
            continue;
        }

        // Generate nested modules for nested messages
        if !message.nested_type.is_empty() {
            let module_name = quote::format_ident!("{}", to_snake(message.name()));
            let nested_content = generate_field_info_for_all_messages(
                package,
                &message.nested_type,
                boxed_types,
                transparent_messages,
            );

            if !nested_content.is_empty() {
                stream.extend(quote! {
                    pub mod #module_name {
                        use super::*;

                        #nested_content
                    }
                });
            }
        }
    }

    // Second pass: Generate top-level messages after nested modules are defined
    for message in messages {
        // Skip map entry messages
        if message.options.as_ref().is_some_and(|o| o.map_entry()) {
            continue;
        }

        stream.extend(generate_field_info_for_message(
            package,
            message,
            boxed_types,
            &dependency_graph,
            &nested_messages,
            transparent_messages,
        ));
    }

    stream
}

fn generate_field_info_for_message(
    package: &str,
    message: &DescriptorProto,
    boxed_types: &[String],
    dependency_graph: &DependencyGraph,
    nested_messages: &HashMap<String, String>,
    transparent_messages: &HashMap<String, TransparentInfo>,
) -> TokenStream {
    let map_types: HashSet<String> = message
        .nested_type
        .iter()
        .filter_map(|m| {
            if m.options.as_ref().is_some_and(|o| o.map_entry()) {
                Some(m.name().to_owned())
            } else {
                None
            }
        })
        .collect();

    let constants = generate_field_constants(
        package,
        message,
        boxed_types,
        dependency_graph,
        &map_types,
        transparent_messages,
    );
    let oneof_constants = generate_oneof_name_constants(message);
    let message_fields_impl = generate_message_fields_impl(message);
    let field_path_builders = generate_field_path_builders_impl(
        package,
        message,
        &map_types,
        nested_messages,
        boxed_types,
        transparent_messages,
    );

    quote! {
        #constants
        #oneof_constants
        #message_fields_impl
        #field_path_builders
    }
}

/// Generates `pub const {NAME}_ONEOF: &'static str = "{name}";` constants for
/// each real `oneof` declaration in a message. Synthetic oneofs created by the
/// proto3-optional feature are excluded.
fn generate_oneof_name_constants(message: &DescriptorProto) -> TokenStream {
    if message.oneof_decl.is_empty() {
        return TokenStream::new();
    }

    // Determine which oneof indices are "real" (not synthetic proto3-optional
    // oneofs). A synthetic oneof only contains fields with proto3_optional = true.
    let real_oneof_indices: HashSet<i32> = message
        .field
        .iter()
        .filter_map(|field| {
            if field.oneof_index.is_some() && !field.proto3_optional() {
                field.oneof_index
            } else {
                None
            }
        })
        .collect();

    if real_oneof_indices.is_empty() {
        return TokenStream::new();
    }

    let message_ident = quote::format_ident!("{}", message.name());
    let mut consts = TokenStream::new();

    for (idx, oneof) in message.oneof_decl.iter().enumerate() {
        if !real_oneof_indices.contains(&(idx as i32)) {
            continue;
        }
        let name = oneof.name();
        let ident = quote::format_ident!("{}_ONEOF", name.to_ascii_uppercase());
        consts.extend(quote! {
            pub const #ident: &'static str = #name;
        });
    }

    if consts.is_empty() {
        return TokenStream::new();
    }

    quote! {
        impl #message_ident {
            #consts
        }
    }
}

fn generate_field_constants(
    package: &str,
    message: &DescriptorProto,
    boxed_types: &[String],
    dependency_graph: &DependencyGraph,
    map_types: &HashSet<String>,
    transparent_messages: &HashMap<String, TransparentInfo>,
) -> TokenStream {
    let message_ident = quote::format_ident!("{}", message.name());
    let mut field_consts = TokenStream::new();

    for field in &message.field {
        field_consts.extend(generate_field_constant(
            package,
            message.name(),
            field,
            boxed_types,
            dependency_graph,
            map_types,
            transparent_messages,
        ));
    }

    quote! {
        impl #message_ident {
            #field_consts
        }
    }
}

fn generate_message_fields_impl(message: &DescriptorProto) -> TokenStream {
    let message_ident = quote::format_ident!("{}", message.name());

    let mut field_refs = TokenStream::new();

    for field in &message.field {
        field_refs.extend(generate_field_reference(field));
    }

    // Collect real (non-synthetic) oneof names for the ONEOFS constant.
    let real_oneof_names = get_real_oneof_names(message);

    let oneofs_impl = if real_oneof_names.is_empty() {
        TokenStream::new()
    } else {
        let names = real_oneof_names.iter().map(|name| {
            quote! { #name, }
        });
        quote! {
            const ONEOFS: &'static [&'static str] = &[
                #(#names)*
            ];
        }
    };

    quote! {
        impl MessageFields for #message_ident {
            const FIELDS: &'static [&'static MessageField] = &[
                #field_refs
            ];
            #oneofs_impl
        }
    }
}

/// Returns the names of real (non-synthetic proto3-optional) oneofs in a
/// message.
fn get_real_oneof_names(message: &DescriptorProto) -> Vec<String> {
    if message.oneof_decl.is_empty() {
        return Vec::new();
    }

    let real_oneof_indices: HashSet<i32> = message
        .field
        .iter()
        .filter_map(|field| {
            if field.oneof_index.is_some() && !field.proto3_optional() {
                field.oneof_index
            } else {
                None
            }
        })
        .collect();

    message
        .oneof_decl
        .iter()
        .enumerate()
        .filter_map(|(idx, oneof)| {
            if real_oneof_indices.contains(&(idx as i32)) {
                Some(oneof.name().to_string())
            } else {
                None
            }
        })
        .collect()
}

fn generate_field_constant(
    package: &str,
    message_name: &str,
    field: &FieldDescriptorProto,
    boxed_types: &[String],
    dependency_graph: &DependencyGraph,
    map_types: &HashSet<String>,
    transparent_messages: &HashMap<String, TransparentInfo>,
) -> TokenStream {
    let ident = quote::format_ident!("{}_FIELD", field.name().to_ascii_uppercase());
    let name = field.name();
    let json_name = field.json_name();
    let number = field.number();

    // Check if the field is optional in the proto definition
    let is_proto3_optional = field.proto3_optional.unwrap_or(false);

    let (is_map, message_fields) =
        if matches!(field.r#type(), Type::Message) && !field.type_name().contains("google") {
            let full_type_name = field.type_name();
            let field_message_name = full_type_name.split('.').next_back().unwrap();

            // Check for circular references that need to be broken:
            // 1. Self-reference (field_message_name == message_name)
            // 2. Map entry types
            // 3. Fields that are boxed AND create circular dependencies in the message
            //    graph
            let field_full_path = format!(".{}.{}.{}", package, message_name, field.name());
            let is_boxed = boxed_types.iter().any(|boxed_path| {
                let boxed_path = boxed_path.strip_prefix('.').unwrap_or(boxed_path);
                field_full_path
                    .strip_prefix('.')
                    .unwrap_or(&field_full_path)
                    == boxed_path
            });

            let is_circular_reference = is_boxed
                && dependency_graph.has_circular_dependency(message_name, field_message_name);

            if field_message_name == message_name || is_circular_reference {
                (quote! { false }, quote! { None })
            } else if map_types.contains(field_message_name) {
                (quote! { true }, quote! { None })
            } else if let Some(info) = transparent_messages.get(full_type_name) {
                // Field points to a transparent wrapper: flatten through it.
                if info.is_map {
                    // The wrapper's inner field is a map → treat this field as a map directly.
                    (quote! { true }, quote! { None })
                } else if let Some(inner_name) = &info.inner_message_name {
                    // The wrapper's inner field is a repeated message → use the inner type's
                    // FIELDS.
                    let inner = quote::format_ident!("{inner_name}");
                    (quote! { false }, quote! { Some(#inner::FIELDS) })
                } else {
                    (quote! { false }, quote! { None })
                }
            } else {
                let field_message = quote::format_ident!("{field_message_name}");
                (quote! { false }, quote! { Some(#field_message::FIELDS) })
            }
        } else {
            (quote! { false }, quote! { None })
        };

    quote! {
        pub const #ident: &'static MessageField = &MessageField {
            name: #name,
            json_name: #json_name,
            number: #number,
            is_optional: #is_proto3_optional,
            is_map: #is_map,
            message_fields: #message_fields,
        };
    }
}

fn generate_field_reference(field: &FieldDescriptorProto) -> TokenStream {
    let ident = quote::format_ident!("{}_FIELD", field.name().to_ascii_uppercase());

    quote! {
        Self::#ident,
    }
}

fn generate_field_path_builders_impl(
    package: &str,
    message: &DescriptorProto,
    map_types: &HashSet<String>,
    nested_messages: &HashMap<String, String>,
    boxed_types: &[String],
    transparent_messages: &HashMap<String, TransparentInfo>,
) -> TokenStream {
    let message_ident = quote::format_ident!("{}", message.name());
    let builder_ident = quote::format_ident!("{}FieldPathBuilder", message.name());

    let mut field_chain_methods = TokenStream::new();

    for field in &message.field {
        field_chain_methods.extend(generate_field_chain_methods(
            package,
            message.name(),
            field,
            map_types,
            nested_messages,
            boxed_types,
            transparent_messages,
        ));
    }

    quote! {
        impl #message_ident {
            pub fn path_builder() -> #builder_ident {
                #builder_ident::new()
            }
        }

        pub struct #builder_ident {
            path: Vec<&'static str>,
        }

        impl #builder_ident {
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                Self {
                    path: Default::default(),
                }
            }

            #[doc(hidden)]
            pub fn new_with_base(base: Vec<&'static str>) -> Self {
                Self { path: base }
            }

            pub fn finish(self) -> String {
                self.path.join(".")
            }

            #field_chain_methods
        }
    }
}

// Helper function to check if a field should be boxed based on its path
fn should_box_field(message_type: &str, field_name: &str, boxed_types: &[String]) -> bool {
    let field_path = format!("{}.{}", message_type, field_name);
    boxed_types.iter().any(|boxed_path| {
        // Remove leading dot if present
        let boxed_path = boxed_path.strip_prefix('.').unwrap_or(boxed_path);
        field_path == boxed_path
    })
}

fn generate_field_chain_methods(
    package: &str,
    message_name: &str,
    field: &FieldDescriptorProto,
    map_types: &HashSet<String>,
    nested_messages: &HashMap<String, String>, // Maps message name to parent module name
    boxed_types: &[String],
    transparent_messages: &HashMap<String, TransparentInfo>,
) -> TokenStream {
    let message_ident = quote::format_ident!("{message_name}");
    let field_const = quote::format_ident!("{}_FIELD", field.name().to_ascii_uppercase());
    let name = if field.name() == "type" {
        quote::format_ident!("r#{}", field.name())
    } else {
        quote::format_ident!("{}", field.name())
    };

    // we need to ignore google types, because we don't generate builders for them
    if matches!(field.r#type(), Type::Message) && !field.type_name().contains("google") {
        let full_type_name = field.type_name();
        let field_message_name = full_type_name.split('.').next_back().unwrap();

        if field_message_name == message_name || map_types.contains(field_message_name) {
            quote! {
                pub fn #name(mut self) -> String {
                    self.path.push(#message_ident::#field_const.name);
                    self.finish()
                }
            }
        } else if let Some(info) = transparent_messages.get(full_type_name) {
            // Field points to a transparent wrapper: generate builder for inner type.
            if info.is_map {
                // Transparent wrapper around a map → leaf method returning String.
                quote! {
                    pub fn #name(mut self) -> String {
                        self.path.push(#message_ident::#field_const.name);
                        self.finish()
                    }
                }
            } else if let Some(inner_name) = &info.inner_message_name {
                // Transparent wrapper around a repeated message → return inner type's builder.
                let builder_name = format!("{inner_name}FieldPathBuilder");
                let return_type =
                    if let Some(parent_module) = nested_messages.get(inner_name.as_str()) {
                        let module_ident = quote::format_ident!("{}", parent_module);
                        let builder_ident = quote::format_ident!("{}", builder_name);
                        quote! { #module_ident::#builder_ident }
                    } else {
                        let builder_ident = quote::format_ident!("{}", builder_name);
                        quote! { #builder_ident }
                    };
                quote! {
                    pub fn #name(mut self) -> #return_type {
                        self.path.push(#message_ident::#field_const.name);
                        #return_type::new_with_base(self.path)
                    }
                }
            } else {
                quote! {
                    pub fn #name(mut self) -> String {
                        self.path.push(#message_ident::#field_const.name);
                        self.finish()
                    }
                }
            }
        } else {
            let builder_name = format!("{field_message_name}FieldPathBuilder");

            // Check if the target message is nested and needs module qualification
            let return_type = if let Some(parent_module) = nested_messages.get(field_message_name) {
                let module_ident = quote::format_ident!("{}", parent_module);
                let builder_ident = quote::format_ident!("{}", builder_name);
                quote! { #module_ident::#builder_ident }
            } else {
                let builder_ident = quote::format_ident!("{}", builder_name);
                quote! { #builder_ident }
            };

            // Check if this field should be boxed
            let full_message_type = format!(".{}.{}", package, message_name);
            if should_box_field(&full_message_type, field.name(), boxed_types) {
                quote! {
                    pub fn #name(mut self) -> Box<#return_type> {
                        self.path.push(#message_ident::#field_const.name);
                        Box::new(#return_type::new_with_base(self.path))
                    }
                }
            } else {
                quote! {
                    pub fn #name(mut self) -> #return_type {
                        self.path.push(#message_ident::#field_const.name);
                        #return_type::new_with_base(self.path)
                    }
                }
            }
        }
    } else {
        quote! {
            pub fn #name(mut self) -> String {
                self.path.push(#message_ident::#field_const.name);
                self.finish()
            }
        }
    }
}
