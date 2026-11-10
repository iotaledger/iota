// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;

use iota_sdk_move_types::iota_framework::account::AuthenticatorFunctionRefV1Key;
use iota_sdk_types::{Address, MoveObjectType, ObjectId, Owner, StructTag, Version};
use iota_types::{
    dynamic_field::derive_dynamic_field_id, iota_sdk_types_conversions::struct_tag_core_to_sdk,
};
use move_binary_format::{
    errors::{PartialVMError, PartialVMResult},
    safe_assert, safe_unwrap,
};
use move_core_types::{
    account_address::AccountAddress, gas_algebra::InternalGas, language_storage::TypeTag,
    vm_status::StatusCode,
};
use move_vm_runtime::{
    execution::{Type, values::Value},
    native_charge_gas_early_exit,
    natives::functions::{NativeContext, NativeResult},
    pop_arg,
};
use smallvec::smallvec;

use super::object_runtime::{ObjectRuntime, TransferResult};
use crate::{
    NativesCostTable, get_extension, get_extension_mut, get_receiver_object_id,
    get_tag_and_layouts,
    object_runtime::object_store::ObjectResult,
    sdk_conv::{AccountAddressExt, move_object_type},
};

const E_SHARED_NON_NEW_OBJECT: u64 = 0;
const E_BCS_SERIALIZATION_FAILURE: u64 = 1;
const E_RECEIVING_OBJECT_TYPE_MISMATCH: u64 = 2;
// Represents both the case where the object does not exist and the case where
// the object is not able to be accessed through the parent that is passed-in.
const E_UNABLE_TO_RECEIVE_OBJECT: u64 = 3;
// Represents the case where it is trying to receive an object owned by an account.
const E_ACCOUNT_CANNOT_RECEIVE_OBJECT: u64 = 5;

#[derive(Clone, Debug)]
pub struct TransferReceiveObjectInternalCostParams {
    pub transfer_receive_object_internal_cost_base: InternalGas,
}

/// ****************************************************************************
/// ********************* native fun receive_object_internal
/// Implementation of the Move native function `receive_object_internal<T:
/// key>(parent: &mut UID, rec: Receiver<T>): T`   gas cost:
/// transfer_receive_object_internal_cost_base |  covers various fixed costs in
/// the oper *******************************************************************
/// ****************************
pub fn receive_object_internal(
    context: &mut NativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(args.len() == 3);
    let transfer_receive_object_internal_cost_params = get_extension!(context, NativesCostTable)?
        .transfer_receive_object_internal_cost_params
        .clone();
    native_charge_gas_early_exit!(
        context,
        transfer_receive_object_internal_cost_params.transfer_receive_object_internal_cost_base
    );
    let child_ty = safe_unwrap!(ty_args.pop());
    let child_receiver_sequence_number: Version = pop_arg!(args, u64).into();
    let child_receiver_object_id = safe_unwrap!(args.pop_back());
    let parent = pop_arg!(args, AccountAddress).to_object_id();
    safe_assert!(args.is_empty());
    let child_id: ObjectId = safe_unwrap!(
        get_receiver_object_id(child_receiver_object_id.copy_value())
            .and_then(|v| v.value_as::<AccountAddress>())
    )
    .to_object_id();
    safe_assert!(ty_args.is_empty());

    let Some((tag, layout, annotated_layout)) = get_tag_and_layouts(context, &child_ty)? else {
        return Ok(NativeResult::err(
            context.gas_used(),
            E_BCS_SERIALIZATION_FAILURE,
        ));
    };
    let tag = struct_tag_core_to_sdk(&tag);

    let object_runtime: &mut ObjectRuntime = get_extension_mut!(context)?;
    if object_runtime.protocol_config.enable_move_authentication() {
        // With Move authentication, an account object (a parent with an authenticator function
        // ref as a dynamic field) cannot receive objects.
        let key_bytes = bcs::to_bytes(&AuthenticatorFunctionRefV1Key::default()).map_err(|e| {
            PartialVMError::new(StatusCode::VALUE_SERIALIZATION_ERROR)
                .with_message(format!("Failed to serialize the authenticator key: {e}"))
        })?;
        let authenticator_fun_ref_id = derive_dynamic_field_id(
            parent,
            &StructTag::new_authenticator_function_ref_v1_key().into(),
            &key_bytes,
        )
        .map_err(|e| {
            PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                .with_message(format!("Failed to derive the authenticator field ID: {e}"))
        })?;
        let (_cache_info, is_account) =
            object_runtime.child_object_exists(parent, authenticator_fun_ref_id)?;
        if is_account {
            return Ok(NativeResult::err(
                context.gas_used(),
                E_ACCOUNT_CANNOT_RECEIVE_OBJECT,
            ));
        }
    }
    let (_cache_info, child) = match object_runtime.receive_object(
        parent,
        child_id,
        child_receiver_sequence_number,
        &layout,
        &annotated_layout,
        MoveObjectType::new(tag),
    ) {
        // NB: Loaded and doesn't exist and inauthenticated read should lead to the exact same error
        Ok(None) => {
            return Ok(NativeResult::err(
                context.gas_used(),
                E_UNABLE_TO_RECEIVE_OBJECT,
            ));
        }
        Ok(Some(ObjectResult::Loaded(gv))) => gv,
        Ok(Some(ObjectResult::MismatchedType)) => {
            return Ok(NativeResult::err(
                context.gas_used(),
                E_RECEIVING_OBJECT_TYPE_MISMATCH,
            ));
        }
        Err(x) => return Err(x),
    };

    Ok(NativeResult::ok(context.gas_used(), smallvec![child]))
}

