// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    base_types::{Identifier, IotaAddress},
    error::ExecutionError,
};
use move_binary_format::{
    CompiledModule,
    file_format::{
        Bytecode, FunctionDefinition, FunctionHandle, FunctionInstantiation, ModuleHandle,
        SignatureToken,
    },
};
use move_bytecode_utils::format_signature_token;
use move_core_types::identifier::IdentStr;

use crate::{TEST_SCENARIO_MODULE_NAME, verification_failure};

pub const TRANSFER_MODULE: Identifier = Identifier::from_static("transfer");
pub const ACCOUNT_MODULE: Identifier = Identifier::from_static("account");
pub const EVENT_MODULE: Identifier = Identifier::from_static("event");
pub const EVENT_FUNCTION: Identifier = Identifier::from_static("emit");
pub const GET_EVENTS_TEST_FUNCTION: Identifier = Identifier::from_static("events_by_type");
pub const PUBLIC_TRANSFER_FUNCTIONS: [Identifier; 5] = [
    Identifier::from_static("public_transfer"),
    Identifier::from_static("public_freeze_object"),
    Identifier::from_static("public_share_object"),
    Identifier::from_static("public_receive"),
    Identifier::from_static("receiving_object_id"),
];
pub const PRIVATE_TRANSFER_FUNCTIONS: [Identifier; 4] = [
    Identifier::from_static("transfer"),
    Identifier::from_static("freeze_object"),
    Identifier::from_static("share_object"),
    Identifier::from_static("receive"),
];
pub const TRANSFER_IMPL_FUNCTIONS: [Identifier; 4] = [
    Identifier::from_static("transfer_impl"),
    Identifier::from_static("freeze_object_impl"),
    Identifier::from_static("share_object_impl"),
    Identifier::from_static("receive_impl"),
];

pub const PUBLIC_ACCOUNT_FUNCTIONS: [Identifier; 2] = [
    Identifier::from_static("borrow_auth_function_ref_v1"),
    Identifier::from_static("has_auth_function_ref_v1"),
];
pub const PRIVATE_ACCOUNT_FUNCTIONS: [Identifier; 3] = [
    Identifier::from_static("create_account_v1"),
    Identifier::from_static("create_immutable_account_v1"),
    Identifier::from_static("rotate_auth_function_ref_v1"),
];

/// All transfer functions (the functions in `iota::transfer`) are "private" in
/// that they are restricted to the module.
/// For example, with `transfer::transfer<T>(...)`, either:
/// - `T` must be a type declared in the current module or
/// - `T` must have `store`
///
/// Similarly, `event::emit` is also "private" to the module. Unlike the
/// `transfer` functions, there is no relaxation for `store`
/// Concretely, with `event::emit<T>(...)`:
/// - `T` must be a type declared in the current module
pub fn verify_module(module: &CompiledModule) -> Result<(), ExecutionError> {
    if module.address().as_ref() == IotaAddress::FRAMEWORK.as_bytes()
        && module.name() == IdentStr::new(TEST_SCENARIO_MODULE_NAME).unwrap()
    {
        // exclude test_module which is a test-only module in the IOTA framework which
        // "emulates" transactional execution and needs to allow test code to
        // bypass private generics
        return Ok(());
    }
    // do not need to check the iota::transfer module itself
    for func_def in &module.function_defs {
        verify_function(module, func_def).map_err(|error| {
            verification_failure(format!(
                "{}::{}. {}",
                module.self_id(),
                module.identifier_at(module.function_handle_at(func_def.function).name),
                error
            ))
        })?;
    }
    Ok(())
}

fn verify_function(view: &CompiledModule, fdef: &FunctionDefinition) -> Result<(), String> {
    let code = match &fdef.code {
        None => return Ok(()),
        Some(code) => code,
    };
    for instr in &code.code {
        if let Bytecode::CallGeneric(finst_idx) = instr {
            let FunctionInstantiation {
                handle,
                type_parameters,
            } = view.function_instantiation_at(*finst_idx);

            let fhandle = view.function_handle_at(*handle);
            let mhandle = view.module_handle_at(fhandle.module);

            let type_arguments = &view.signature_at(*type_parameters).0;
            let ident = addr_module(view, mhandle);
            if ident == (IotaAddress::FRAMEWORK, TRANSFER_MODULE) {
                verify_private_transfer_module_functions(view, fhandle, type_arguments)?
            } else if ident == (IotaAddress::FRAMEWORK, EVENT_MODULE) {
                verify_private_event_emit(view, fhandle, type_arguments)?
            } else if ident == (IotaAddress::FRAMEWORK, ACCOUNT_MODULE) {
                verify_private_account_module_functions(view, fhandle, type_arguments)?
            }
        }
    }
    Ok(())
}

