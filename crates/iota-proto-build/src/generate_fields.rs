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

// Collects types from other packages that need to be imported.
// IOTA uses separate packages (common, events, checkpoints), so when
// events.proto references Address from common.proto, we need explicit imports.
// This function identifies all external message types (and their
// FieldPathBuilders) to generate the necessary `use crate::v0::common::*`
// statements in the generated code.
fn collect_external_types(
    messages: &[DescriptorProto],
    local_messages: &HashSet<String>,
    external_types: &mut HashSet<String>,
) {
    for message in messages {
        for field in &message.field {
            if matches!(field.r#type(), Type::Message) && !field.type_name().contains("google") {
                let field_message_name = field.type_name().split('.').next_back().unwrap();
                if !local_messages.contains(field_message_name) {
                    external_types.insert(field_message_name.to_owned());
                }
            }
        }
        // Recurse into nested messages
        collect_external_types(&message.nested_type, local_messages, external_types);
    }
}

pub(crate) fn generate_field_info(packages: &HashMap<String, FileDescriptorSet>, out_dir: &Path) {
    for (package, fds) in packages {
        if package.contains("google") {
            continue;
        }

        let mut buf = String::new();
        let mut stream = TokenStream::new();

        // Collect all message names from this package to know what's local
        let local_messages: HashSet<String> = fds
            .file
            .iter()
            .flat_map(|f| f.message_type.iter().map(|m| m.name().to_owned()))
            .collect();

        // Collect external message types that need to be imported
        let mut external_types: HashSet<String> = HashSet::new();
        for file in &fds.file {
            collect_external_types(&file.message_type, &local_messages, &mut external_types);
        }

        for file in &fds.file {
            for message in &file.message_type {
                stream.extend(generate_field_info_for_message(message));
            }
        }

        // Sort external types by name
        let mut external_types: Vec<String> = external_types.into_iter().collect();
        external_types.sort();

        // Generate imports for external types
        let mut imports = TokenStream::new();
        for ext_type in &external_types {
            let type_ident = quote::format_ident!("{}", ext_type);
            let builder_ident = quote::format_ident!("{}FieldPathBuilder", ext_type);
            imports.extend(quote! {
                #[allow(unused_imports)]
                use crate::v0::common::#type_ident;
                #[allow(unused_imports)]
                use crate::v0::common::#builder_ident;
            });
        }

        let code = quote! {
            mod _field_impls {
                #![allow(clippy::wrong_self_convention)]

                use super::*;

                use crate::field::MessageFields;
                use crate::field::MessageField;

                #imports

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

fn generate_field_info_for_message(message: &DescriptorProto) -> TokenStream {
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

    let constants = generate_field_constants(message, &map_types);
    let message_fields_impl = generate_message_fields_impl(message);
    let field_path_builders = generate_field_path_builders_impl(message, &map_types);

    quote! {
        #constants
        #message_fields_impl
        #field_path_builders
    }
}

fn generate_field_constants(message: &DescriptorProto, map_types: &HashSet<String>) -> TokenStream {
    let message_ident = quote::format_ident!("{}", message.name());
    let mut field_consts = TokenStream::new();

    for field in &message.field {
        field_consts.extend(generate_field_constant(message.name(), field, map_types));
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
    message_name: &str,
    field: &FieldDescriptorProto,
    map_types: &HashSet<String>,
) -> TokenStream {
    let ident = quote::format_ident!("{}_FIELD", field.name().to_ascii_uppercase());
    let name = field.name();
    let json_name = field.json_name();
    let number = field.number();

    let message_fields =
        if matches!(field.r#type(), Type::Message) && !field.type_name().contains("google") {
            let field_message_name = field.type_name().split('.').next_back().unwrap();

            if field_message_name == message_name || map_types.contains(field_message_name) {
                quote! { None }
            } else {
                let field_message = quote::format_ident!("{}", field_message_name);
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
    message: &DescriptorProto,
    map_types: &HashSet<String>,
) -> TokenStream {
    let message_ident = quote::format_ident!("{}", message.name());
    let builder_ident = quote::format_ident!("{}FieldPathBuilder", message.name());

    let mut field_chain_methods = TokenStream::new();

    for field in &message.field {
        field_chain_methods.extend(generate_field_chain_methods(
            message.name(),
            field,
            map_types,
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

fn generate_field_chain_methods(
    message_name: &str,
    field: &FieldDescriptorProto,
    map_types: &HashSet<String>,
) -> TokenStream {
    let message_ident = quote::format_ident!("{message_name}");
    let field_const = quote::format_ident!("{}_FIELD", field.name().to_ascii_uppercase());
    let name = if field.name() == "type" {
        quote::format_ident!("r#{}", field.name())
    } else {
        quote::format_ident!("{}", field.name())
    };

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
            let return_type = quote::format_ident!("{}FieldPathBuilder", field_message_name);
            quote! {
                pub fn #name(mut self) -> #return_type {
                    self.path.push(#message_ident::#field_const.name);
                    #return_type::new_with_base(self.path)
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
