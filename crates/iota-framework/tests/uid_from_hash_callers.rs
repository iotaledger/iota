// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Pins the functions that can mint a `UID` at an address their caller
//! chooses, read from the compiled system packages, so that an aliased import
//! or a reformatted call cannot hide a caller.
//!
//! Post-consensus validation keeps at most one `ClaimAccount` per address on
//! the premise that only a claim creates an object whose id is a key-derived
//! address. `object::new_uid_from_hash` mints a `UID` at any address it is
//! given. Every caller but `claim::claim_address` derives that address by
//! hashing; `claim_address` takes the sender's address. It is reached only
//! through `smart_account::claim_builder`, which only the private
//! `smart_account_builtin_auth::claim_account_v1` calls, and only the node's
//! `ClaimAccount` pipeline calls that. A new caller anywhere on this chain
//! would let other Move code mint a `UID` at a chosen address.

use std::collections::BTreeSet;

use iota_framework::BuiltInFramework;
use move_binary_format::{
    CompiledModule,
    file_format::{Bytecode, Visibility},
};
use move_core_types::account_address::AccountAddress;

#[test]
fn functions_that_mint_a_uid_at_a_chosen_address_have_the_expected_callers() {
    let modules = system_modules();

    assert_eq!(
        callers(&modules, "object", "new_uid_from_hash"),
        functions(&[
            ("claim", "claim_address"),
            ("dynamic_field", "add"),
            ("module_metadata", "new"),
            (
                "package_metadata",
                "build_package_metadata_v1_with_dynamic_metadata"
            ),
        ]),
        "the callers of `object::new_uid_from_hash` changed. A caller that passes an address \
         of its own caller's choosing, rather than one derived by hashing, would let a \
         transaction other than `ClaimAccount` create an object at a key-derived address"
    );
    assert_eq!(
        callers(&modules, "claim", "claim_address"),
        functions(&[("smart_account", "claim_builder")]),
        "`claim::claim_address` mints a `UID` at the sender's address, so only the claim's \
         account builder may call it"
    );
    assert_eq!(
        callers(&modules, "smart_account", "claim_builder"),
        functions(&[("smart_account_builtin_auth", "claim_account_v1")]),
        "`smart_account::claim_builder` builds an account at the sender's address, so only the \
         claim may call it"
    );
    assert!(
        callers(&modules, "smart_account_builtin_auth", "claim_account_v1").is_empty(),
        "only the node's `ClaimAccount` pipeline may call `claim_account_v1`"
    );
    let claim = definition(&modules, "smart_account_builtin_auth", "claim_account_v1");
    assert!(
        claim.visibility == Visibility::Private && !claim.is_entry,
        "`claim_account_v1` must stay private and not entry, so that no transaction but \
         `ClaimAccount` reaches it"
    );
}

/// The modules of every system package, as compiled.
fn system_modules() -> Vec<CompiledModule> {
    BuiltInFramework::iter_system_packages()
        .flat_map(|package| package.modules())
        .collect()
}

/// `(module, function)` pairs.
fn functions(names: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    names
        .iter()
        .map(|(module, function)| (module.to_string(), function.to_string()))
        .collect()
}

/// The functions of `modules` that call `module::function` of the IOTA
/// framework, as `(module, function)` pairs.
fn callers(modules: &[CompiledModule], module: &str, function: &str) -> BTreeSet<(String, String)> {
    let mut callers = BTreeSet::new();
    for compiled in modules {
        for definition in compiled.function_defs() {
            let Some(code) = &definition.code else {
                continue;
            };
            let calls = code.code.iter().any(|instruction| {
                let handle = match instruction {
                    Bytecode::Call(index) => *index,
                    Bytecode::CallGeneric(index) => {
                        compiled.function_instantiation_at(*index).handle
                    }
                    _ => return false,
                };
                let handle = compiled.function_handle_at(handle);
                let callee = compiled.module_handle_at(handle.module);
                *compiled.address_identifier_at(callee.address) == AccountAddress::TWO
                    && compiled.identifier_at(callee.name).as_str() == module
                    && compiled.identifier_at(handle.name).as_str() == function
            });
            if calls {
                let caller = compiled.function_handle_at(definition.function);
                callers.insert((
                    compiled.self_id().name().to_string(),
                    compiled.identifier_at(caller.name).to_string(),
                ));
            }
        }
    }
    callers
}

/// The definition of `module::function` in the IOTA framework.
fn definition<'a>(
    modules: &'a [CompiledModule],
    module: &str,
    function: &str,
) -> &'a move_binary_format::file_format::FunctionDefinition {
    modules
        .iter()
        .filter(|compiled| {
            *compiled.self_id().address() == AccountAddress::TWO
                && compiled.self_id().name().as_str() == module
        })
        .flat_map(|compiled| {
            compiled.function_defs().iter().filter(move |definition| {
                compiled
                    .identifier_at(compiled.function_handle_at(definition.function).name)
                    .as_str()
                    == function
            })
        })
        .next()
        .unwrap_or_else(|| panic!("`{module}::{function}` is defined in the IOTA framework"))
}