fn verify_private_transfer_module_functions(
    view: &CompiledModule,
    fhandle: &FunctionHandle,
    type_arguments: &[SignatureToken],
) -> Result<(), String> {
    let self_handle = view.module_handle_at(view.self_handle_idx());
    if addr_module(view, self_handle) == (IotaAddress::FRAMEWORK, TRANSFER_MODULE) {
        return Ok(());
    }
    let fident = Identifier::new_unchecked(view.identifier_at(fhandle.name).as_str());
    // public transfer functions require `store` and have no additional rules
    if PUBLIC_TRANSFER_FUNCTIONS.contains(&fident) {
        return Ok(());
    }
    if !PRIVATE_TRANSFER_FUNCTIONS.contains(&fident) {
        // unknown function, so a bug in the implementation here
        debug_assert!(false, "unknown transfer function {fident}");
        return Err(format!("Calling unknown transfer function, {fident}"));
    };

    if type_arguments.len() != 1 {
        debug_assert!(false, "Expected 1 type argument for {fident}");
        return Err(format!("Expected 1 type argument for {fident}"));
    }

    let type_arg = &type_arguments[0];
    if !is_defined_in_current_module(view, type_arg) {
        return Err(format!(
            "Invalid call to '{iota}::transfer::{f}' on an object of type '{t}'. \
            The transferred object's type must be defined in the current module. \
            If the object has the 'store' type ability, you can use the non-internal variant \
            instead, i.e. '{iota}::transfer::public_{f}'",
            iota = IotaAddress::FRAMEWORK,
            f = fident,
            t = format_signature_token(view, type_arg),
        ));
    }

    Ok(())
}

fn verify_private_account_module_functions(
    view: &CompiledModule,
    fhandle: &FunctionHandle,
    type_arguments: &[SignatureToken],
) -> Result<(), String> {
    let self_handle = view.module_handle_at(view.self_handle_idx());
    if addr_module(view, self_handle) == (IotaAddress::FRAMEWORK, ACCOUNT_MODULE) {
        return Ok(());
    }
    let fident = Identifier::new_unchecked(view.identifier_at(fhandle.name).as_str());
    // public account functions have no additional rules
    if PUBLIC_ACCOUNT_FUNCTIONS.contains(&fident) {
        return Ok(());
    }
    if !PRIVATE_ACCOUNT_FUNCTIONS.contains(&fident) {
        // unknown function, so a bug in the implementation here
        debug_assert!(false, "unknown account function {fident}");
        return Err(format!("Calling unknown account function, {fident}"));
    };

    if type_arguments.len() != 1 {
        debug_assert!(false, "Expected 1 type argument for {fident}");
        return Err(format!("Expected 1 type argument for {fident}"));
    }

    let type_arg = &type_arguments[0];
    if !is_defined_in_current_module(view, type_arg) {
        return Err(format!(
            "Invalid call to '{iota}::{account}::{f}' on an object of type '{t}'. \
            The account object's type must be defined in the current module.",
            iota = IotaAddress::FRAMEWORK,
            account = ACCOUNT_MODULE,
            f = fident,
            t = format_signature_token(view, type_arg),
        ));
    }

    Ok(())
}

fn verify_private_event_emit(
    view: &CompiledModule,
    fhandle: &FunctionHandle,
    type_arguments: &[SignatureToken],
) -> Result<(), String> {
    let fident = view.identifier_at(fhandle.name);
    if fident.as_str() == GET_EVENTS_TEST_FUNCTION.as_str() {
        // test-only function with no params--no need to verify
        return Ok(());
    }
    if fident.as_str() != EVENT_FUNCTION.as_str() {
        debug_assert!(false, "unknown event function {fident}");
        return Err(format!("Calling unknown event function, {fident}"));
    };

    if type_arguments.len() != 1 {
        debug_assert!(false, "Expected 1 type argument for {fident}");
        return Err(format!("Expected 1 type argument for {fident}"));
    }

    let type_arg = &type_arguments[0];
    if !is_defined_in_current_module(view, type_arg) {
        return Err(format!(
            "Invalid call to '{}::event::{}' with an event type '{}'. \
                The event's type must be defined in the current module",
            IotaAddress::FRAMEWORK,
            fident,
            format_signature_token(view, type_arg),
        ));
    }

    Ok(())
}

fn is_defined_in_current_module(view: &CompiledModule, type_arg: &SignatureToken) -> bool {
    match type_arg {
        SignatureToken::Datatype(_) | SignatureToken::DatatypeInstantiation(_) => {
            let idx = match type_arg {
                SignatureToken::Datatype(idx) => *idx,
                SignatureToken::DatatypeInstantiation(s) => s.0,
                _ => unreachable!(),
            };
            let shandle = view.datatype_handle_at(idx);
            view.self_handle_idx() == shandle.module
        }
        SignatureToken::TypeParameter(_)
        | SignatureToken::Bool
        | SignatureToken::U8
        | SignatureToken::U16
        | SignatureToken::U32
        | SignatureToken::U64
        | SignatureToken::U128
        | SignatureToken::U256
        | SignatureToken::Address
        | SignatureToken::Vector(_)
        | SignatureToken::Signer
        | SignatureToken::Reference(_)
        | SignatureToken::MutableReference(_) => false,
    }
}

fn addr_module(view: &CompiledModule, mhandle: &ModuleHandle) -> (IotaAddress, Identifier) {
    let maddr = view.address_identifier_at(mhandle.address);
    let mident = view.identifier_at(mhandle.name);
    (
        IotaAddress::new(maddr.into_bytes()),
        Identifier::new(mident.as_str()).unwrap(),
    )
}
