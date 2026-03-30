// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use std::{path::Path, str::FromStr};

use heck::ToPascalCase;
use proc_macro2::TokenStream;
use prost_types::field_descriptor_proto::Type;
use quote::quote;

use crate::{
    codegen::accessor_config::{AccessorMap, AccessorTypes},
    context::Context,
    message_graph::{Field, Message, OneofField},
};

pub(crate) fn generate_accessors(
    context: &Context,
    out_dir: &Path,
    boxed_types_prost: &[String],
    boxed_types_accessor: &[String],
    accessor_map: &AccessorMap,
) {
    for package in context.graph().packages.iter() {
        let mut buf = String::new();
        let mut stream = TokenStream::new();

        for message in context
            .graph()
            .messages
            .values()
            .filter(|m| &m.package == package && !context.is_extern(&m.type_name))
        {
            stream.extend(generate_accessors_for_message(
                context,
                message,
                boxed_types_prost,
                boxed_types_accessor,
                accessor_map,
            ));
        }

        // If we didn't generate anything then just skip
        if !stream.is_empty() {
            let code = quote! {
                mod _accessor_impls {
                    #![allow(clippy::useless_conversion)]

                    #stream
                }
            };

            let ast: syn::File = syn::parse2(code).expect("not a valid tokenstream");
            let code = prettyplease::unparse(&ast);

            // Add IOTA license header
            buf.push_str("// Copyright (c) Mysten Labs, Inc.\n");
            buf.push_str("// Modifications Copyright (c) 2025 IOTA Stiftung\n");
            buf.push_str("// SPDX-License-Identifier: Apache-2.0\n");
            buf.push('\n');
            buf.push_str(&code);

            let file_name = format!("{}.accessors.rs", package.trim_start_matches('.'));
            std::fs::write(out_dir.join(file_name), &buf).unwrap();
        }
    }
}

fn generate_accessors_for_message(
    context: &Context,
    message: &Message,
    boxed_types_prost: &[String],
    boxed_types_accessor: &[String],
    accessor_map: &AccessorMap,
) -> TokenStream {
    let package = format!("{}.__accessors", message.package);
    let message_rust_path =
        TokenStream::from_str(&context.resolve_ident(&package, &message.type_name)).unwrap();

    let mut functions = TokenStream::new();

    // Check if any field in this message needs the default_instance function, and
    // generate it if so. We do this at the message level (instead of per-field)
    // to avoid generating multiple default_instance functions for the same message
    // if multiple fields need it.
    let needs_default_instance = message_needs_default_instance(message, accessor_map);

    if needs_default_instance {
        functions.extend(generate_const_default_functions(
            context,
            message,
            &message_rust_path,
        ));
    }

    functions.extend(generate_accessors_functions(
        context,
        message,
        boxed_types_prost,
        boxed_types_accessor,
        accessor_map,
    ));

    // Only generate the impl block if there are any functions
    if functions.is_empty() {
        return TokenStream::new();
    }

    quote! {
        impl #message_rust_path {

            #functions
        }
    }
}

/// Check if any field in the message needs the default_instance function
/// This is needed when:
/// 1. DEFAULT is explicitly requested, OR
/// 2. GETTER is requested AND a getter method will actually be generated
fn message_needs_default_instance(message: &Message, accessor_map: &AccessorMap) -> bool {
    let message_name = message
        .type_name
        .rsplit('.')
        .next()
        .unwrap_or(&message.type_name);

    // Check regular fields
    for field in &message.fields {
        if let Some(accessor_types) =
            AccessorTypes::from_field(&field.inner, accessor_map, message_name)
        {
            // Always generate if DEFAULT is explicitly set
            if accessor_types.contains(AccessorTypes::DEFAULT) {
                return true;
            }

            // Check if GETTER is set AND will actually generate a getter method
            if accessor_types.contains(AccessorTypes::GETTER) {
                // Maps and repeated fields always generate getters
                if field.is_map() || field.is_repeated() {
                    return true;
                }
                // Optional fields only generate getters for message types (non-well-known)
                if field.is_optional() && field.is_message() && !field.is_well_known_type() {
                    return true;
                }
                // Required/implicit optional fields don't generate getters, so
                // no default needed
            }
        }
    }

    // Check oneof fields - these always generate getters if GETTER is set
    for oneof_field in &message.oneof_fields {
        for field in &oneof_field.fields {
            if let Some(accessor_types) =
                AccessorTypes::from_field(&field.inner, accessor_map, message_name)
            {
                if accessor_types.contains(AccessorTypes::DEFAULT) {
                    return true;
                }
                // Oneof fields always generate getters when GETTER is set
                if accessor_types.contains(AccessorTypes::GETTER) {
                    return true;
                }
            }
        }
    }

    false
}

