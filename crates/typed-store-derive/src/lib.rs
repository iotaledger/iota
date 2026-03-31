// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{
    AngleBracketedGenericArguments, Attribute, Generics, ItemStruct, Lit, Meta, PathArguments,
    Type::{self},
    parse_macro_input,
};

// This is used as default when none is specified
const DEFAULT_DB_OPTIONS_CUSTOM_FN: &str = "typed_store::rocks::default_db_options";
// Custom function which returns the option and overrides the defaults for this
// table
const DB_OPTIONS_CUSTOM_FUNCTION: &str = "default_options_override_fn";
// Use a different name for the column than the identifier
const DB_OPTIONS_RENAME: &str = "rename";
// Deprecate a column family with optional migration support
// Usage: `#[deprecated_db_map]` or `#[deprecated_db_map(migration =
// "migration_fn_path")]`
// Hint: we can't use `#[deprecated]` because it doesn't allow us to specify a
// migration parameter
const DB_OPTIONS_DEPRECATED_TABLE: &str = "deprecated_db_map";

/// Options can either be simplified form or
enum GeneralTableOptions {
    OverrideFunction(String),
}

impl Default for GeneralTableOptions {
    fn default() -> Self {
        Self::OverrideFunction(DEFAULT_DB_OPTIONS_CUSTOM_FN.to_owned())
    }
}

