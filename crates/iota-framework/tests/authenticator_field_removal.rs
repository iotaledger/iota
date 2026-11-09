// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Pins that an account's authenticator function field is never removed for
//! good, read from the compiled IOTA framework, so that an aliased import or a
//! reformatted signature cannot hide a removal.
//!
//! Post-consensus validation records a claim of an address by that field
//! rather than by the account object: the object can be deleted once its
//! `UID` moves to another type, while the field stays attached to the `UID`
//! and, after the `UID` is deleted, stays behind orphaned. A way to remove the
//! field would let the address be claimed again.

use std::collections::BTreeSet;

use iota_framework::BuiltInFramework;
use iota_sdk_types::ObjectId;
use move_binary_format::{
    CompiledModule,
    file_format::{Bytecode, SignatureToken, Visibility},
};
use move_core_types::account_address::AccountAddress;

const KEY: &str = "AuthenticatorFunctionRefV1Key";

#[test]
fn authenticator_field_is_only_removed_to_be_added_back() {
    let modules = BuiltInFramework::get_package_by_id(&ObjectId::FRAMEWORK).modules();
    let account = modules
        .iter()
        .find(|compiled| compiled.self_id().name().as_str() == "account")
        .expect("the IOTA framework has the `account` module");

    // Only `iota::account` can build the field's key, so only it can remove
    // the field.
    let removing: BTreeSet<String> = ["remove", "remove_if_exists"]
        .into_iter()
        .flat_map(|remove| functions_calling(account, "dynamic_field", remove))
        .collect();
    assert_eq!(
        removing,
        BTreeSet::from(["rotate_auth_function_ref_v1".to_string()]),
        "`iota::account` must remove the authenticator function field only inside the rotation"
    );
    assert!(
        functions_calling(account, "dynamic_field", "add").contains("rotate_auth_function_ref_v1"),
        "the rotation must add the authenticator function field back after removing it"
    );

    // No key value leaves the module: every function that takes or returns
    // the key is private.
    for definition in account.function_defs() {
        let handle = account.function_handle_at(definition.function);
        let name = account.identifier_at(handle.name);
        let mentions_key = [handle.parameters, handle.return_]
            .into_iter()
            .flat_map(|signature| &account.signature_at(signature).0)
            .any(|token| mentions(account, token, KEY));
        assert!(
            !mentions_key || definition.visibility == Visibility::Private,
            "`account::{name}` takes or returns the key, so it must be private"
        );
    }
}

/// The functions of `compiled` that call `module::function` of the IOTA
/// framework.
fn functions_calling(compiled: &CompiledModule, module: &str, function: &str) -> BTreeSet<String> {
    compiled
        .function_defs()
        .iter()
        .filter(|definition| {
            definition.code.as_ref().is_some_and(|code| {
                code.code.iter().any(|instruction| {
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
                })
            })
        })
        .map(|definition| {
            compiled
                .identifier_at(compiled.function_handle_at(definition.function).name)
                .to_string()
        })
        .collect()
}

/// Whether `token` names the datatype `name` of `compiled`, at any depth.
fn mentions(compiled: &CompiledModule, token: &SignatureToken, name: &str) -> bool {
    token.preorder_traversal().any(|token| match token {
        SignatureToken::Datatype(index) => {
            compiled
                .identifier_at(compiled.datatype_handle_at(*index).name)
                .as_str()
                == name
        }
        SignatureToken::DatatypeInstantiation(instantiation) => {
            compiled
                .identifier_at(compiled.datatype_handle_at(instantiation.0).name)
                .as_str()
                == name
        }
        _ => false,
    })
}
