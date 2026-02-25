// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub use iota_sdk_types::ActiveJwk;
use serde::{Deserialize, Serialize};

use crate::{
    base_types::{Identifier, ObjectID, SequenceNumber},
    dynamic_field::get_dynamic_field_from_store,
    error::{IotaError, IotaResult},
    id::UID,
    storage::ObjectStore,
};

pub const AUTHENTICATOR_STATE_UPDATE_FUNCTION_NAME: Identifier =
    Identifier::from_static("update_authenticator_state");
pub const AUTHENTICATOR_STATE_CREATE_FUNCTION_NAME: Identifier = Identifier::from_static("create");
pub const AUTHENTICATOR_STATE_EXPIRE_JWKS_FUNCTION_NAME: Identifier =
    Identifier::from_static("expire_jwks");

/// Current latest version of the authenticator state object.
pub const AUTHENTICATOR_STATE_VERSION: u64 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticatorState {
    pub id: UID,
    pub version: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthenticatorStateInner {
    pub version: u64,

    /// List of currently active JWKs.
    pub active_jwks: Vec<ActiveJwk>,
}

pub fn get_authenticator_state(
    object_store: impl ObjectStore,
) -> IotaResult<Option<AuthenticatorStateInner>> {
    let outer = object_store.try_get_object(&ObjectID::AUTHENTICATOR_STATE)?;
    let Some(outer) = outer else {
        return Ok(None);
    };
    let move_object = outer.data.try_as_move().ok_or_else(|| {
        IotaError::IotaSystemStateRead("AuthenticatorState object must be a Move object".to_owned())
    })?;
    let outer = bcs::from_bytes::<AuthenticatorState>(move_object.contents())
        .map_err(|err| IotaError::IotaSystemStateRead(err.to_string()))?;

    // No other versions exist yet.
    assert_eq!(outer.version, AUTHENTICATOR_STATE_VERSION);

    let id = outer.id.id.bytes;
    let inner: AuthenticatorStateInner =
        get_dynamic_field_from_store(&object_store, id, &outer.version).map_err(|err| {
            IotaError::DynamicFieldRead(format!(
                "Failed to load iota system state inner object with ID {:?} and version {:?}: {:?}",
                id, outer.version, err
            ))
        })?;

    Ok(Some(inner))
}

pub fn get_authenticator_state_obj_initial_shared_version(
    object_store: &dyn ObjectStore,
) -> IotaResult<Option<SequenceNumber>> {
    Ok(object_store
        .try_get_object(&ObjectID::AUTHENTICATOR_STATE)?
        .map(|obj| {
            obj.owner
                .into_shared_opt()
                .expect("Authenticator state object must be shared")
        }))
}
