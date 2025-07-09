// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote, quote_spanned};
use syn::{
    Attribute, BinOp, Data, DataEnum, DeriveInput, Expr, ExprBinary, ExprMacro, ImplItem,
    ImplItemFn, Item, ItemImpl, ItemMacro, ItemTrait, LitStr, ReturnType, Stmt, StmtMacro, Token,
    TraitItem, TraitItemFn, UnOp,
    fold::{Fold, fold_expr, fold_item_macro, fold_stmt},
    parse::Parser,
    parse_macro_input, parse2,
    punctuated::Punctuated,
    spanned::Spanned,
};

#[proc_macro_attribute]
pub fn init_static_initializers(_args: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as syn::ItemFn);

    let body = &input.block;
    input.block = syn::parse2(quote! {
        {
            // We have some lazily-initialized static state in the program. The initializers
            // alter the thread-local hash container state any time they create a new hash
            // container. Therefore, we need to ensure that these initializers are run in a
            // separate thread before the first test thread is launched. Otherwise, they would
            // run inside of the first test thread, but not subsequent ones.
            //
            // Note that none of this has any effect on process-level determinism. Without this
            // code, we can still get the same test results from two processes started with the
            // same seed.
            //
            // However, when using sim_test(check_determinism) or MSIM_TEST_CHECK_DETERMINISM=1,
            // we want the same test invocation to be deterministic when run twice
            // _in the same process_, so we need to take care of this. This will also
            // be very important for being able to reproduce a failure that occurs in the Nth
            // iteration of a multi-iteration test run.
            std::thread::spawn(|| {
                use iota_protocol_config::ProtocolConfig;
                ::iota_simulator::telemetry_subscribers::init_for_testing();
                ::iota_simulator::iota_types::execution::get_denied_certificates();
                ::iota_simulator::iota_framework::BuiltInFramework::all_package_ids();
                ::iota_simulator::iota_types::gas::IotaGasStatus::new_unmetered();

                // For reasons I can't understand, LruCache causes divergent behavior the second
                // time one is constructed and inserted into, so construct one before the first
                // test run for determinism.
                let mut cache = ::iota_simulator::lru::LruCache::new(1.try_into().unwrap());
                cache.put(1, 1);

                {
                    // Initialize the static initializers here:
                    // https://github.com/move-language/move/blob/652badf6fd67e1d4cc2aa6dc69d63ad14083b673/language/tools/move-package/src/package_lock.rs#L12
                    use std::path::PathBuf;
                    use iota_simulator::iota_move_build::{BuildConfig, IotaPackageHooks};
                    use iota_simulator::tempfile::TempDir;
                    use iota_simulator::move_package::package_hooks::register_package_hooks;

                    register_package_hooks(Box::new(IotaPackageHooks {}));
                    let mut path = PathBuf::from(env!("SIMTEST_STATIC_INIT_MOVE"));
                    let mut build_config = BuildConfig::default();

                    build_config.config.install_dir = Some(TempDir::new().unwrap().into_path());
                    let _all_module_bytes = build_config
                        .build(&path)
                        .unwrap()
                        .get_package_bytes(/* with_unpublished_deps */ false);
                }


                use ::iota_simulator::anemo_tower::callback::CallbackLayer;
                use ::iota_simulator::anemo_tower::trace::DefaultMakeSpan;
                use ::iota_simulator::anemo_tower::trace::DefaultOnFailure;
                use ::iota_simulator::anemo_tower::trace::TraceLayer;
                use ::iota_metrics::metrics_network::{NetworkMetrics, MetricsMakeCallbackHandler};

                use std::sync::Arc;
                use ::iota_simulator::fastcrypto::traits::KeyPair;
                use ::iota_simulator::rand_crate::rngs::{StdRng, OsRng};
                use ::iota_simulator::rand::SeedableRng;
                use ::iota_simulator::tower::ServiceBuilder;

                // anemo uses x509-parser, which has many lazy static variables. start a network to
                // initialize all that static state before the first test.
                let rt = ::iota_simulator::runtime::Runtime::new();
                rt.block_on(async move {
                    use ::iota_simulator::anemo::{Network, Request};

                    let make_network = |port: u16| {
                        let registry = prometheus::Registry::new();
                        let inbound_network_metrics =
                            NetworkMetrics::new("iota", "inbound", &registry);
                        let outbound_network_metrics =
                            NetworkMetrics::new("iota", "outbound", &registry);

                        let service = ServiceBuilder::new()
                            .layer(
                                TraceLayer::new_for_server_errors()
                                    .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                                    .on_failure(DefaultOnFailure::new().level(tracing::Level::WARN)),
                            )
                            .layer(CallbackLayer::new(MetricsMakeCallbackHandler::new(
                                Arc::new(inbound_network_metrics),
                                usize::MAX,
                            )))
                            .service(::iota_simulator::anemo::Router::new());

                        let outbound_layer = ServiceBuilder::new()
                            .layer(
                                TraceLayer::new_for_client_and_server_errors()
                                    .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                                    .on_failure(DefaultOnFailure::new().level(tracing::Level::WARN)),
                            )
                            .layer(CallbackLayer::new(MetricsMakeCallbackHandler::new(
                                Arc::new(outbound_network_metrics),
                                usize::MAX,
                            )))
                            .into_inner();


                        Network::bind(format!("127.0.0.1:{}", port))
                            .server_name("static-init-network")
                            .private_key(
                                ::iota_simulator::fastcrypto::ed25519::Ed25519KeyPair::generate(&mut StdRng::from_rng(OsRng).unwrap())
                                    .private()
                                    .0
                                    .to_bytes(),
                            )
                            .start(service)
                            .unwrap()
                    };
                    let n1 = make_network(80);
                    let n2 = make_network(81);

                    let _peer = n1.connect(n2.local_addr()).await.unwrap();
                });
            }).join().unwrap();

            #body
        }
    })
    .expect("Parsing failure");

    let result = quote! {
        #input
    };

    result.into()
}

