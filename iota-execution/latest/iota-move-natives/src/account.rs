// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_types::base_types::MoveObjectType;
use move_binary_format::errors::PartialVMResult;
use move_core_types::{account_address::AccountAddress, vm_status::StatusCode};
use move_vm_runtime::native_functions::NativeContext;
use move_vm_types::{
    loaded_data::runtime_types::Type, natives::function::NativeResult, pop_arg, values::Value,
};
use smallvec::smallvec;

use crate::object_runtime::{ObjectRuntime, account_assets_store::AssetResult};

const E_ASSET_DOES_NOT_EXIST: u64 = 1;
const E_ASSET_TYPE_MISMATCH: u64 = 2;
const E_BCS_SERIALIZATION_FAILURE: u64 = 3;

macro_rules! get_or_fetch_object {
    ($context:ident, $ty_args:ident, $account_address:ident, $asset_address:ident/*, $ty_cost_per_byte:expr*/) => {{
        let child_ty = $ty_args.pop().unwrap();

        // TODO: Uncomment when the asset cost params are available.

        // native_charge_gas_early_exit!(
        //     $context,
        //     $ty_cost_per_byte * u64::from(child_ty.size()).into()
        // );

        assert!($ty_args.is_empty());
        let (tag, layout, annotated_layout) = match crate::get_tag_and_layouts($context, &child_ty)?
        {
            Some(res) => res,
            None => {
                return Ok(NativeResult::err(
                    $context.gas_used(),
                    E_BCS_SERIALIZATION_FAILURE,
                ));
            }
        };

        let object_runtime: &mut ObjectRuntime = $context.extensions_mut().get_mut();
        object_runtime.get_or_fetch_account_asset(
            $account_address,
            $asset_address,
            &child_ty,
            &layout,
            &annotated_layout,
            MoveObjectType::from(tag),
        )?
    }};
}

/// ****************************************************************************
/// native fun account::borrow_asset
/// Implementation of the Move native function:
/// native fun borrow_asset<Value: key>(account: address, asset: address):
/// &Value; ********************************************************************
/// ********
pub fn borrow_asset(
    context: &mut NativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    assert!(ty_args.len() == 1);
    assert!(args.len() == 2);

    // TODO: Uncomment when the asset cost params are available.

    // let address_asset_borrow_child_object_cost_params = context
    //     .extensions_mut()
    //     .get::<NativesCostTable>()
    //     .address_asset_borrow_child_object_cost_params
    //     .clone();
    // native_charge_gas_early_exit!(
    //     context,
    //     address_asset_borrow_child_object_cost_params.
    // address_asset_borrow_child_object_cost_base );

    let asset_address = pop_arg!(args, AccountAddress).into();
    let account_address = pop_arg!(args, AccountAddress).into();

    assert!(args.is_empty());

    let global_value_result = get_or_fetch_object!(
        context,
        ty_args,
        account_address,
        asset_address /* address_asset_borrow_child_object_cost_params
                       *     .address_asset_borrow_child_object_type_cost_per_byte */
    );
    let global_value = match global_value_result {
        AssetResult::MismatchedType => {
            return Ok(NativeResult::err(context.gas_used(), E_ASSET_TYPE_MISMATCH));
        }
        AssetResult::Loaded(gv) => gv,
    };
    if !global_value.exists()? {
        return Ok(NativeResult::err(
            context.gas_used(),
            E_ASSET_DOES_NOT_EXIST,
        ));
    }
    let child_ref = global_value.borrow_global().inspect_err(|err| {
        assert!(err.major_status() != StatusCode::MISSING_DATA);
    })?;

    // TODO: Uncomment when the asset cost params are available.

    // native_charge_gas_early_exit!(
    //     context,
    //     address_asset_borrow_child_object_cost_params
    //         .address_asset_borrow_child_object_child_ref_cost_per_byte
    //         * u64::from(child_ref.legacy_size()).into()
    // );

    Ok(NativeResult::ok(context.gas_used(), smallvec![child_ref]))
}