/// Parse the migration function path from `#[deprecated_db_map(migration =
/// "fn_path")]`
fn parse_deprecated_db_map_migration(attr: &Attribute) -> Option<syn::Path> {
    match attr.parse_meta() {
        Ok(Meta::Path(_)) => None, // #[deprecated_db_map] with no args
        Ok(Meta::List(ml)) => {
            for nested in &ml.nested {
                if let syn::NestedMeta::Meta(Meta::NameValue(nv)) = nested {
                    if nv.path.is_ident("migration") {
                        if let Lit::Str(s) = &nv.lit {
                            let fn_path: syn::Path =
                                s.parse().expect("migration value must be a valid path");
                            return Some(fn_path);
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

// Extracts the field names, field types, inner types (K,V in {map_type_name}<K,
// V>), and the options attrs
fn extract_struct_info(input: ItemStruct) -> ExtractedStructInfo {
    let mut active_table_fields = TableFields::default();
    let mut deprecated_table_fields = TableFields::default();
    let mut deprecated_cfs_with_migration_opts: Vec<(Ident, Option<syn::Path>)> = Vec::new();

    for f in &input.fields {
        let attrs: BTreeMap<_, _> = f
            .attrs
            .iter()
            .filter(|a| {
                a.path.is_ident(DB_OPTIONS_CUSTOM_FUNCTION)
                    || a.path.is_ident(DB_OPTIONS_RENAME)
                    || a.path.is_ident(DB_OPTIONS_DEPRECATED_TABLE)
            })
            .map(|a| (a.path.get_ident().unwrap().to_string(), a))
            .collect();

        let options = if let Some(options) = attrs.get(DB_OPTIONS_CUSTOM_FUNCTION) {
            GeneralTableOptions::OverrideFunction(get_options_override_function(options).unwrap())
        } else {
            GeneralTableOptions::default()
        };

        let Type::Path(p) = &f.ty else {
            panic!("All struct members must be of type DBMap or Option<DBMap>");
        };

        let is_deprecated = attrs.contains_key(DB_OPTIONS_DEPRECATED_TABLE);

        let type_info = &p.path.segments.first().unwrap();

        // For deprecated fields, unwrap Option<DBMap<K,V>> to extract DBMap<K,V>.
        // Active fields must be DBMap<K,V> directly.
        let (db_map_ident_str, inner_type) = if is_deprecated {
            if type_info.ident == "Option" {
                // Extract DBMap<K,V> from Option<DBMap<K,V>>
                let option_args = match &type_info.arguments {
                    PathArguments::AngleBracketed(ab) => ab,
                    _ => panic!(
                        "Expected Option<DBMap<K, V>> for deprecated field `{}`",
                        f.ident.as_ref().unwrap()
                    ),
                };
                let inner_path = match option_args.args.first() {
                    Some(syn::GenericArgument::Type(Type::Path(p))) => p,
                    _ => panic!(
                        "Expected Option<DBMap<K, V>> for deprecated field `{}`",
                        f.ident.as_ref().unwrap()
                    ),
                };
                let inner_info = inner_path.path.segments.first().unwrap();
                let inner_type = match &inner_info.arguments {
                    PathArguments::AngleBracketed(ab) => ab.clone(),
                    _ => panic!(
                        "Expected DBMap<K, V> inside Option for deprecated field `{}`",
                        f.ident.as_ref().unwrap()
                    ),
                };
                (inner_info.ident.to_string(), inner_type)
            } else {
                panic!(
                    "Deprecated field `{}` must use Option<DBMap<K, V>> instead of DBMap<K, V>",
                    f.ident.as_ref().unwrap()
                );
            }
        } else {
            let inner_type =
                if let PathArguments::AngleBracketed(angle_bracket_type) = &type_info.arguments {
                    angle_bracket_type.clone()
                } else {
                    panic!("All struct members must be of type DBMap");
                };
            (type_info.ident.to_string(), inner_type)
        };

        assert!(
            db_map_ident_str == "DBMap",
            "All struct members must be of type DBMap (or Option<DBMap> for deprecated fields)"
        );

        let field_name = f.ident.as_ref().unwrap().clone();
        let cf_name = if let Some(rename) = attrs.get(DB_OPTIONS_RENAME) {
            match rename.parse_meta().expect("Cannot parse meta of attribute") {
                Meta::NameValue(val) => {
                    if let Lit::Str(s) = val.lit {
                        // convert to ident
                        s.parse().expect("Rename value must be identifier")
                    } else {
                        panic!("Expected string value for rename")
                    }
                }
                _ => panic!("Expected string value for rename"),
            }
        } else {
            field_name.clone()
        };

        // None: active cf
        // Some(None): deprecated cf with no migration
        // Some(Some(fn_path)): deprecated cf with migration
        let deprecated_cf_with_migration_opt = attrs
            .get(DB_OPTIONS_DEPRECATED_TABLE)
            .map(|attr| parse_deprecated_db_map_migration(attr));

        let target_table_fields = if let Some(migration_opt) = deprecated_cf_with_migration_opt {
            deprecated_cfs_with_migration_opts.push((cf_name.clone(), migration_opt));
            &mut deprecated_table_fields
        } else {
            &mut active_table_fields
        };

        target_table_fields.field_names.push(field_name);
        target_table_fields.cf_names.push(cf_name);
        target_table_fields.inner_types.push(inner_type);
        target_table_fields.derived_table_options.push(options);
    }

    ExtractedStructInfo {
        active_table_fields,
        deprecated_table_fields,
        deprecated_cfs_with_migration_opts,
    }
}

/// Extracts the table options override function
/// The function must take no args and return Options
fn get_options_override_function(attr: &Attribute) -> syn::Result<String> {
    let meta = attr.parse_meta()?;

    let val = match meta.clone() {
        Meta::NameValue(val) => val,
        _ => {
            return Err(syn::Error::new_spanned(
                meta,
                format!(
                    "Expected function name in format `#[{DB_OPTIONS_CUSTOM_FUNCTION} = {{function_name}}]`"
                ),
            ));
        }
    };

    if !val.path.is_ident(DB_OPTIONS_CUSTOM_FUNCTION) {
        return Err(syn::Error::new_spanned(
            meta,
            format!(
                "Expected function name in format `#[{DB_OPTIONS_CUSTOM_FUNCTION} = {{function_name}}]`"
            ),
        ));
    }

    let fn_name = match val.lit {
        Lit::Str(fn_name) => fn_name,
        _ => {
            return Err(syn::Error::new_spanned(
                meta,
                format!(
                    "Expected function name in format `#[{DB_OPTIONS_CUSTOM_FUNCTION} = {{function_name}}]`"
                ),
            ));
        }
    };
    Ok(fn_name.value())
}

fn extract_generics_names(generics: &Generics) -> Vec<Ident> {
    generics
        .params
        .iter()
        .map(|g| match g {
            syn::GenericParam::Type(t) => t.ident.clone(),
            _ => panic!("Unsupported generic type"),
        })
        .collect()
}

/// Parallel vecs describing a set of DB column families.
#[derive(Default)]
struct TableFields {
    field_names: Vec<Ident>,
    cf_names: Vec<Ident>,
    inner_types: Vec<AngleBracketedGenericArguments>,
    derived_table_options: Vec<GeneralTableOptions>,
}

struct ExtractedStructInfo {
    /// Active (non-deprecated) tables
    active_table_fields: TableFields,
    /// Deprecated tables
    deprecated_table_fields: TableFields,
    /// CF names paired with their optional migration function paths
    deprecated_cfs_with_migration_opts: Vec<(Ident, Option<syn::Path>)>,
}

#[proc_macro_derive(
    DBMapUtils,
    attributes(default_options_override_fn, rename, deprecated_db_map)
)]
pub fn derive_dbmap_utils_general(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let name = &input.ident;
    let generics = &input.generics;
    let generics_names = extract_generics_names(generics);

    // TODO: use `parse_quote` over `parse()`
    let ExtractedStructInfo {
        active_table_fields,
        deprecated_table_fields,
        deprecated_cfs_with_migration_opts,
    } = extract_struct_info(input.clone());

    // Bind fields for use in quote! macro
    let active_field_names = &active_table_fields.field_names;
    let active_cf_names = &active_table_fields.cf_names;
    let active_inner_types = &active_table_fields.inner_types;
    let deprecated_field_names = &deprecated_table_fields.field_names;
    let deprecated_cf_names = &deprecated_table_fields.cf_names;
    let deprecated_inner_types = &deprecated_table_fields.inner_types;

    // Combined field names for struct definitions (need all fields, active +
    // deprecated)
    let all_field_names: Vec<_> = active_field_names
        .iter()
        .chain(deprecated_field_names.iter())
        .collect();

    let active_default_options_override_fn_names: Vec<proc_macro2::TokenStream> =
        active_table_fields
            .derived_table_options
            .iter()
            .map(|q| {
                let GeneralTableOptions::OverrideFunction(fn_name) = q;
                fn_name.parse().unwrap()
            })
            .collect();

    let active_key_names: Vec<_> = active_inner_types
        .iter()
        .map(|q| q.args.first().unwrap())
        .collect();
    let active_value_names: Vec<_> = active_inner_types
        .iter()
        .map(|q| q.args.last().unwrap())
        .collect();

    let generics_bounds =
        "std::fmt::Debug + serde::Serialize + for<'de> serde::de::Deserialize<'de>";
    let generics_bounds_token: proc_macro2::TokenStream = generics_bounds.parse().unwrap();

    let intermediate_db_map_struct_name_str = format!("{name}IntermediateDBMapStructPrimary");
    let intermediate_db_map_struct_name: proc_macro2::TokenStream =
        intermediate_db_map_struct_name_str.parse().unwrap();

    let secondary_db_map_struct_name_str = format!("{name}ReadOnly");
    let secondary_db_map_struct_name: proc_macro2::TokenStream =
        secondary_db_map_struct_name_str.parse().unwrap();

    // Generate deprecation cleanup code: for each deprecated CF, optionally run
    // migration then drop the CF. Uses cf_handle check for idempotency.
    // Note: Migration functions are only called when the CF exists on disk.
    // They receive `&Arc<RocksDB>` and may assume the deprecated CF is present.
    let deprecation_cleanup: Vec<proc_macro2::TokenStream> = deprecated_cfs_with_migration_opts
        .iter()
        .map(|(cf_name, migration)| {
            let migration_call = if let Some(fn_path) = migration {
                quote! { #fn_path(&db).expect("deprecated table migration failed"); }
            } else {
                quote! {}
            };
            quote! {
                if db.cf_handle(stringify!(#cf_name)).is_some() {
                    #migration_call
                    db.drop_cf(stringify!(#cf_name)).expect("failed to drop a deprecated cf");
                }
            }
        })
        .collect();

    TokenStream::from(quote! {

        // <----------- This section generates the core open logic for opening DBMaps -------------->

        /// Create an intermediate struct used to open the DBMap tables in primary mode
        /// This is only used internally
        struct #intermediate_db_map_struct_name #generics {
                #(
                    pub #active_field_names : DBMap #active_inner_types,
                )*
                #(
                    pub #deprecated_field_names : Option<DBMap #deprecated_inner_types>,
                )*
        }

        impl <
                #(
                    #generics_names: #generics_bounds_token,
                )*
            > #intermediate_db_map_struct_name #generics {
            /// Opens a set of tables in read-write mode
            /// If as_secondary_with_path is set, the DB is opened in read only mode with the path specified
            pub fn open_tables_impl(
                path: std::path::PathBuf,
                as_secondary_with_path: Option<std::path::PathBuf>,
                metric_conf: typed_store::rocks::MetricConf,
                global_db_options_override: Option<typed_store::rocksdb::Options>,
                tables_db_options_override: Option<typed_store::rocks::DBMapTableConfigMap>,
            ) -> Self {
                let path = &path;
                let default_cf_opt = if let Some(opt) = global_db_options_override.as_ref() {
                    typed_store::rocks::DBOptions {
                        options: opt.clone(),
                        rw_options: typed_store::rocks::default_db_options().rw_options,
                    }
                } else {
                    typed_store::rocks::default_db_options()
                };
                let (db, rwopt_cfs) = {
                    let opt_cfs = match tables_db_options_override {
                        None => [
                            #(
                                (stringify!(#active_cf_names).to_owned(), #active_default_options_override_fn_names()),
                            )*
                        ],
                        Some(o) => [
                            #(
                                (stringify!(#active_cf_names).to_owned(), o.to_map().get(stringify!(#active_cf_names)).unwrap_or(&default_cf_opt).clone()),
                            )*
                        ]
                    };
                    // Safe to call unwrap because we will have at least one field_name entry in the struct
                    let rwopt_cfs: std::collections::HashMap<String, typed_store::rocks::ReadWriteOptions> = opt_cfs.iter().map(|q| (q.0.as_str().to_string(), q.1.rw_options.clone())).collect();
                    let opt_cfs: Vec<_> = opt_cfs.iter().map(|q| (q.0.as_str(), q.1.options.clone())).collect();
                    let db = match as_secondary_with_path.clone() {
                        Some(p) => typed_store::rocks::open_cf_opts_secondary(path, Some(&p), global_db_options_override, metric_conf, &opt_cfs),
                        _ => typed_store::rocks::open_cf_opts(path, global_db_options_override, metric_conf, &opt_cfs)
                    };
                    db.map(|d| (d, rwopt_cfs))
                }.expect(&format!("Cannot open DB at {:?}", path));
                let (
                        #(
                            #active_field_names
                        ),*
                ) = (#(
                        DBMap::#active_inner_types::reopen(&db, Some(stringify!(#active_cf_names)), rwopt_cfs.get(stringify!(#active_cf_names)).unwrap_or(&typed_store::rocks::ReadWriteOptions::default()), false).expect(&format!("Cannot open {} CF.", stringify!(#active_cf_names))[..])
                    ),*);

                // Open deprecated CFs only if they exist on disk.
                // `mut` is needed because primary mode reassigns to `None` after cleanup,
                // but in secondary mode no reassignment happens — hence `allow(unused_mut)`.
                #(
                    #[allow(unused_mut)]
                    let mut #deprecated_field_names = if db.cf_handle(stringify!(#deprecated_cf_names)).is_some() {
                        Some(DBMap::#deprecated_inner_types::reopen(
                            &db,
                            Some(stringify!(#deprecated_cf_names)),
                            rwopt_cfs.get(stringify!(#deprecated_cf_names)).unwrap_or(&typed_store::rocks::ReadWriteOptions::default()),
                            true,
                        ).expect(&format!("Cannot open deprecated {} CF.", stringify!(#deprecated_cf_names))[..]))
                    } else {
                        None
                    };
                )*

                if as_secondary_with_path.is_none() {
                    #(#deprecation_cleanup)*
                    // After cleanup, CF handles for deprecated tables are stale
                    #(
                        #deprecated_field_names = None;
                    )*
                }
                Self {
                    #(
                        #all_field_names,
                    )*
                }
            }
        }


        // <----------- This section generates the read-write open logic and other common utils -------------->

        impl <
                #(
                    #generics_names: #generics_bounds_token,
                )*
            > #name #generics {
            /// Opens a set of tables in read-write mode
            /// Only one process is allowed to do this at a time
            /// `global_db_options_override` apply to the whole DB
            /// `tables_db_options_override` apply to each table. If `None`, the attributes from `default_options_override_fn` are used if any
            #[expect(unused_parens)]
            pub fn open_tables_read_write(
                path: std::path::PathBuf,
                metric_conf: typed_store::rocks::MetricConf,
                global_db_options_override: Option<typed_store::rocksdb::Options>,
                tables_db_options_override: Option<typed_store::rocks::DBMapTableConfigMap>
            ) -> Self {
                let inner = #intermediate_db_map_struct_name::open_tables_impl(path, None, metric_conf, global_db_options_override, tables_db_options_override);
                Self {
                    #(
                        #all_field_names: inner.#all_field_names,
                    )*
                }
            }

            /// Returns a list of the tables name and type pairs
            pub fn describe_tables() -> std::collections::BTreeMap<String, (String, String)> {
                vec![#(
                    (stringify!(#active_field_names).to_owned(), (stringify!(#active_key_names).to_owned(), stringify!(#active_value_names).to_owned())),
                )*].into_iter().collect()
            }

            /// This opens the DB in read only mode and returns a struct which exposes debug features
            pub fn get_read_only_handle (
                primary_path: std::path::PathBuf,
                with_secondary_path: Option<std::path::PathBuf>,
                global_db_options_override: Option<typed_store::rocksdb::Options>,
                metric_conf: typed_store::rocks::MetricConf,
                ) -> #secondary_db_map_struct_name #generics {
                #secondary_db_map_struct_name::open_tables_read_only(primary_path, with_secondary_path, metric_conf, global_db_options_override)
            }
        }


        // <----------- This section generates the features that use read-only open logic -------------->
        /// Create an intermediate struct used to open the DBMap tables in secondary mode
        /// This is only used internally
        pub struct #secondary_db_map_struct_name #generics {
            #(
                pub #active_field_names : DBMap #active_inner_types,
            )*
            #(
                pub #deprecated_field_names : Option<DBMap #deprecated_inner_types>,
            )*
        }

        impl <
                #(
                    #generics_names: #generics_bounds_token,
                )*
            > #secondary_db_map_struct_name #generics {
            /// Open in read only mode. No limitation on number of processes to do this
            pub fn open_tables_read_only(
                primary_path: std::path::PathBuf,
                with_secondary_path: Option<std::path::PathBuf>,
                metric_conf: typed_store::rocks::MetricConf,
                global_db_options_override: Option<typed_store::rocksdb::Options>,
            ) -> Self {
                let inner = match with_secondary_path {
                    Some(q) => #intermediate_db_map_struct_name::open_tables_impl(primary_path, Some(q), metric_conf, global_db_options_override, None),
                    None => {
                        let p: std::path::PathBuf = tempfile::tempdir()
                        .expect("Failed to open temporary directory")
                        .keep();
                        #intermediate_db_map_struct_name::open_tables_impl(primary_path, Some(p), metric_conf, global_db_options_override, None)
                    }
                };
                Self {
                    #(
                        #all_field_names: inner.#all_field_names,
                    )*
                }
            }

            fn cf_name_to_table_name(cf_name: &str) -> eyre::Result<&'static str> {
                Ok(match cf_name {
                    #(
                        stringify!(#active_cf_names) => stringify!(#active_field_names),
                    )*
                    _ => eyre::bail!("No such cf name: {}", cf_name),
                })
            }

            /// Dump all key-value pairs in the page at the given table name
            /// Tables must be opened in read only mode using `open_tables_read_only`
            pub fn dump(&self, cf_name: &str, page_size: u16, page_number: usize) -> eyre::Result<std::collections::BTreeMap<String, String>> {
                let table_name = Self::cf_name_to_table_name(cf_name)?;

                Ok(match table_name {
                    #(
                        stringify!(#active_field_names) => {
                            typed_store::traits::Map::try_catch_up_with_primary(&self.#active_field_names)?;
                            typed_store::traits::Map::safe_iter(&self.#active_field_names)
                                .skip((page_number * (page_size) as usize))
                                .take(page_size as usize)
                                .map(|result| result.map(|(k, v)| (format!("{:?}", k), format!("{:?}", v))))
                                .collect::<eyre::Result<std::collections::BTreeMap<_, _>, _>>()?
                        }
                    )*

                    _ => eyre::bail!("No such table name: {}", table_name),
                })
            }

            /// Get key value sizes from the db
            /// Tables must be opened in read only mode using `open_tables_read_only`
            pub fn table_summary(&self, table_name: &str) -> eyre::Result<typed_store::traits::TableSummary> {
                let mut count = 0;
                let mut key_bytes = 0;
                let mut value_bytes = 0;
                match table_name {
                    #(
                        stringify!(#active_field_names) => {
                            typed_store::traits::Map::try_catch_up_with_primary(&self.#active_field_names)?;
                            self.#active_field_names.table_summary()
                        }
                    )*

                    _ => eyre::bail!("No such table name: {}", table_name),
                }
            }

            pub fn describe_tables() -> std::collections::BTreeMap<String, (String, String)> {
                vec![#(
                    (stringify!(#active_field_names).to_owned(), (stringify!(#active_key_names).to_owned(), stringify!(#active_value_names).to_owned())),
                )*].into_iter().collect()
            }

            /// Try catch up with primary for all tables. This can be a slow operation
            /// Tables must be opened in read only mode using `open_tables_read_only`
            pub fn try_catch_up_with_primary_all(&self) -> eyre::Result<()> {
                #(
                    typed_store::traits::Map::try_catch_up_with_primary(&self.#active_field_names)?;
                )*
                Ok(())
            }
        }
    })
}