fn generate_accessors_functions(
    context: &Context,
    message: &Message,
    boxed_types_prost: &[String],
    boxed_types_accessor: &[String],
    accessor_map: &AccessorMap,
) -> TokenStream {
    let mut accessors = TokenStream::new();

    for field in &message.fields {
        accessors.extend(generate_accessors_functions_for_field(
            context,
            message,
            field,
            None,
            boxed_types_prost,
            boxed_types_accessor,
            accessor_map,
        ));
    }

    for oneof_field in &message.oneof_fields {
        for field in &oneof_field.fields {
            accessors.extend(generate_accessors_functions_for_field(
                context,
                message,
                field,
                Some(oneof_field),
                boxed_types_prost,
                boxed_types_accessor,
                accessor_map,
            ));
        }
    }

    accessors
}

fn is_field_boxed_from_config(message: &Message, field: &Field, boxed_types: &[String]) -> bool {
    // Create the field path pattern and check against boxed_types config
    let field_path = format!("{}.{}", message.type_name, field.inner.name());

    boxed_types
        .iter()
        .any(|boxed_path| boxed_path == &field_path)
}

fn generate_accessors_functions_for_field(
    context: &Context,
    message: &Message,
    field: &Field,
    oneof: Option<&OneofField>,
    boxed_types_prost: &[String],
    boxed_types_accessor: &[String],
    accessor_map: &AccessorMap,
) -> TokenStream {
    // Extract the simple message name from the fully qualified type name
    // e.g., ".iota.grpc.v1.ledger_service.ObjectRequest" -> "ObjectRequest"
    let message_name = message
        .type_name
        .rsplit('.')
        .next()
        .unwrap_or(&message.type_name);

    // Check if this field has the accessors custom option
    let accessor_types = match AccessorTypes::from_field(&field.inner, accessor_map, message_name) {
        Some(types) => types,
        None => return TokenStream::new(), // No option, skip this field
    };

    // Generate only the requested accessor types
    generate_selective_accessors_for_field(
        context,
        message,
        field,
        oneof,
        boxed_types_prost,
        boxed_types_accessor,
        accessor_types,
    )
}