#[derive(Clone, Debug)]
pub struct TransferInternalCostParams {
    pub transfer_transfer_internal_cost_base: InternalGas,
}
/// ****************************************************************************
/// ********************* native fun transfer_impl
/// Implementation of the Move native function `transfer_impl<T: key>(obj: T,
/// recipient: address)`   gas cost: transfer_transfer_internal_cost_base
/// |  covers various fixed costs in the oper **********************************
/// *************************************************************
pub fn transfer_internal(
    context: &mut NativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(args.len() == 2);

    let transfer_transfer_internal_cost_params = get_extension!(context, NativesCostTable)?
        .transfer_transfer_internal_cost_params
        .clone();

    native_charge_gas_early_exit!(
        context,
        transfer_transfer_internal_cost_params.transfer_transfer_internal_cost_base
    );

    let ty = safe_unwrap!(ty_args.pop());
    let recipient = pop_arg!(args, AccountAddress);
    let obj = safe_unwrap!(args.pop_back());

    let owner = Owner::Address(Address::new(recipient.into_bytes()));
    object_runtime_transfer(context, owner, ty, obj)?;
    // TODO check permissions for transfer
    let cost = context.gas_used();
    Ok(NativeResult::ok(cost, smallvec![]))
}

#[derive(Clone, Debug)]
pub struct TransferFreezeObjectCostParams {
    pub transfer_freeze_object_cost_base: InternalGas,
}
/// ****************************************************************************
/// ********************* native fun freeze_object
/// Implementation of the Move native function `freeze_object<T: key>(obj: T)`
///   gas cost: transfer_freeze_object_cost_base                  |  covers
/// various fixed costs in the oper ********************************************
/// ***************************************************
pub fn freeze_object(
    context: &mut NativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(args.len() == 1);

    let transfer_freeze_object_cost_params = get_extension!(context, NativesCostTable)?
        .transfer_freeze_object_cost_params
        .clone();

    native_charge_gas_early_exit!(
        context,
        transfer_freeze_object_cost_params.transfer_freeze_object_cost_base
    );

    let ty = safe_unwrap!(ty_args.pop());
    let obj = safe_unwrap!(args.pop_back());

    object_runtime_transfer(context, Owner::Immutable, ty, obj)?;
    // TODO check permissions for transfer
    Ok(NativeResult::ok(context.gas_used(), smallvec![]))
}

#[derive(Clone, Debug)]
pub struct TransferShareObjectCostParams {
    pub transfer_share_object_cost_base: InternalGas,
}
/// ****************************************************************************
/// ********************* native fun share_object
/// Implementation of the Move native function `share_object<T: key>(obj: T)`
///   gas cost: transfer_share_object_cost_base                  |  covers
/// various fixed costs in the oper ********************************************
/// ***************************************************
pub fn share_object(
    context: &mut NativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(args.len() == 1);

    let transfer_share_object_cost_params = get_extension!(context, NativesCostTable)?
        .transfer_share_object_cost_params
        .clone();

    native_charge_gas_early_exit!(
        context,
        transfer_share_object_cost_params.transfer_share_object_cost_base
    );

    let ty = safe_unwrap!(ty_args.pop());
    let obj = safe_unwrap!(args.pop_back());
    let transfer_result = object_runtime_transfer(
        context,
        // Dummy version, to be filled with the correct initial version when the effects of the
        // transaction are written to storage.
        Owner::Shared(Default::default()),
        ty,
        obj,
    )?;
    // TODO check permissions for transfer
    let cost = context.gas_used();
    Ok(match transfer_result {
        // New means the ID was created in this transaction
        // SameOwner means the object was previously shared and was re-shared
        TransferResult::New | TransferResult::SameOwner => NativeResult::ok(cost, smallvec![]),
        TransferResult::OwnerChanged => NativeResult::err(cost, E_SHARED_NON_NEW_OBJECT),
    })
}

fn object_runtime_transfer(
    context: &mut NativeContext,
    owner: Owner,
    ty: Type,
    obj: Value,
) -> PartialVMResult<TransferResult> {
    let object_type = match context.type_to_type_tag(&ty)? {
        TypeTag::Struct(s) => move_object_type(*s),
        _ => {
            return Err(
                PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
                    .with_message("IOTA verifier guarantees this is a struct".to_string()),
            );
        }
    };

    let obj_runtime: &mut ObjectRuntime = get_extension_mut!(context)?;
    obj_runtime.transfer(owner, object_type, obj, /* end of transaction */ false)
}
