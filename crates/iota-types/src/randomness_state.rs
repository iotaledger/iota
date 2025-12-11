// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_2::types::{Address, ObjectId};
use move_binary_format::{CompiledModule, file_format::SignatureToken};
use move_bytecode_utils::resolve_struct;
use move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr};

use crate::{
    base_types::Version,
    error::{IotaError, IotaResult},
    object::Owner,
    storage::ObjectStore,
};

pub const RANDOMNESS_MODULE_NAME: &IdentStr = ident_str!("random");
pub const RANDOMNESS_STATE_STRUCT_NAME: &IdentStr = ident_str!("Random");
pub const RANDOMNESS_STATE_UPDATE_FUNCTION_NAME: &IdentStr = ident_str!("update_randomness_state");
pub const RANDOMNESS_STATE_CREATE_FUNCTION_NAME: &IdentStr = ident_str!("create");
pub const RESOLVED_IOTA_RANDOMNESS_STATE: (&AccountAddress, &IdentStr, &IdentStr) = (
    &AccountAddress::new(Address::FRAMEWORK.into_bytes()),
    RANDOMNESS_MODULE_NAME,
    RANDOMNESS_STATE_STRUCT_NAME,
);

pub fn get_randomness_state_obj_initial_shared_version(
    object_store: &dyn ObjectStore,
) -> IotaResult<Version> {
    object_store
        .try_get_object(&ObjectId::RANDOMNESS_STATE)?
        .map(|obj| match obj.owner {
            Owner::Shared {
                initial_shared_version,
            } => initial_shared_version,
            _ => unreachable!("Randomness state object must be shared"),
        })
        .ok_or(IotaError::Storage(
            "Randomness state object not found".to_string(),
        ))
}

pub fn is_mutable_random(view: &CompiledModule, s: &SignatureToken) -> bool {
    match s {
        SignatureToken::MutableReference(inner) => is_mutable_random(view, inner),
        SignatureToken::Datatype(idx) => {
            resolve_struct(view, *idx) == RESOLVED_IOTA_RANDOMNESS_STATE
        }
        _ => false,
    }
}
