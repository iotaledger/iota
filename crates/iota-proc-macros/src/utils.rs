// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ImplItemFn, ItemFn, LitStr, ReturnType, TraitItemFn};

// ============================================================================
// Non-Fallible Method Generation Framework
// ============================================================================

/// Parse the expect message from macro attributes
pub fn parse_expect_message(attr: TokenStream, default_msg: &str) -> String {
    if attr.is_empty() {
        default_msg.to_string()
    } else {
        match syn::parse::<LitStr>(attr) {
            Ok(lit_str) => lit_str.value(),
            Err(_) => default_msg.to_string(),
        }
    }
}

/// Helper function to check if a type name is a Result-like type
fn is_result_type(type_name: &str) -> bool {
    type_name == "Result" || type_name == "IotaResult"
}

/// Helper function to check if a type name is a Future-like type
fn is_future_type(type_name: &str) -> bool {
    type_name == "Future" || type_name == "BoxFuture"
}

/// Helper function to extract the last segment name from a type path
fn get_type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|seg| seg.ident.to_string()),
        _ => None,
    }
}

/// Extract inner type from Result<T, E> or IotaResult<T, E>
fn extract_inner_from_result_type(ty: &syn::Type) -> Option<proc_macro2::TokenStream> {
    if let syn::Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        let segment_name = segment.ident.to_string();

        if is_result_type(&segment_name) {
            return extract_result_inner_type(&segment.arguments, &segment_name);
        }
    }
    None
}