fn generate_selective_accessors_for_field(
    context: &Context,
    message: &Message,
    field: &Field,
    oneof: Option<&OneofField>,
    boxed_types_prost: &[String],
    boxed_types_accessor: &[String],
    accessor_types: AccessorTypes,
) -> TokenStream {
    let package = format!("{}.__accessors", message.package);
    let name = quote::format_ident!("{}", field.rust_struct_field_name());
    let name_opt = quote::format_ident!("{}_opt", field.inner.name());
    let set_name = quote::format_ident!("set_{}", field.inner.name());
    let name_mut = quote::format_ident!("{}_mut", field.inner.name());
    let name_opt_mut = quote::format_ident!("{}_opt_mut", field.inner.name());
    let with_name = quote::format_ident!("with_{}", field.inner.name());

    // doc comments

    let name_comments = vec![format!(
        " Returns the value of `{name}`, or the default value if `{name}` is unset."
    )];
    let name_opt_comments = vec![format!(
        " If `{name}` is set, returns [`Some`] with the value; otherwise returns [`None`]."
    )];
    let mut set_name_comments = vec![format!(" Sets `{name}` with the provided value.")];
    let mut name_mut_comments = vec![
        format!(" Returns a mutable reference to `{name}`."),
        " If the field is unset, it is first initialized with the default value.".to_owned(),
    ];
    let name_opt_mut_comments = vec![format!(
        " If `{name}` is set, returns [`Some`] with a mutable reference to the value; otherwise returns [`None`]."
    )];

    let is_boxed_in_accessor = is_field_boxed_from_config(message, field, boxed_types_accessor);
    let is_boxed_in_prost = is_field_boxed_from_config(message, field, boxed_types_prost);
    let base_field_type_path = field.resolve_rust_type_path(context, &package);
    let field_type_path = if is_boxed_in_accessor {
        TokenStream::from_str(&format!(
            "::prost::alloc::boxed::Box<{}>",
            base_field_type_path
        ))
        .unwrap()
    } else {
        TokenStream::from_str(&base_field_type_path).unwrap()
    };

    // Conversion logic based on boxing configuration:
    // - If both accessor and proto are boxed: field.into() (Box -> Box)
    // - If accessor is boxed but proto is not: *field.into() (Box -> T by unboxing)
    // - If accessor is not boxed: field.into() (T -> T with Into conversions)
    let into_conversion = if is_boxed_in_accessor {
        if is_boxed_in_prost {
            quote! { field.into() }
        } else {
            quote! { *field.into() }
        }
    } else {
        quote! { field.into() }
    };

    let setter_assignment_value = if use_into_for_setter(field) {
        quote! { #into_conversion }
    } else {
        quote! { field }
    };

    let set_param_type = if use_into_for_setter(field) {
        quote! { <T: Into<#field_type_path>>(&mut self, field: T) }
    } else {
        quote! { (&mut self, field: #field_type_path) }
    };

    let with_param_type = if use_into_for_setter(field) {
        quote! { <T: Into<#field_type_path>>(mut self, field: T) }
    } else {
        quote! { (mut self, field: #field_type_path) }
    };

    let default_instance = TokenStream::from_str(&type_default(field, context, &package)).unwrap();
    let ref_return_type =
        TokenStream::from_str(&ref_return_type(field, context, &package)).unwrap();
    let field_as = if is_ref_return(field) {
        quote! {field as _}
    } else {
        quote! {*field}
    };

    if let Some((key, value)) = &field.map {
        // Map Types
        let key_type =
            TokenStream::from_str(&resolve_rust_type_path(key, context, &package)).unwrap();
        let value_type =
            TokenStream::from_str(&resolve_rust_type_path(value, context, &package)).unwrap();

        let mut accessors = TokenStream::new();

        if accessor_types.contains(AccessorTypes::GETTER) {
            accessors.extend(quote! {
                #( #[doc = #name_comments] )*
                pub fn #name(&self) -> &::std::collections::BTreeMap<#key_type, #value_type> {
                    &self.#name
                }
            });
        }

        if accessor_types.contains(AccessorTypes::MUT) {
            accessors.extend(quote! {
                #( #[doc = #name_mut_comments] )*
                pub fn #name_mut(&mut self) -> &mut ::std::collections::BTreeMap<#key_type, #value_type> {
                    &mut self.#name
                }
            });
        }

        if accessor_types.contains(AccessorTypes::SET) {
            accessors.extend(quote! {
                #( #[doc = #set_name_comments] )*
                pub fn #set_name(&mut self, field: ::std::collections::BTreeMap<#key_type, #value_type>) {
                    self.#name = field;
                }
            });
        }

        if accessor_types.contains(AccessorTypes::WITH) {
            accessors.extend(quote! {
                #( #[doc = #set_name_comments] )*
                pub fn #with_name(mut self, field: ::std::collections::BTreeMap<#key_type, #value_type>) -> Self {
                    self.#name = field;
                    self
                }
            });
        }

        accessors
    } else if field.is_repeated() {
        let mut accessors = TokenStream::new();

        // For repeated enum fields, prost stores them as Vec<i32>
        let is_enum = field.is_enum();
        let storage_type = if is_enum {
            TokenStream::from_str("i32").unwrap()
        } else {
            field_type_path
        };

        if accessor_types.contains(AccessorTypes::GETTER) {
            accessors.extend(quote! {
                #( #[doc = #name_comments] )*
                pub fn #name(&self) -> &[#storage_type] {
                    &self.#name
                }
            });
        }

        if accessor_types.contains(AccessorTypes::MUT) {
            accessors.extend(quote! {
                #( #[doc = #name_mut_comments] )*
                pub fn #name_mut(&mut self) -> &mut Vec<#storage_type> {
                    &mut self.#name
                }
            });
        }

        if accessor_types.contains(AccessorTypes::SET) {
            accessors.extend(quote! {
                #( #[doc = #set_name_comments] )*
                pub fn #set_name(&mut self, field: Vec<#storage_type>) {
                    self.#name = field;
                }
            });
        }

        if accessor_types.contains(AccessorTypes::WITH) {
            accessors.extend(quote! {
                #( #[doc = #set_name_comments] )*
                pub fn #with_name(mut self, field: Vec<#storage_type>) -> Self {
                    self.#name = field;
                    self
                }
            });
        }

        accessors
    } else if let Some(oneof) = oneof {
        if field.inner.type_name() == ".google.protobuf.Empty" {
            return TokenStream::new();
        }

        let oneof_field = quote::format_ident!("{}", oneof.rust_struct_field_name());
        let oneof_type_path = TokenStream::from_str(&context.resolve_ident(
            &package,
            &format!(
                "{}.{}",
                message.type_name,
                oneof.descriptor.name().to_pascal_case()
            ),
        ))
        .unwrap();
        let variant = quote::format_ident!("{}", field.inner.name().to_pascal_case());

        name_mut_comments.push(
            " If any other oneof field in the same oneof is set, it will be cleared.".to_owned(),
        );
        set_name_comments.push(
            " If any other oneof field in the same oneof is set, it will be cleared.".to_owned(),
        );

        let mut accessors = TokenStream::new();

        if accessor_types.contains(AccessorTypes::GETTER) {
            accessors.extend(quote! {
                #( #[doc = #name_comments] )*
                pub fn #name(&self) -> #ref_return_type {
                    if let Some(#oneof_type_path::#variant(field)) = &self.#oneof_field {
                        #field_as
                    } else {
                        #default_instance
                    }
                }
            });
        }

        if accessor_types.contains(AccessorTypes::GETTER_OPT) {
            accessors.extend(quote! {
                #( #[doc = #name_opt_comments] )*
                pub fn #name_opt(&self) -> Option<#ref_return_type> {
                    if let Some(#oneof_type_path::#variant(field)) = &self.#oneof_field {
                        Some(#field_as)
                    } else {
                        None
                    }
                }
            });
        }

        if accessor_types.contains(AccessorTypes::MUT_OPT) {
            accessors.extend(quote! {
                #( #[doc = #name_opt_mut_comments] )*
                pub fn #name_opt_mut(&mut self) -> Option<&mut #field_type_path> {
                    if let Some(#oneof_type_path::#variant(field)) = &mut self.#oneof_field {
                        Some(field as _)
                    } else {
                        None
                    }
                }
            });
        }

        if accessor_types.contains(AccessorTypes::MUT) {
            let (field_access, default_value) = if is_boxed_in_accessor {
                (
                    quote! { field as _ },
                    quote! { ::prost::alloc::boxed::Box::default() },
                )
            } else {
                (quote! { field }, quote! { #field_type_path::default() })
            };

            accessors.extend(quote! {
                #( #[doc = #name_mut_comments] )*
                pub fn #name_mut(&mut self) -> &mut #field_type_path {
                    if let Some(#oneof_type_path::#variant(field)) = &mut self.#oneof_field {
                        #field_access
                    } else {
                        self.#oneof_field = Some(#oneof_type_path::#variant(#default_value));
                        if let Some(#oneof_type_path::#variant(field)) = &mut self.#oneof_field {
                            #field_access
                        } else {
                            unreachable!()
                        }
                    }
                }
            });
        }

        if accessor_types.contains(AccessorTypes::SET) {
            accessors.extend(quote! {
                #( #[doc = #set_name_comments] )*
                pub fn #set_name #set_param_type {
                    self.#oneof_field = Some(#oneof_type_path::#variant(#setter_assignment_value));
                }
            });
        }

        if accessor_types.contains(AccessorTypes::WITH) {
            accessors.extend(quote! {
                #( #[doc = #set_name_comments] )*
                pub fn #with_name #with_param_type -> Self {
                    self.#oneof_field = Some(#oneof_type_path::#variant(#setter_assignment_value));
                    self
                }
            });
        }

        accessors
    } else if field.is_optional() {
        let mut accessors = TokenStream::new();

        // only include "bare getter" for message types
        if accessor_types.contains(AccessorTypes::GETTER)
            && field.is_message()
            && !field.is_well_known_type()
        {
            accessors.extend(quote! {
                #( #[doc = #name_comments] )*
                pub fn #name(&self) -> #ref_return_type {
                    self.#name
                        .as_ref()
                        .map(|field| field as _)
                        .unwrap_or_else(|| #default_instance)
                }
            });
        }

        // Only include mut getters for non bytes/enum types
        if !matches!(field.inner.r#type(), Type::Bytes | Type::Enum) {
            if accessor_types.contains(AccessorTypes::MUT_OPT) {
                accessors.extend(quote! {
                    #( #[doc = #name_opt_mut_comments] )*
                    pub fn #name_opt_mut(&mut self) -> Option<&mut #field_type_path> {
                        self.#name
                            .as_mut()
                            .map(|field| field as _)
                    }
                });
            }

            if accessor_types.contains(AccessorTypes::MUT) {
                accessors.extend(quote! {
                    #( #[doc = #name_mut_comments] )*
                    pub fn #name_mut(&mut self) -> &mut #field_type_path {
                        self.#name
                            .get_or_insert_default()
                    }
                });
            }
        }

        // only include _opt and set for non enums (as this already exists for enums
        // from prost)
        if !matches!(field.inner.r#type(), Type::Enum)
            && accessor_types.contains(AccessorTypes::GETTER_OPT)
        {
            accessors.extend(quote! {
                #( #[doc = #name_opt_comments] )*
                pub fn #name_opt(&self) -> Option<#ref_return_type> {
                    self.#name
                        .as_ref()
                        .map(|field| #field_as)
                }
            });
        }

        if !matches!(field.inner.r#type(), Type::Enum)
            && accessor_types.contains(AccessorTypes::SET)
        {
            accessors.extend(quote! {
                #( #[doc = #set_name_comments] )*
                pub fn #set_name #set_param_type {
                    self.#name = Some(#setter_assignment_value);
                }
            });
        }

        if accessor_types.contains(AccessorTypes::WITH) {
            // For optional enum fields, prost stores as Option<i32>.
            // Take the enum type and convert via .into().
            if field.is_enum() {
                accessors.extend(quote! {
                    #( #[doc = #set_name_comments] )*
                    pub fn #with_name(mut self, field: #field_type_path) -> Self {
                        self.#name = Some(field.into());
                        self
                    }
                });
            } else {
                accessors.extend(quote! {
                    #( #[doc = #set_name_comments] )*
                    pub fn #with_name #with_param_type -> Self {
                        self.#name = Some(#setter_assignment_value);
                        self
                    }
                });
            }
        }

        accessors
    } else {
        // maybe required or implicit optional

        let mut accessors = TokenStream::new();

        // For enum fields, prost stores the value as i32 but
        // field_type_path resolves to the enum type. We need special
        // handling: take the enum type as parameter and convert via
        // .into() for assignment.
        if field.is_enum() {
            if accessor_types.contains(AccessorTypes::SET) {
                accessors.extend(quote! {
                    #( #[doc = #set_name_comments] )*
                    pub fn #set_name(&mut self, field: #field_type_path) {
                        self.#name = field.into();
                    }
                });
            }

            if accessor_types.contains(AccessorTypes::WITH) {
                accessors.extend(quote! {
                    #( #[doc = #set_name_comments] )*
                    pub fn #with_name(mut self, field: #field_type_path) -> Self {
                        self.#name = field.into();
                        self
                    }
                });
            }

            return accessors;
        }

        if field.inner.r#type() != Type::Bytes && accessor_types.contains(AccessorTypes::MUT) {
            accessors.extend(quote! {
            #( #[doc = #name_mut_comments] )*
                pub fn #name_mut(&mut self) -> &mut #field_type_path {
                    &mut self.#name
                }
            });
        }

        if accessor_types.contains(AccessorTypes::SET) {
            accessors.extend(quote! {
                #( #[doc = #set_name_comments] )*
                pub fn #set_name #set_param_type {
                    self.#name = #setter_assignment_value;
                }
            });
        }

        if accessor_types.contains(AccessorTypes::WITH) {
            accessors.extend(quote! {
                #( #[doc = #set_name_comments] )*
                pub fn #with_name #with_param_type -> Self {
                    self.#name = #setter_assignment_value;
                    self
                }
            });
        }

        accessors
    }
}

