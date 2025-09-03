use std::collections::VecDeque;

use iota_types::{Identifier, base_types::ObjectID};
use iota_verifier::account_auth_verifier;
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    account_address::AccountAddress, gas_algebra::InternalGas, language_storage::ModuleId,
    vm_status::StatusCode,
};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{Value, VectorRef},
};
use smallvec::smallvec;

use crate::{NativesCostTable, raw_module_loader::RawModuleLoader};

#[derive(Copy, Clone, Debug)]
pub struct CreateAuthInfoV1ImplCostParams {
    pub create_auth_info_v1_cost_base: InternalGas,
}

pub fn create_auth_info_v1_impl(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 3);

    // charge gas
    let account_create_auth_info_v1_impl_params = context
        .extensions_mut()
        .get::<NativesCostTable>()
        .account_create_auth_info_v1_impl_params;
    native_charge_gas_early_exit!(
        context,
        account_create_auth_info_v1_impl_params.create_auth_info_v1_cost_base
    );

    let function_name_bytes = pop_arg!(args, VectorRef);
    let function_name = String::from(unsafe {
        std::str::from_utf8_unchecked(function_name_bytes.as_bytes_ref().as_slice())
    });
    let function_identifier = Identifier::new(function_name.clone()).unwrap();

    let module_name_bytes = pop_arg!(args, VectorRef);
    let module_name = String::from(unsafe {
        std::str::from_utf8_unchecked(module_name_bytes.as_bytes_ref().as_slice())
    });
    let module_identifier = Identifier::new(module_name.clone()).unwrap();

    let package = pop_arg!(args, AccountAddress);
    let package_id = ObjectID::from(package);

    // Loading module for context verifying the referenced `authenticate` function.
    // There are two base cases when looking for an `authenticate` function. The
    // `authenticate` function is either in the current module (which is not handled
    // by this function as the user cannot write such a requirement down at the
    // moment) or it must be loaded. Either because it is in a completely
    // different package or its for this package, but a different version.
    let raw_module_loader = &context.extensions().get::<RawModuleLoader>();
    let Some(compiled_module) = raw_module_loader.get_module(&package_id, &module_identifier)
    else {
        return Err(
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR).with_message(
                format!(
                    "Referenced module:
    {package}::{module_name} unavailable"
                ),
            ),
        );
    };

    if let Err(execution_error) =
        account_auth_verifier::verify_authenticate_func(&compiled_module, function_identifier)
    {
        return Err(
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message(execution_error.to_string()),
        );
    }

    let authenticator_info_v1 = Value::struct_(move_vm_types::values::Struct::pack([
        Value::address(package),
        Value::vector_u8(module_name.as_bytes().iter().copied()),
        Value::vector_u8(function_name.as_bytes().iter().copied()),
    ]));

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![authenticator_info_v1],
    ))
}

#[derive(Copy, Clone, Debug)]
pub struct CreateAuthInfoSelfV1ImplCostParams {
    pub create_auth_info_self_v1_cost_base: InternalGas,
}

pub fn create_auth_info_self_v1_impl(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 2);

    // charge gas
    let account_create_auth_info_self_v1_impl_params = context
        .extensions_mut()
        .get::<NativesCostTable>()
        .account_create_auth_info_self_v1_impl_params;
    native_charge_gas_early_exit!(
        context,
        account_create_auth_info_self_v1_impl_params.create_auth_info_self_v1_cost_base
    );

    let function_name_bytes = pop_arg!(args, VectorRef);
    let function_name = String::from(unsafe {
        std::str::from_utf8_unchecked(function_name_bytes.as_bytes_ref().as_slice())
    });
    let function_identifier = Identifier::new(function_name.clone()).unwrap();

    let module_name_bytes = pop_arg!(args, VectorRef);
    let module_name = String::from(unsafe {
        std::str::from_utf8_unchecked(module_name_bytes.as_bytes_ref().as_slice())
    });
    let module_identifier = Identifier::new(module_name.clone()).unwrap();

    // We are looking for the stack frame from which `create_auth_info_self_v1` was
    // called from. Which has to be the last one, as native functions do not create
    // a stack frame.
    // This stack frame also has to exist, as `create_auth_info_self_v1` can't be
    // called directly.
    let relevant_frames = context.stack_frames(1);
    let previous_frame = &relevant_frames.stack_trace()[0];

    let callsite_module_id = &previous_frame.0;
    let code_module_id = ModuleId::new(*callsite_module_id.address(), module_identifier);

    let data_store = context.data_store();
    let compiled_module = context.get_module(data_store.link_context(), &code_module_id);

    if let Err(execution_error) =
        account_auth_verifier::verify_authenticate_func(&compiled_module, function_identifier)
    {
        return Err(
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message(execution_error.to_string()),
        );
    }

    let storage_id = data_store.relocate(&code_module_id).unwrap();

    let authenticator_info_v1 = Value::struct_(move_vm_types::values::Struct::pack([
        Value::address(*storage_id.address()),
        Value::vector_u8(module_name.as_bytes().iter().copied()),
        Value::vector_u8(function_name.as_bytes().iter().copied()),
    ]));

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![authenticator_info_v1],
    ))
}
