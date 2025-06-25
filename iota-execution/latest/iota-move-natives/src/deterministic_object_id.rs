// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::VecDeque};

use iota_types::base_types::{ObjectID};
use move_binary_format::errors::PartialVMResult;
use move_core_types::{account_address::AccountAddress, gas_algebra::InternalGas};
use move_vm_runtime::{native_charge_gas_early_exit, native_functions::NativeContext};
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
};
use smallvec::smallvec;

use crate::{NativesCostTable, object_runtime::ObjectRuntime};

#[derive(Clone)]
pub struct DeterministicObjecIdDeriveIdWithSaltCostParams {
    pub deterministic_object_id_derive_id_with_salt_cost_base: InternalGas,
}


/// ****************************************************************************
/// ********************* native fun derive_id_with_salt
/// Implementation of the Move native function `fun derive_id_with_salt(flag: u64, iota_address: address, salt: vector<u8>): address`
/// gas cost: tx_context_derive_id_cost_base
/// we operate on fixed size data structures
/// ****************************************************************************
/// ***********************************
pub fn derive_id_with_salt(
    context: &mut NativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.is_empty());
    debug_assert!(args.len() == 3);

    let deterministic_object_id_derive_id_with_salt_cost_params = context
        .extensions_mut()
        .get::<NativesCostTable>()
        .deterministic_object_id_derive_id_with_salt_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        deterministic_object_id_derive_id_with_salt_cost_params.deterministic_object_id_derive_id_with_salt_cost_base
    );

    let salt = pop_arg!(args, Vec<u8>);
    let iota_address = pop_arg!(args, AccountAddress);
    let flag = pop_arg!(args, u64);

    let address = AccountAddress::from(ObjectID::derive_id_with_salt(
        flag,
        iota_address.into(),
        salt.as_slice()
    ).unwrap());
    
    let obj_runtime: &mut ObjectRuntime = context.extensions_mut().get_mut();
    obj_runtime.new_id(address.into())?;

    Ok(NativeResult::ok(
        context.gas_used(),
        smallvec![Value::address(address)],
    ))
}
