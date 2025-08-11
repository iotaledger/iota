use std::collections::VecDeque;

use iota_types::Identifier;
use iota_verifier::account_auth_verifier;
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    account_address::AccountAddress, gas_algebra::InternalGas, vm_status::StatusCode,
};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    natives::function::NativeResult,
    pop_arg,
    values::{Value, VectorRef},
};
use smallvec::smallvec;

use crate::NativesCostTable;

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

    // loading module for context
    let compiled_module = context.load_module(module_identifier)?;

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