fn generate_const_default_functions(
    _context: &Context,
    message: &Message,
    message_rust_path: &TokenStream,
) -> TokenStream {
    let mut const_default_fields = TokenStream::new();

    for field in &message.fields {
        let field_name = quote::format_ident!("{}", field.rust_struct_field_name());

        let field_default = if field.is_map() {
            quote! {
                #field_name: std::collections::BTreeMap::new(),
            }
        } else if field.is_repeated() {
            quote! {
                #field_name: Vec::new(),
            }
        } else if field.is_optional() {
            quote! {
                #field_name: None,
            }
        } else {
            // maybe required or implicit optional
            match field.inner.r#type() {
                Type::Double
                | Type::Float
                | Type::Int64
                | Type::Uint64
                | Type::Int32
                | Type::Fixed64
                | Type::Fixed32
                | Type::Uint32
                | Type::Enum
                | Type::Sfixed32
                | Type::Sfixed64
                | Type::Sint32
                | Type::Sint64 => {
                    quote! {
                        #field_name: 0,
                    }
                }

                Type::Bool => {
                    quote! {
                        #field_name: false,
                    }
                }
                Type::String => {
                    quote! {
                        #field_name: String::new(),
                    }
                }
                Type::Bytes => {
                    quote! {
                        #field_name: ::prost::bytes::Bytes::new(),
                    }
                }
                Type::Group | Type::Message => {
                    panic!("messages are optional");
                }
            }
        };

        const_default_fields.extend(field_default);
    }

    for oneof in &message.oneof_fields {
        let oneof_field = quote::format_ident!("{}", oneof.rust_struct_field_name());
        const_default_fields.extend(quote! {
            #oneof_field: None,
        });
    }

    quote! {
        pub const fn const_default() -> Self {
            Self {
                #const_default_fields
            }
        }

        #[doc(hidden)]
        pub fn default_instance() -> &'static Self {
            static DEFAULT: #message_rust_path = #message_rust_path::const_default();
            &DEFAULT
        }
    }
}

