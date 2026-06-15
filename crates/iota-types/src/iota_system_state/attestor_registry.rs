// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rust mirrors of `iota_system::attestor_registry` plus helpers to read
//! the registry from the object store. The registry lives as a dynamic
//! field on the `IotaSystemState` wrapper object under
//! `AttestorRegistryKey`, so its object ID is deterministic.

use iota_sdk_types::{Address, Identifier, StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_SYSTEM_STATE_OBJECT_ID, MoveTypeTagTrait, TypeTag,
    balance::Balance,
    base_types::{IotaAddress, ObjectID},
    dynamic_field::{derive_dynamic_field_id, get_dynamic_field_from_store},
    error::IotaError,
    storage::ObjectStore,
};

/// Mirror of `iota_system::attestor_registry::AttestorRegistryKey`. Move
/// compiles fieldless structs with a hidden `dummy_field: bool`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AttestorRegistryKey {
    pub dummy_field: bool,
}

impl MoveTypeTagTrait for AttestorRegistryKey {
    fn get_type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(StructTag::new(
            Address::SYSTEM,
            Identifier::new("attestor_registry").unwrap(),
            Identifier::new("AttestorRegistryKey").unwrap(),
            vec![],
        )))
    }
}

/// Mirror of `iota_system::attestor_registry::AttestorV1`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AttestorV1 {
    pub attestor_address: IotaAddress,
    /// flag byte || raw pubkey bytes (plain schemes only).
    pub attestor_pubkey: Vec<u8>,
    pub next_epoch_attestor_pubkey: Option<Vec<u8>>,
    pub bond: Balance,
    pub activation_epoch: u64,
}

/// Mirror of `iota_system::attestor_registry::AttestorRegistryV1`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AttestorRegistryV1 {
    /// Ordered active set; an attestor's per-epoch index is its position.
    pub active_attestors: Vec<AttestorV1>,
    pub pending_active: Vec<AttestorV1>,
    pub pending_removals: Vec<u64>,
}

/// Deterministic object ID of the registry dynamic field on the system
/// state wrapper.
pub fn derive_attestor_registry_object_id() -> Result<ObjectID, bcs::Error> {
    derive_dynamic_field_id(
        IOTA_SYSTEM_STATE_OBJECT_ID,
        &AttestorRegistryKey::get_type_tag(),
        &bcs::to_bytes(&AttestorRegistryKey::default())?,
    )
}

/// Read the attestor registry. The registry is created lazily on-chain, so
/// absence is normal and yields the empty registry.
pub fn get_attestor_registry(
    object_store: &dyn ObjectStore,
) -> Result<AttestorRegistryV1, IotaError> {
    let id = derive_attestor_registry_object_id()
        .map_err(|e| IotaError::DynamicFieldRead(e.to_string()))?;
    if object_store.get_object(&id).is_none() {
        return Ok(AttestorRegistryV1::default());
    }
    get_dynamic_field_from_store(
        object_store,
        IOTA_SYSTEM_STATE_OBJECT_ID,
        &AttestorRegistryKey::default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attestor_registry_key_bcs_matches_move_empty_struct() {
        // Move adds a hidden `dummy_field: bool = false` to fieldless
        // structs; BCS must be exactly [0x00].
        assert_eq!(bcs::to_bytes(&AttestorRegistryKey::default()).unwrap(), vec![0u8]);
    }

    #[test]
    fn attestor_registry_v1_bcs_round_trip() {
        let registry = AttestorRegistryV1 {
            active_attestors: vec![AttestorV1 {
                attestor_address: IotaAddress::ZERO,
                attestor_pubkey: vec![0u8; 33],
                next_epoch_attestor_pubkey: None,
                bond: Balance::new(2_000_000_000_000),
                activation_epoch: 7,
            }],
            pending_active: vec![],
            pending_removals: vec![3, 1],
        };
        let bytes = bcs::to_bytes(&registry).unwrap();
        let decoded: AttestorRegistryV1 = bcs::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, registry);
    }

    #[test]
    fn registry_object_id_is_deterministic() {
        let a = derive_attestor_registry_object_id().unwrap();
        let b = derive_attestor_registry_object_id().unwrap();
        assert_eq!(a, b);
    }
}
