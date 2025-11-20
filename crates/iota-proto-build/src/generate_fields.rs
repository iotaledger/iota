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
                    // "iota.grpc.v0.types")
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
// the correct import paths like `use crate::v0::object::Object`.
fn collect_external_types(
    current_package: &str,
    messages: &[DescriptorProto],
    external_types: &mut HashMap<String, String>,
    all_packages: &HashMap<String, FileDescriptorSet>,
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
                    // Find which package this type belongs to (returns None for map entries)
                    if let Some(package) = find_package_for_type(field_message_name, all_packages) {
                        external_types.insert(field_message_name.to_owned(), package);
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
            );
        }

        for file in &fd_set.file {
            stream.extend(generate_field_info_for_all_messages(
                package,
                &file.message_type,
                boxed_types,
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
            let nested_content =
                generate_field_info_for_all_messages(package, &message.nested_type, boxed_types);

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

    let constants =
        generate_field_constants(package, message, boxed_types, dependency_graph, &map_types);
    let message_fields_impl = generate_message_fields_impl(message);
    let field_path_builders = generate_field_path_builders_impl(
        package,
        message,
        &map_types,
        nested_messages,
        boxed_types,
    );

    quote! {
        #constants
        #message_fields_impl
        #field_path_builders
    }
}

fn generate_field_constants(
    package: &str,
    message: &DescriptorProto,
    boxed_types: &[String],
    dependency_graph: &DependencyGraph,
    map_types: &HashSet<String>,
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

    quote! {
        impl MessageFields for #message_ident {
            const FIELDS: &'static [&'static MessageField] = &[
                #field_refs
            ];
        }
    }
}

fn generate_field_constant(
    package: &str,
    message_name: &str,
    field: &FieldDescriptorProto,
    boxed_types: &[String],
    dependency_graph: &DependencyGraph,
    map_types: &HashSet<String>,
) -> TokenStream {
    let ident = quote::format_ident!("{}_FIELD", field.name().to_ascii_uppercase());
    let name = field.name();
    let json_name = field.json_name();
    let number = field.number();

    let message_fields =
        if matches!(field.r#type(), Type::Message) && !field.type_name().contains("google") {
            let field_message_name = field.type_name().split('.').next_back().unwrap();

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

            if field_message_name == message_name
                || map_types.contains(field_message_name)
                || is_circular_reference
            {
                quote! { None }
            } else {
                let field_message = quote::format_ident!("{field_message_name}");
                quote! { Some(#field_message::FIELDS) }
            }
        } else {
            quote! { None }
        };

    quote! {
        pub const #ident: &'static MessageField = &MessageField {
            name: #name,
            json_name: #json_name,
            number: #number,
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
        let field_message_name = field.type_name().split('.').next_back().unwrap();

        if field_message_name == message_name || map_types.contains(field_message_name) {
            quote! {
                pub fn #name(mut self) -> String {
                    self.path.push(#message_ident::#field_const.name);
                    self.finish()
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