fn type_default(field: &Field, context: &Context, package: &str) -> String {
    match field.inner.r#type() {
        Type::Float => String::from("0.0f32"),
        Type::Double => String::from("0.0f64"),
        Type::Uint32 | Type::Fixed32 => String::from("0u32"),
        Type::Uint64 | Type::Fixed64 => String::from("0u64"),
        Type::Int32 | Type::Sfixed32 | Type::Sint32 | Type::Enum => String::from("0i32"),
        Type::Int64 | Type::Sfixed64 | Type::Sint64 => String::from("0i64"),
        Type::Bool => String::from("false"),
        Type::String => String::from("\"\""),
        Type::Bytes => String::from("&[]"),
        Type::Group | Type::Message => {
            let ty = context.resolve_ident(package, field.inner.type_name());
            format!("{}::default_instance() as _", ty)
        }
    }
}

fn ref_return_type(field: &Field, context: &Context, package: &str) -> String {
    match field.inner.r#type() {
        Type::Float => String::from("f32"),
        Type::Double => String::from("f64"),
        Type::Uint32 | Type::Fixed32 => String::from("u32"),
        Type::Uint64 | Type::Fixed64 => String::from("u64"),
        Type::Int32 | Type::Sfixed32 | Type::Sint32 | Type::Enum => String::from("i32"),
        Type::Int64 | Type::Sfixed64 | Type::Sint64 => String::from("i64"),
        Type::Bool => String::from("bool"),
        Type::String => String::from("&str"),
        Type::Bytes => String::from("&[u8]"),
        Type::Group | Type::Message => {
            let ty = context.resolve_ident(package, field.inner.type_name());
            format!("&{}", ty)
        }
    }
}

