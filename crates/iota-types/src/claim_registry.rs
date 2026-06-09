// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{ident_str, identifier::IdentStr};

use crate::{
    IOTA_CLAIM_REGISTRY_OBJECT_ID, base_types::SequenceNumber, error::IotaResult, object::Owner,
    storage::ObjectStore,
};

pub const CLAIM_REGISTRY_MODULE_NAME: &IdentStr = ident_str!("claim_registry");
pub const CLAIM_REGISTRY_CREATE_FUNCTION_NAME: &IdentStr = ident_str!("create");

/// Returns the `initial_shared_version` of the `ClaimRegistry` object if it
/// exists in the object store, or `None` if it has not yet been created.
pub fn get_claim_registry_obj_initial_shared_version(
    object_store: &dyn ObjectStore,
) -> IotaResult<Option<SequenceNumber>> {
    Ok(object_store
        .try_get_object(&IOTA_CLAIM_REGISTRY_OBJECT_ID)?
        .map(|obj| match obj.owner {
            Owner::Shared {
                initial_shared_version,
            } => initial_shared_version,
            _ => unreachable!("ClaimRegistry object must be shared"),
        }))
}