/// The iota_test macro will invoke either `#[msim::test]` or `#[tokio::test]`,
/// depending on whether the simulator config var is enabled.
///
/// This should be used for tests that can meaningfully run in either
/// environment.
#[proc_macro_attribute]
pub fn iota_test(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    let arg_parser = Punctuated::<syn::Meta, Token![,]>::parse_terminated;
    let args = arg_parser.parse(args).unwrap().into_iter();

    let header = if cfg!(msim) {
        quote! {
            #[::iota_simulator::sim_test(crate = "iota_simulator", #(#args)* )]
        }
    } else {
        quote! {
            #[::tokio::test(#(#args)*)]
        }
    };

    let result = quote! {
        #header
        #[::iota_macros::init_static_initializers]
        #input
    };

    result.into()
}

/// The `sim_test` macro will invoke `#[msim::test]` if the simulator config var
/// (`msim`) is enabled.
///
/// On this premise, this macro can be used in order to pass any
/// simulator-specific arguments, such as `check_determinism`,
/// which is not understood by tokio.
///
/// If the simulator config var is disabled, tests will run via
/// `#[tokio::test]`, unless disabled by setting the environment variable
/// `IOTA_SKIP_SIMTESTS`.
#[proc_macro_attribute]
pub fn sim_test(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemFn);
    let arg_parser = Punctuated::<syn::Meta, Token![,]>::parse_terminated;
    let args = arg_parser.parse(args).unwrap().into_iter();

    let ignore = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("ignore"))
        .map_or(quote! {}, |_| quote! { #[ignore] });

    let result = if cfg!(msim) {
        let sig = &input.sig;
        let return_type = &sig.output;
        let body = &input.block;
        quote! {
            #[::iota_simulator::sim_test(crate = "iota_simulator", #(#args),*)]
            #[::iota_macros::init_static_initializers]
            #ignore
            #sig {
                async fn body_fn() #return_type { #body }

                let ret = body_fn().await;

                ::iota_simulator::task::shutdown_all_nodes();

                // all node handles should have been dropped after the above block exits, but task
                // shutdown is asynchronous, so we need a brief delay before checking for leaks.
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

                assert_eq!(
                    iota_simulator::NodeLeakDetector::get_current_node_count(),
                    0,
                    "IotaNode leak detected"
                );

                ret
            }
        }
    } else {
        let fn_name = &input.sig.ident;
        let sig = &input.sig;
        let body = &input.block;
        quote! {
            #[expect(clippy::needless_return)]
            #[tokio::test]
            #ignore
            #sig {
                if std::env::var("IOTA_SKIP_SIMTESTS").is_ok() {
                    println!("not running test {} in `cargo test`: IOTA_SKIP_SIMTESTS is set", stringify!(#fn_name));

                    struct Ret;

                    impl From<Ret> for () {
                        fn from(_ret: Ret) -> Self {
                        }
                    }

                    impl<E> From<Ret> for Result<(), E> {
                        fn from(_ret: Ret) -> Self {
                            Ok(())
                        }
                    }

                    return Ret.into();
                }

                #body
            }
        }
    };

    result.into()
}

#[proc_macro]
pub fn checked_arithmetic(input: TokenStream) -> TokenStream {
    let input_file = CheckArithmetic.fold_file(parse_macro_input!(input));

    let output_items = input_file.items;

    let output = quote! {
        #(#output_items)*
    };

    TokenStream::from(output)
}

#[proc_macro_attribute]
pub fn with_checked_arithmetic(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_item = parse_macro_input!(item as Item);
    match input_item {
        Item::Fn(input_fn) => {
            let transformed_fn = CheckArithmetic.fold_item_fn(input_fn);
            TokenStream::from(quote! { #transformed_fn })
        }
        Item::Impl(input_impl) => {
            let transformed_impl = CheckArithmetic.fold_item_impl(input_impl);
            TokenStream::from(quote! { #transformed_impl })
        }
        item => {
            let transformed_impl = CheckArithmetic.fold_item(item);
            TokenStream::from(quote! { #transformed_impl })
        }
    }
}

struct CheckArithmetic;

impl CheckArithmetic {
    fn maybe_skip_macro(&self, attrs: &mut Vec<Attribute>) -> bool {
        if let Some(idx) = attrs
            .iter()
            .position(|attr| attr.path().is_ident("skip_checked_arithmetic"))
        {
            // Skip processing macro because it is annotated with
            // #[skip_checked_arithmetic]
            attrs.remove(idx);
            true
        } else {
            false
        }
    }

    fn process_macro_contents(
        &mut self,
        tokens: proc_macro2::TokenStream,
    ) -> syn::Result<proc_macro2::TokenStream> {
        // Parse the macro's contents as a comma-separated list of expressions.
        let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
        let Ok(exprs) = parser.parse(tokens.clone().into()) else {
            return Err(syn::Error::new_spanned(
                tokens,
                "could not process macro contents - use #[skip_checked_arithmetic] to skip this macro",
            ));
        };

        // Fold each sub expression.
        let folded_exprs = exprs
            .into_iter()
            .map(|expr| self.fold_expr(expr))
            .collect::<Vec<_>>();

        // Convert the folded expressions back into tokens and reconstruct the macro.
        let mut folded_tokens = proc_macro2::TokenStream::new();
        for (i, folded_expr) in folded_exprs.into_iter().enumerate() {
            if i > 0 {
                folded_tokens.extend(std::iter::once::<proc_macro2::TokenTree>(
                    proc_macro2::Punct::new(',', proc_macro2::Spacing::Alone).into(),
                ));
            }
            folded_expr.to_tokens(&mut folded_tokens);
        }

        Ok(folded_tokens)
    }
}

impl Fold for CheckArithmetic {
    fn fold_stmt(&mut self, stmt: Stmt) -> Stmt {
        let stmt = fold_stmt(self, stmt);
        if let Stmt::Macro(stmt_macro) = stmt {
            let StmtMacro {
                mut attrs,
                mut mac,
                semi_token,
            } = stmt_macro;

            if self.maybe_skip_macro(&mut attrs) {
                Stmt::Macro(StmtMacro {
                    attrs,
                    mac,
                    semi_token,
                })
            } else {
                match self.process_macro_contents(mac.tokens.clone()) {
                    Ok(folded_tokens) => {
                        mac.tokens = folded_tokens;
                        Stmt::Macro(StmtMacro {
                            attrs,
                            mac,
                            semi_token,
                        })
                    }
                    Err(error) => parse2(error.to_compile_error()).unwrap(),
                }
            }
        } else {
            stmt
        }
    }

    fn fold_item_macro(&mut self, mut item_macro: ItemMacro) -> ItemMacro {
        if !self.maybe_skip_macro(&mut item_macro.attrs) {
            let err = syn::Error::new_spanned(
                item_macro.to_token_stream(),
                "cannot process macros - use #[skip_checked_arithmetic] to skip \
                    processing this macro",
            );

            return parse2(err.to_compile_error()).unwrap();
        }
        fold_item_macro(self, item_macro)
    }

    fn fold_expr(&mut self, expr: Expr) -> Expr {
        let span = expr.span();
        let expr = fold_expr(self, expr);
        let expr = match expr {
            Expr::Macro(expr_macro) => {
                let ExprMacro { mut attrs, mut mac } = expr_macro;

                if self.maybe_skip_macro(&mut attrs) {
                    return Expr::Macro(ExprMacro { attrs, mac });
                } else {
                    match self.process_macro_contents(mac.tokens.clone()) {
                        Ok(folded_tokens) => {
                            mac.tokens = folded_tokens;
                            let expr_macro = Expr::Macro(ExprMacro { attrs, mac });
                            quote!(#expr_macro)
                        }
                        Err(error) => {
                            return Expr::Verbatim(error.to_compile_error());
                        }
                    }
                }
            }

            Expr::Binary(expr_binary) => {
                let ExprBinary {
                    attrs,
                    mut left,
                    op,
                    mut right,
                } = expr_binary;

                fn remove_parens(expr: &mut Expr) {
                    if let Expr::Paren(paren) = expr {
                        // i don't even think rust allows this, but just in case
                        assert!(paren.attrs.is_empty(), "TODO: attrs on parenthesized");
                        *expr = *paren.expr.clone();
                    }
                }

                macro_rules! wrap_op {
                    ($left: expr, $right: expr, $method: ident, $span: expr) => {{
                        // Remove parens from exprs since both sides get assigned to tmp variables.
                        // otherwise we get lint errors
                        remove_parens(&mut $left);
                        remove_parens(&mut $right);

                        quote_spanned!($span => {
                            // assign in one stmt in case either #left or #right contains
                            // references to `left` or `right` symbols.
                            let (left, right) = (#left, #right);
                            left.$method(right)
                                .unwrap_or_else(||
                                    panic!(
                                        "Overflow or underflow in {} {} + {}",
                                        stringify!($method),
                                        left,
                                        right,
                                    )
                                )
                        })
                    }};
                }

                macro_rules! wrap_op_assign {
                    ($left: expr, $right: expr, $method: ident, $span: expr) => {{
                        // Remove parens from exprs since both sides get assigned to tmp variables.
                        // otherwise we get lint errors
                        remove_parens(&mut $left);
                        remove_parens(&mut $right);

                        quote_spanned!($span => {
                            // assign in one stmt in case either #left or #right contains
                            // references to `left` or `right` symbols.
                            let (left, right) = (&mut #left, #right);
                            *left = (*left).$method(right)
                                .unwrap_or_else(||
                                    panic!(
                                        "Overflow or underflow in {} {} + {}",
                                        stringify!($method),
                                        *left,
                                        right
                                    )
                                )
                        })
                    }};
                }

                match op {
                    BinOp::Add(_) => {
                        wrap_op!(left, right, checked_add, span)
                    }
                    BinOp::Sub(_) => {
                        wrap_op!(left, right, checked_sub, span)
                    }
                    BinOp::Mul(_) => {
                        wrap_op!(left, right, checked_mul, span)
                    }
                    BinOp::Div(_) => {
                        wrap_op!(left, right, checked_div, span)
                    }
                    BinOp::Rem(_) => {
                        wrap_op!(left, right, checked_rem, span)
                    }
                    BinOp::AddAssign(_) => {
                        wrap_op_assign!(left, right, checked_add, span)
                    }
                    BinOp::SubAssign(_) => {
                        wrap_op_assign!(left, right, checked_sub, span)
                    }
                    BinOp::MulAssign(_) => {
                        wrap_op_assign!(left, right, checked_mul, span)
                    }
                    BinOp::DivAssign(_) => {
                        wrap_op_assign!(left, right, checked_div, span)
                    }
                    BinOp::RemAssign(_) => {
                        wrap_op_assign!(left, right, checked_rem, span)
                    }
                    _ => {
                        let expr_binary = ExprBinary {
                            attrs,
                            left,
                            op,
                            right,
                        };
                        quote_spanned!(span => #expr_binary)
                    }
                }
            }
            Expr::Unary(expr_unary) => {
                let op = &expr_unary.op;
                let operand = &expr_unary.expr;
                match op {
                    UnOp::Neg(_) => {
                        quote_spanned!(span => #operand.checked_neg().expect("Overflow or underflow in negation"))
                    }
                    _ => quote_spanned!(span => #expr_unary),
                }
            }
            _ => quote_spanned!(span => #expr),
        };

        parse2(expr).unwrap()
    }
}

/// This proc macro generates a function `order_to_variant_map` which returns a
/// map of the position of each variant to the name of the variant.
/// It is intended to catch changes in enum order when backward compat is
/// required.
/// ```rust,ignore
///    /// Example for this enum
///    #[derive(EnumVariantOrder)]
///    pub enum MyEnum {
///         A,
///         B(u64),
///         C{x: bool, y: i8},
///     }
///     let order_map = MyEnum::order_to_variant_map();
///     assert!(order_map.get(0).unwrap() == "A");
///     assert!(order_map.get(1).unwrap() == "B");
///     assert!(order_map.get(2).unwrap() == "C");
/// ```
#[proc_macro_derive(EnumVariantOrder)]
pub fn enum_variant_order_derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let name = &ast.ident;

    if let Data::Enum(DataEnum { variants, .. }) = ast.data {
        let variant_entries = variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                let variant_name = variant.ident.to_string();
                quote! {
                    map.insert( #index as u64, (#variant_name).to_string());
                }
            })
            .collect::<Vec<_>>();

        let deriv = quote! {
            impl iota_enum_compat_util::EnumOrderMap for #name {
                fn order_to_variant_map() -> std::collections::BTreeMap<u64, String > {
                    let mut map = std::collections::BTreeMap::new();
                    #(#variant_entries)*
                    map
                }
            }
        };

        deriv.into()
    } else {
        panic!("EnumVariantOrder can only be used with enums.");
    }
}

/// Helper function to extract inner type from Result<T, E>, IotaResult<T, E>,
/// Future<Output = Result<T, E>>, impl Future<Output = Result<T, E>>, etc.
/// Returns None if the type doesn't match any of these patterns
/// For Future<Output = Result<(), E>> or Future<Output = IotaResult<(), E>>,
/// returns ()
fn extract_inner_type_from_result_or_future(ty: &syn::Type) -> Option<proc_macro2::TokenStream> {
    match ty {
        syn::Type::Path(type_path) => {
            let segment = type_path.path.segments.last()?;

            // Handle direct Result<T, E> or IotaResult<T, E>
            if segment.ident == "Result" || segment.ident == "IotaResult" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(quote! { #inner });
                    } else {
                        // Handle IotaResult without explicit type arguments (defaults to ())
                        return Some(quote! { () });
                    }
                }
            }

            // Handle Future<Output = Result<T, E>>, Future<Output = IotaResult<T, E>>, 
            // BoxFuture<'_, Result<T, E>>, BoxFuture<'_, IotaResult<T, E>>
            if segment.ident == "Future" || segment.ident == "BoxFuture" {
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
            }

            None
        }
        syn::Type::ImplTrait(impl_trait) => {
            // Handle impl Future<Output = Result<T, E>> or impl Future<Output =
            // IotaResult<T, E>>
            for bound in &impl_trait.bounds {
                if let syn::TypeParamBound::Trait(trait_bound) = bound {
                    let path = &trait_bound.path;
                    if let Some(segment) = path.segments.last() {
                        if segment.ident == "Future" || segment.ident == "BoxFuture" {
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
        _ => None,
    }
}

/// Helper function to extract inner type from Result<T, E> or IotaResult<T, E>
fn extract_inner_from_result_type(ty: &syn::Type) -> Option<proc_macro2::TokenStream> {
    if let syn::Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        if segment.ident == "Result" || segment.ident == "IotaResult" {
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                    // Check if the inner type is () - if so, return ()
                    if let syn::Type::Tuple(tuple) = inner {
                        if tuple.elems.is_empty() {
                            return Some(quote! { () });
                        }
                    }
                    return Some(quote! { #inner });
                } else {
                    // Handle IotaResult without explicit type arguments (defaults to ())
                    return Some(quote! { () });
                }
            } else {
                // Handle IotaResult without any angle brackets (defaults to ())
                return Some(quote! { () });
            }
        }
    }
    None
}

/// Helper function to parse the expect message from macro attributes
fn parse_expect_message(attr: TokenStream, default_msg: &str) -> String {
    if attr.is_empty() {
        default_msg.to_string()
    } else {
        match syn::parse::<LitStr>(attr) {
            Ok(lit_str) => lit_str.value(),
            Err(_) => default_msg.to_string(),
        }
    }
}

/// Helper function to determine if a return type represents a Future
fn is_future_return_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(type_path) => {
            type_path.path.segments.last().map_or(false, |seg| {
                seg.ident == "Future" || seg.ident == "BoxFuture"
            })
        }
        syn::Type::ImplTrait(impl_trait) => impl_trait.bounds.iter().any(|bound| {
            if let syn::TypeParamBound::Trait(trait_bound) = bound {
                trait_bound.path.segments.last().map_or(false, |seg| {
                    seg.ident == "Future" || seg.ident == "BoxFuture"
                })
            } else {
                false
            }
        }),
        _ => false,
    }
}

/// Helper function to check if a function signature has a self parameter
fn has_self_parameter(inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>) -> bool {
    inputs.iter().any(|arg| matches!(arg, syn::FnArg::Receiver(_)))
}

/// Helper function to generate argument identifiers for method calls
/// This converts patterns like `_` to usable identifiers in function calls
fn generate_arg_identifiers(inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>) -> Vec<syn::Ident> {
    let mut arg_counter = 0usize;
    inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_ty) => {
                Some(match &*pat_ty.pat {
                    syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                    syn::Pat::Wild(_) => {
                        // Generate a unique parameter name for wildcard patterns
                        let ident = format_ident!("__arg_{}", arg_counter);
                        arg_counter += 1;
                        ident
                    }
                    _ => {
                        // For other complex patterns, generate a parameter name
                        let ident = format_ident!("__arg_{}", arg_counter);
                        arg_counter += 1;
                        ident
                    }
                })
            }
        })
        .collect()
}

/// Helper function to generate proper function parameters for the new signature
/// This converts wildcard patterns to named parameters
fn generate_function_inputs(inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>) -> syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma> {
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
                    syn::Pat::Wild(_) => {
                        // Replace wildcard with a named parameter
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
                    _ => {
                        // For other complex patterns, replace with a named parameter
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

/// Helper function to generate a non-fallible method call
fn generate_non_fallible_call(
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

/// Helper struct to hold information about a try_ method
struct TryMethodInfo {
    pub original_name: syn::Ident,
    pub new_name: syn::Ident,
    pub inputs: syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
    pub inner_return_type: proc_macro2::TokenStream,
    pub is_future: bool,
    pub has_self: bool,
    pub generics: syn::Generics,
}

/// Helper function to analyze a try_ method signature and extract relevant information
fn analyze_try_method(sig: &syn::Signature) -> Option<TryMethodInfo> {
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
    let inner_ty = extract_inner_type_from_result_or_future(&**output)?;
    let is_future = is_future_return_type(&**output);
    let has_self = has_self_parameter(&sig.inputs);

    Some(TryMethodInfo {
        original_name: method_name.clone(),
        new_name: new_method_name,
        inputs: sig.inputs.clone(),
        inner_return_type: inner_ty,
        is_future,
        has_self,
        generics: sig.generics.clone(),
    })
}

/// This macro generates a non-fallible version of a function that returns
/// Result or IotaResult. For a function named `try_foo` returning `Result<T,
/// E>` or `IotaResult<T, E>`, it generates a function named `foo` that returns
/// `T`.
///
/// It also works with async functions that return Future<Output = Result<T, E>>
/// or similar.
///
/// Example:
/// ```rust,ignore
/// #[generate_non_fallible_fn("Error message")]
/// fn try_get_value(key: &str) -> Result<String, Error> {
///     // implementation
/// }
/// // The macro generates:
/// // fn get_value(key: &str) -> String {
/// //     try_get_value(key).expect("Error message")
/// // }
/// ```
#[proc_macro_attribute]
pub fn generate_non_fallible_fn(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as syn::ItemFn);
    
    // Analyze the try_ method
    let method_info = match analyze_try_method(&input_fn.sig) {
        Some(info) => info,
        None => {
            return syn::Error::new_spanned(
                &input_fn.sig.ident, 
                "Function name must start with `try_` and return a Result type"
            )
            .to_compile_error()
            .into();
        }
    };

    // Get the error message for .expect()
    let expect_msg = parse_expect_message(attr, "Unwrapped failed in generated non-fallible function");

    // Check if the function is async or returns a Future
    let is_async_fn = input_fn.sig.asyncness.is_some();
    let returns_future = method_info.is_future;

    // Generate identifiers for function call and parameters for new signature
    let arg_identifiers = generate_arg_identifiers(&method_info.inputs);
    let new_inputs = generate_function_inputs(&method_info.inputs);

    // Generate the non-fallible function call (for free functions, we don't use Self::)
    let original_name = &method_info.original_name;
    let call = if is_async_fn || returns_future {
        quote! {
            #original_name(#(#arg_identifiers),*).await.expect(#expect_msg)
        }
    } else {
        quote! {
            #original_name(#(#arg_identifiers),*).expect(#expect_msg)
        }
    };

    // Generate the new function with visibility, attributes, and generics preserved
    let vis = &input_fn.vis;
    let generics = &input_fn.sig.generics;
    let where_clause = &input_fn.sig.generics.where_clause;
    let new_fn_name = &method_info.new_name;
    let inner_ty_tokens = &method_info.inner_return_type;

    let new_fn_sig_asyncness = if is_async_fn && !returns_future {
        // Only add async if it's an async fn and not returning BoxFuture
        // BoxFuture returns should not be async to maintain object safety
        quote! { async }
    } else {
        quote! {}
    };

    let generated = quote! {
        #input_fn

        #vis #new_fn_sig_asyncness fn #new_fn_name #generics(#new_inputs) -> #inner_ty_tokens #where_clause {
            #call
        }
    };

    generated.into()
}

/// This macro extends an existing trait with non-fallible versions of its
/// `try_` methods. It adds the non-fallible methods directly to the original
/// trait. For methods with default implementations, it generates default 
/// implementations that call the `try_` method and use `.expect()`.
///
/// Example:
/// ```rust,ignore
/// #[extend_trait_with_non_fallible("Operation failed")]
/// pub trait MyService {
///     fn try_get_data(&self) -> Result<String, Error>;
///     // The macro will add:
///     // fn get_data(&self) -> String;
///     
///     fn try_get_many(&self, ids: &[u32]) -> Result<Vec<String>, Error> {
///         // default implementation
///     }
///     // The macro will add:
///     // fn get_many(&self, ids: &[u32]) -> Vec<String> {
///     //     self.try_get_many(ids).expect("Operation failed")
///     // }
/// }
/// ```
#[proc_macro_attribute]
pub fn extend_trait_with_non_fallible(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input_trait = parse_macro_input!(item as ItemTrait);
    
    let expect_msg = parse_expect_message(attr, "Unwrapped failed in non-fallible method");
    let mut non_fallible_methods = Vec::new();

    for item in &input_trait.items {
        if let TraitItem::Fn(TraitItemFn { sig, default, .. }) = item {
            let method_info = match analyze_try_method(sig) {
                Some(info) => info,
                None => continue,
            };

            // Split into two separate flags for better control
            let is_async_fn = sig.asyncness.is_some();
            let returns_future = method_info.is_future;

            // Check if the method has a default implementation
            let has_default_impl = default.is_some();

            let method_item = if has_default_impl {
                // Generate a default implementation that calls the try_ method with expect
                let arg_identifiers = generate_arg_identifiers(&method_info.inputs);
                let call = if returns_future && has_default_impl {
                    // For trait default implementations that return BoxFuture, we need to return the boxed future
                    let original_name = &method_info.original_name;
                    quote! {
                        Box::pin(async move {
                            self.#original_name(#(#arg_identifiers),*).await.expect(#expect_msg)
                        })
                    }
                } else {
                    generate_non_fallible_call(
                        &method_info.original_name,
                        &arg_identifiers,
                        &expect_msg,
                        true, // traits always use self
                        is_async_fn, // Use .await only for async fn (not for BoxFuture returns)
                        None, // No trait qualification needed for trait default implementations
                        None, // No concrete self type needed for trait implementations
                    )
                };

                let new_name = &method_info.new_name;
                let new_inputs = generate_function_inputs(&method_info.inputs);
                let return_type = if returns_future {
                    // For BoxFuture returns, preserve the BoxFuture structure with proper lifetimes
                    let inner_type = &method_info.inner_return_type;
                    // Use the same lifetime structure as the original method
                    if method_info.generics.lt_token.is_some() {
                        // Method has explicit lifetime parameters, use 'a
                        quote! { BoxFuture<'a, #inner_type> }
                    } else {
                        // Method uses elided lifetimes, use '_'
                        quote! { BoxFuture<'_, #inner_type> }
                    }
                } else {
                    method_info.inner_return_type.clone()
                };
                let generics = &method_info.generics;
                let where_clause = &method_info.generics.where_clause;

                // Generate non-async methods to maintain object safety (BoxFuture instead of async)
                quote! {
                    fn #new_name #generics(#new_inputs) -> #return_type #where_clause {
                        #call
                    }
                }
            } else {
                // Generate just a signature for methods without default implementations
                let new_name = &method_info.new_name;
                let new_inputs = generate_function_inputs(&method_info.inputs);
                let return_type = if returns_future {
                    // For BoxFuture returns, preserve the BoxFuture structure with proper lifetimes
                    let inner_type = &method_info.inner_return_type;
                    // Use the same lifetime structure as the original method
                    if method_info.generics.lt_token.is_some() {
                        // Method has explicit lifetime parameters, use 'a
                        quote! { BoxFuture<'a, #inner_type> }
                    } else {
                        // Method uses elided lifetimes, use '_'
                        quote! { BoxFuture<'_, #inner_type> }
                    }
                } else {
                    method_info.inner_return_type.clone()
                };
                let generics = &method_info.generics;
                let where_clause = &method_info.generics.where_clause;

                // Generate non-async method signatures to maintain object safety
                quote! {
                    fn #new_name #generics(#new_inputs) -> #return_type #where_clause;
                }
            };

            non_fallible_methods.push(method_item);
        }
    }

    // Add the non-fallible methods to the trait definition
    for method in non_fallible_methods {
        input_trait.items.push(syn::parse2(method).unwrap());
    }

    let expanded = quote! {
        #input_trait
    };

    expanded.into()
}

/// This macro extends an existing trait implementation with non-fallible
/// versions of its `try_` methods. It adds the non-fallible methods
/// directly to the original implementation.
///
/// Example:
/// ```rust,ignore
/// #[extend_impl_with_non_fallible("Operation failed")]
/// impl MyService for MyServiceImpl {
///     fn try_get_data(&self) -> Result<String, Error> {
///         // implementation
///     }
///     // The macro will add:
///     // fn get_data(&self) -> String {
///     //     self.try_get_data().expect("Operation failed")
///     // }
/// }
/// ```
#[proc_macro_attribute]
pub fn extend_impl_with_non_fallible(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_impl = parse_macro_input!(item as ItemImpl);

    let expect_msg = parse_expect_message(attr, "Unwrapped failed in non-fallible method");
    
    // Extract trait path for explicit qualification - we'll always use this when available
    let trait_path = item_impl.trait_.as_ref().map(|(_, path, _)| path);
    
    let mut non_fallible_methods = Vec::new();

    for item in &item_impl.items {
        if let ImplItem::Fn(ImplItemFn { sig, .. }) = item {
            let method_info = match analyze_try_method(sig) {
                Some(info) => info,
                None => continue,
            };

            // Split into two separate flags for better control
            let is_async_fn = sig.asyncness.is_some();
            let returns_future = method_info.is_future;

            let arg_identifiers = generate_arg_identifiers(&method_info.inputs);
            let new_name = &method_info.new_name;
            let new_inputs = generate_function_inputs(&method_info.inputs);
            let return_type = if returns_future {
                // For BoxFuture returns, preserve the BoxFuture structure with proper lifetimes
                let inner_type = &method_info.inner_return_type;
                // Use the same lifetime structure as the original method
                if method_info.generics.lt_token.is_some() {
                    // Method has explicit lifetime parameters, use 'a
                    quote! { BoxFuture<'a, #inner_type> }
                } else {
                    // Method uses elided lifetimes, use '_'
                    quote! { BoxFuture<'_, #inner_type> }
                }
            } else {
                method_info.inner_return_type.clone()
            };
            let generics = &method_info.generics;
            let where_clause = &method_info.generics.where_clause;
            
            let method_sig = if returns_future {
                // For BoxFuture returns, we need to generate a special call that awaits and expects
                let original_name = &method_info.original_name;
                let call_with_await_expect = if method_info.has_self {
                    if let Some(trait_path) = trait_path {
                        // Use explicit trait qualification - need to pass self explicitly
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
                
                let boxed_call = quote! {
                    Box::pin(async move {
                        #call_with_await_expect
                    })
                };
                quote! {
                    fn #new_name #generics(#new_inputs) -> #return_type #where_clause {
                        #boxed_call
                    }
                }
            } else {
                // For non-BoxFuture methods, use the normal call generation
                let call = generate_non_fallible_call(
                    &method_info.original_name,
                    &arg_identifiers,
                    &expect_msg,
                    method_info.has_self,
                    is_async_fn, // Use .await only for async fn
                    trait_path, // This will ensure we generate <Self as Trait>::method calls
                    Some(item_impl.self_ty.as_ref()), // Pass the concrete self type for macro contexts
                );
                
                if is_async_fn {
                    // For async fn, generate async method
                    quote! {
                        async fn #new_name #generics(#new_inputs) -> #return_type #where_clause {
                            #call
                        }
                    }
                } else {
                    // For regular sync methods
                    quote! {
                        fn #new_name #generics(#new_inputs) -> #return_type #where_clause {
                            #call
                        }
                    }
                }
            };

            non_fallible_methods.push(syn::parse2(method_sig).unwrap());
        }
    }

    // Add the non-fallible methods to the impl block
    for method in non_fallible_methods {
        item_impl.items.push(method);
    }

    let generated = quote! {
        #item_impl
    };

    generated.into()
}