/// Extract the inner type from Result/IotaResult arguments
fn extract_result_inner_type(
    args: &syn::PathArguments,
    type_name: &str,
) -> Option<proc_macro2::TokenStream> {
    match args {
        syn::PathArguments::AngleBracketed(args) => {
            if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                // Check if the inner type is () - if so, return ()
                if let syn::Type::Tuple(tuple) = inner {
                    if tuple.elems.is_empty() {
                        return Some(quote! { () });
                    }
                }
                Some(quote! { #inner })
            } else {
                // Handle IotaResult without explicit type arguments (defaults to ())
                Some(quote! { () })
            }
        }
        _ => {
            // Handle IotaResult without any angle brackets (defaults to ())
            if type_name == "IotaResult" {
                Some(quote! { () })
            } else {
                None
            }
        }
    }
}

/// Extract type from Future arguments
fn extract_future_inner_type(segment: &syn::PathSegment) -> Option<proc_macro2::TokenStream> {
    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
        // For BoxFuture<'_, T>, the second argument is the type
        if segment.ident == "BoxFuture" {
            if let Some(syn::GenericArgument::Type(inner)) = args.args.iter().nth(1) {
                return extract_inner_from_result_type(inner).or_else(|| Some(quote! { #inner }));
            }
        }

        // For Future<Output = T>, look for the Output associated type
        for arg in &args.args {
            if let syn::GenericArgument::AssocType(assoc_type) = arg {
                if assoc_type.ident == "Output" {
                    return extract_inner_from_result_type(&assoc_type.ty);
                }
            }
        }
    }
    None
}

/// Extract inner type from impl Future bounds
fn extract_impl_future_inner_type(
    impl_trait: &syn::TypeImplTrait,
) -> Option<proc_macro2::TokenStream> {
    for bound in &impl_trait.bounds {
        if let syn::TypeParamBound::Trait(trait_bound) = bound {
            let path = &trait_bound.path;
            if let Some(segment) = path.segments.last() {
                if is_future_type(&segment.ident.to_string()) {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        for arg in &args.args {
                            if let syn::GenericArgument::AssocType(assoc_type) = arg {
                                if assoc_type.ident == "Output" {
                                    return extract_inner_from_result_type(&assoc_type.ty);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extract inner type from Result<T, E>, IotaResult<T, E>, Future<Output =
/// Result<T, E>>, etc.
fn extract_inner_type_from_result_or_future(ty: &syn::Type) -> Option<proc_macro2::TokenStream> {
    match ty {
        syn::Type::Path(type_path) => {
            let segment = type_path.path.segments.last()?;
            let segment_name = segment.ident.to_string();

            // Handle direct Result<T, E> or IotaResult<T, E>
            if is_result_type(&segment_name) {
                return extract_result_inner_type(&segment.arguments, &segment_name);
            }

            // Handle Future<Output = Result<T, E>>, BoxFuture<'_, Result<T, E>>
            if is_future_type(&segment_name) {
                return extract_future_inner_type(segment);
            }

            None
        }
        syn::Type::ImplTrait(impl_trait) => {
            // Handle impl Future<Output = Result<T, E>>
            extract_impl_future_inner_type(impl_trait)
        }
        _ => None,
    }
}

/// Determine if a return type represents a Future
fn is_future_return_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|seg| is_future_type(&seg.ident.to_string()))
            .unwrap_or(false),
        syn::Type::ImplTrait(impl_trait) => impl_trait.bounds.iter().any(|bound| {
            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                trait_bound
                    .path
                    .segments
                    .last()
                    .map(|seg| is_future_type(&seg.ident.to_string()))
                    .unwrap_or(false)
            } else {
                false
            }
        }),
        _ => false,
    }
}

/// Determine if a return type is specifically BoxFuture (not impl Future)
fn is_boxfuture_return_type(ty: &syn::Type) -> bool {
    get_type_name(ty)
        .map(|name| name == "BoxFuture")
        .unwrap_or(false)
}

/// Check if a function signature has a self parameter
fn has_self_parameter(inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>) -> bool {
    inputs
        .iter()
        .any(|arg| matches!(arg, syn::FnArg::Receiver(_)))
}

/// Generate argument identifiers for method calls (handles wildcard patterns).
/// This converts patterns like `_` to usable identifiers in function calls
pub fn generate_arg_identifiers(
    inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
) -> Vec<syn::Ident> {
    let mut arg_counter = 0usize;
    inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_ty) => {
                Some(match &*pat_ty.pat {
                    syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                    syn::Pat::Wild(_) | _ => {
                        // Generate a unique parameter name for wildcard or complex patterns
                        let ident = format_ident!("__arg_{}", arg_counter);
                        arg_counter += 1;
                        ident
                    }
                })
            }
        })
        .collect()
}

/// Generate proper function parameters (converts wildcard patterns to named
/// parameters)
fn generate_function_inputs(
    inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
) -> syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma> {
    let mut arg_counter = 0usize;
    let mut new_inputs = syn::punctuated::Punctuated::new();

    for arg in inputs {
        match arg {
            syn::FnArg::Receiver(receiver) => {
                new_inputs.push(syn::FnArg::Receiver(receiver.clone()));
            }
            syn::FnArg::Typed(pat_ty) => {
                let new_pat = match &*pat_ty.pat {
                    syn::Pat::Ident(_) => pat_ty.pat.clone(),
                    syn::Pat::Wild(_) | _ => {
                        // Replace wildcard/complex patterns with named parameters
                        let ident = format_ident!("__arg_{}", arg_counter);
                        arg_counter += 1;
                        Box::new(syn::Pat::Ident(syn::PatIdent {
                            attrs: vec![],
                            by_ref: None,
                            mutability: None,
                            ident,
                            subpat: None,
                        }))
                    }
                };

                new_inputs.push(syn::FnArg::Typed(syn::PatType {
                    attrs: pat_ty.attrs.clone(),
                    pat: new_pat,
                    colon_token: pat_ty.colon_token,
                    ty: pat_ty.ty.clone(),
                }));
            }
        }
    }

    new_inputs
}

/// Generate the return type for a non-fallible method
fn generate_return_type(method_info: &TryMethodInfo) -> proc_macro2::TokenStream {
    if method_info.is_boxfuture {
        // For BoxFuture returns, preserve the BoxFuture structure with proper lifetimes
        let inner_type = &method_info.inner_return_type;
        if method_info.generics.lt_token.is_some() {
            quote! { BoxFuture<'a, #inner_type> }
        } else {
            quote! { BoxFuture<'_, #inner_type> }
        }
    } else if method_info.is_future {
        // For impl Future returns, preserve the impl Future structure
        let inner_type = &method_info.inner_return_type;
        quote! { impl Future<Output = #inner_type> }
    } else {
        method_info.inner_return_type.clone()
    }
}

/// Generate a method call for BoxFuture returns in trait contexts
pub fn generate_boxfuture_trait_call(
    method_info: &TryMethodInfo,
    expect_msg: &str,
) -> proc_macro2::TokenStream {
    let original_name = &method_info.original_name;
    let arg_identifiers = generate_arg_identifiers(&method_info.inputs);

    quote! {
        Box::pin(async move {
            self.#original_name(#(#arg_identifiers),*).await.expect(#expect_msg)
        })
    }
}

/// Generate a method call for impl Future returns in trait contexts
pub fn generate_impl_future_trait_call(
    method_info: &TryMethodInfo,
    expect_msg: &str,
) -> proc_macro2::TokenStream {
    let original_name = &method_info.original_name;
    let arg_identifiers = generate_arg_identifiers(&method_info.inputs);

    quote! {
        async move {
            self.#original_name(#(#arg_identifiers),*).await.expect(#expect_msg)
        }
    }
}

/// Generate a method call for BoxFuture returns in impl contexts
pub fn generate_boxfuture_impl_call(
    method_info: &TryMethodInfo,
    expect_msg: &str,
    trait_path: Option<&syn::Path>,
) -> proc_macro2::TokenStream {
    let original_name = &method_info.original_name;
    let arg_identifiers = generate_arg_identifiers(&method_info.inputs);

    let call_with_await_expect = if method_info.has_self {
        if let Some(trait_path) = trait_path {
            quote! {
                <Self as #trait_path>::#original_name(self, #(#arg_identifiers),*).await.expect(#expect_msg)
            }
        } else {
            quote! {
                self.#original_name(#(#arg_identifiers),*).await.expect(#expect_msg)
            }
        }
    } else {
        quote! {
            #original_name(#(#arg_identifiers),*).await.expect(#expect_msg)
        }
    };

    quote! {
        Box::pin(async move {
            #call_with_await_expect
        })
    }
}

/// Generate a complete method signature (trait or impl)
pub fn generate_method_signature(
    method_info: &TryMethodInfo,
    call_body: Option<proc_macro2::TokenStream>,
    is_trait_method: bool,
    is_async: bool,
) -> proc_macro2::TokenStream {
    let new_name = &method_info.new_name;
    let new_inputs = generate_function_inputs(&method_info.inputs);
    let return_type = generate_return_type(method_info);
    let generics = &method_info.generics;
    let where_clause = &method_info.generics.where_clause;

    let async_keyword = if is_async {
        quote! { async }
    } else {
        quote! {}
    };

    if is_trait_method {
        // Trait methods don't have explicit visibility
        match call_body {
            Some(body) => {
                // Method with default implementation
                quote! {
                    #async_keyword fn #new_name #generics(#new_inputs) -> #return_type #where_clause {
                        #body
                    }
                }
            }
            None => {
                // Just a method signature without body
                quote! {
                    #async_keyword fn #new_name #generics(#new_inputs) -> #return_type #where_clause;
                }
            }
        }
    } else {
        // Impl methods use the visibility from the method info and always have a body
        let vis = &method_info.visibility;
        let body = call_body.expect("Impl methods must have a body");
        quote! {
            #vis #async_keyword fn #new_name #generics(#new_inputs) -> #return_type #where_clause {
                #body
            }
        }
    }
}

/// Generate a non-fallible method call with proper trait qualification
pub fn generate_non_fallible_call(
    method_name: &syn::Ident,
    arg_identifiers: &[syn::Ident],
    expect_msg: &str,
    has_self: bool,
    is_future: bool,
    trait_path: Option<&syn::Path>,
    self_type: Option<&syn::Type>,
) -> proc_macro2::TokenStream {
    if has_self {
        // Instance method - use explicit trait qualification when available
        let method_call = if let Some(trait_path) = trait_path {
            if let Some(self_type) = self_type {
                // Use the concrete type if available (for macro contexts)
                quote! { <#self_type as #trait_path>::#method_name(self, #(#arg_identifiers),*) }
            } else {
                // Fallback to Self for normal contexts
                quote! { <Self as #trait_path>::#method_name(self, #(#arg_identifiers),*) }
            }
        } else {
            // Fallback to self call if no trait path is provided
            quote! { self.#method_name(#(#arg_identifiers),*) }
        };

        if is_future {
            quote! {
                #method_call.await.expect(#expect_msg)
            }
        } else {
            quote! {
                #method_call.expect(#expect_msg)
            }
        }
    } else {
        // Static function - call without self (using Self::)
        if is_future {
            quote! {
                Self::#method_name(#(#arg_identifiers),*).await.expect(#expect_msg)
            }
        } else {
            quote! {
                Self::#method_name(#(#arg_identifiers),*).expect(#expect_msg)
            }
        }
    }
}

/// Information about a try_ method for non-fallible method generation
#[derive(Debug, Clone)]
pub struct TryMethodInfo {
    pub original_name: syn::Ident,
    pub new_name: syn::Ident,
    pub inputs: syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
    pub inner_return_type: proc_macro2::TokenStream,
    pub is_future: bool,
    pub is_boxfuture: bool,
    pub has_self: bool,
    pub generics: syn::Generics,
    pub visibility: syn::Visibility,
}

/// Analyze a try_ method from a function item
pub fn analyze_try_method_from_fn_item(fn_item: &ItemFn) -> Option<TryMethodInfo> {
    analyze_try_method_with_visibility(&fn_item.sig, fn_item.vis.clone())
}

/// Analyze a try_ method from a trait item
pub fn analyze_try_method_from_trait_item(trait_item: &TraitItemFn) -> Option<TryMethodInfo> {
    // Trait methods don't have explicit visibility (they inherit from the trait)
    analyze_try_method_with_visibility(&trait_item.sig, syn::Visibility::Inherited)
}

/// Analyze a try_ method from an impl item
pub fn analyze_try_method_from_impl_item(impl_item: &ImplItemFn) -> Option<TryMethodInfo> {
    analyze_try_method_with_visibility(&impl_item.sig, impl_item.vis.clone())
}

/// Analyze a try_ method signature with visibility and extract relevant
/// information
fn analyze_try_method_with_visibility(
    sig: &syn::Signature,
    visibility: syn::Visibility,
) -> Option<TryMethodInfo> {
    let method_name = &sig.ident;
    let method_name_str = method_name.to_string();

    // Check if method name starts with "try_"
    if !method_name_str.starts_with("try_") {
        return None;
    }

    let new_method_name = format_ident!("{}", method_name_str.trim_start_matches("try_"));

    // Extract return type
    let output = match &sig.output {
        ReturnType::Type(_, ty) => ty,
        ReturnType::Default => return None,
    };

    // Extract inner return type from Result<...> or Future<...>
    let inner_ty = extract_inner_type_from_result_or_future(output)?;
    let is_future = is_future_return_type(output);
    let is_boxfuture = is_boxfuture_return_type(output);
    let has_self = has_self_parameter(&sig.inputs);

    Some(TryMethodInfo {
        original_name: method_name.clone(),
        new_name: new_method_name,
        inputs: sig.inputs.clone(),
        inner_return_type: inner_ty,
        is_future,
        is_boxfuture,
        has_self,
        generics: sig.generics.clone(),
        visibility,
    })
}