fn is_ref_return(field: &Field) -> bool {
    match field.inner.r#type() {
        Type::Float => false,
        Type::Double => false,
        Type::Uint32 | Type::Fixed32 => false,
        Type::Uint64 | Type::Fixed64 => false,
        Type::Int32 | Type::Sfixed32 | Type::Sint32 | Type::Enum => false,
        Type::Int64 | Type::Sfixed64 | Type::Sint64 => false,
        Type::Bool => false,
        Type::String => true,
        Type::Bytes => true,
        Type::Group | Type::Message => true,
    }
}

fn use_into_for_setter(field: &Field) -> bool {
    match field.inner.r#type() {
        Type::Float => false,
        Type::Double => false,
        Type::Uint32 | Type::Fixed32 => false,
        Type::Uint64 | Type::Fixed64 => false,
        Type::Int32 | Type::Sfixed32 | Type::Sint32 => false,
        Type::Int64 | Type::Sfixed64 | Type::Sint64 => false,
        Type::Bool => false,
        Type::Enum => true,
        Type::String => true,
        Type::Bytes => true,
        Type::Group | Type::Message => true,
    }
}

pub fn resolve_rust_type_path(
    field: &prost_types::FieldDescriptorProto,
    context: &crate::context::Context,
    package: &str,
) -> String {
    match field.r#type() {
        Type::Float => String::from("f32"),
        Type::Double => String::from("f64"),
        Type::Uint32 | Type::Fixed32 => String::from("u32"),
        Type::Uint64 | Type::Fixed64 => String::from("u64"),
        Type::Int32 | Type::Sfixed32 | Type::Sint32 | Type::Enum => String::from("i32"),
        Type::Int64 | Type::Sfixed64 | Type::Sint64 => String::from("i64"),
        Type::Bool => String::from("bool"),
        Type::String => String::from("String"),
        Type::Bytes => String::from("::prost::bytes::Bytes"),
        Type::Group | Type::Message => context.resolve_ident(package, field.type_name()),
    }
}
