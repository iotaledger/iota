// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rust mirrors of `iota_system::attestor_registry` plus helpers to read
//! the registry from the object store. The registry lives as a dynamic
//! field on the `IotaSystemState` wrapper object under
//! `AttestorRegistryKey`, so its object ID is deterministic.

use std::collections::HashMap;

use iota_sdk_types::{Address, Identifier, ObjectId, StructTag};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_SYSTEM_STATE_OBJECT_ID, MoveTypeTagTrait, TypeTag,
    balance::Balance,
    crypto::{PublicKey, SignatureScheme},
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
    pub attestor_address: Address,
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

/// Abort code for an invalid attestor public key; matches `EInvalidPubkey`
/// in `iota_system::attestor_registry`.
pub const E_INVALID_ATTESTOR_PUBKEY: u64 = 3;

/// Validate a `flag || raw_key` attestor signing key, accepting only the
/// plain schemes (ed25519 / secp256k1 / secp256r1). Backs the
/// `attestor_registry::validate_attestor_pubkey` native.
pub fn verify_attestor_pubkey(pubkey: &[u8]) -> Result<(), u64> {
    let Some((&flag, key_bytes)) = pubkey.split_first() else {
        return Err(E_INVALID_ATTESTOR_PUBKEY);
    };
    let scheme = SignatureScheme::from_flag_byte(&flag).map_err(|_| E_INVALID_ATTESTOR_PUBKEY)?;
    match scheme {
        SignatureScheme::ED25519 | SignatureScheme::Secp256k1 | SignatureScheme::Secp256r1 => {
            PublicKey::try_from_bytes(scheme, key_bytes)
                .map(|_| ())
                .map_err(|_| E_INVALID_ATTESTOR_PUBKEY)
        }
        _ => Err(E_INVALID_ATTESTOR_PUBKEY),
    }
}

/// Per-epoch snapshot entry for an active attestor, carried by
/// `EpochStartSystemStateV3`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EpochStartAttestorInfoV1 {
    pub attestor_address: Address,
    pub attestor_pubkey: Vec<u8>,
}

/// The active attestor set of one epoch, with committee-style lookups.
/// An attestor's index is its position in the underlying ordered set
/// (mirroring the Move `active_attestors` vector at epoch start).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttestorSet {
    epoch: u64,
    entries: Vec<EpochStartAttestorInfoV1>,
    index_by_address: HashMap<Address, u32>,
}

impl AttestorSet {
    pub fn new(epoch: u64, entries: Vec<EpochStartAttestorInfoV1>) -> Self {
        let index_by_address = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.attestor_address, i as u32))
            .collect();
        Self {
            epoch,
            entries,
            index_by_address,
        }
    }

    pub fn empty(epoch: u64) -> Self {
        Self::new(epoch, vec![])
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn by_index(&self, index: u32) -> Option<&EpochStartAttestorInfoV1> {
        self.entries.get(index as usize)
    }

    pub fn by_address(&self, address: &Address) -> Option<(u32, &EpochStartAttestorInfoV1)> {
        let idx = *self.index_by_address.get(address)?;
        Some((idx, &self.entries[idx as usize]))
    }

    pub fn iter(&self) -> impl Iterator<Item = &EpochStartAttestorInfoV1> {
        self.entries.iter()
    }
}

/// Deterministic object ID of the registry dynamic field on the system
/// state wrapper.
pub fn derive_attestor_registry_object_id() -> Result<ObjectId, bcs::Error> {
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

/// Read the registry and shape its active set for the epoch-start snapshot.
pub fn read_epoch_start_attestors(
    object_store: &dyn ObjectStore,
) -> Result<Vec<EpochStartAttestorInfoV1>, IotaError> {
    Ok(get_attestor_registry(object_store)?
        .active_attestors
        .into_iter()
        .map(|a| EpochStartAttestorInfoV1 {
            attestor_address: a.attestor_address,
            attestor_pubkey: a.attestor_pubkey,
        })
        .collect())
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
                attestor_address: Address::ZERO,
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

    use fastcrypto::{
        ed25519::Ed25519KeyPair, secp256k1::Secp256k1KeyPair, secp256r1::Secp256r1KeyPair,
    };
    use rand::{SeedableRng, rngs::StdRng};

    use crate::crypto::{IotaKeyPair, KeypairTraits};

    /// `flag || raw_key` encoding for a public key.
    fn flagged(pk: &PublicKey) -> Vec<u8> {
        let mut bytes = vec![pk.flag()];
        bytes.extend_from_slice(pk.as_ref());
        bytes
    }

    fn seeded_rng() -> StdRng {
        StdRng::from_seed([7u8; 32])
    }

    #[test]
    fn verify_attestor_pubkey_accepts_plain_schemes() {
        let mut rng = seeded_rng();
        let ed = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut rng)).public();
        let k1 = IotaKeyPair::Secp256k1(Secp256k1KeyPair::generate(&mut rng)).public();
        let r1 = IotaKeyPair::Secp256r1(Secp256r1KeyPair::generate(&mut rng)).public();
        assert!(verify_attestor_pubkey(&flagged(&ed)).is_ok());
        assert!(verify_attestor_pubkey(&flagged(&k1)).is_ok());
        assert!(verify_attestor_pubkey(&flagged(&r1)).is_ok());
    }

    #[test]
    fn verify_attestor_pubkey_rejects_bad_keys() {
        let mut rng = seeded_rng();
        let ed = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut rng)).public();
        let valid = flagged(&ed);

        // empty
        assert_eq!(verify_attestor_pubkey(&[]), Err(E_INVALID_ATTESTOR_PUBKEY));
        // valid key bytes but wrong/truncated length
        assert_eq!(
            verify_attestor_pubkey(&valid[..valid.len() - 1]),
            Err(E_INVALID_ATTESTOR_PUBKEY)
        );
        // not a valid curve point (right length, garbage bytes)
        let mut garbage = vec![0u8];
        garbage.extend(std::iter::repeat(0xAB).take(32));
        assert_eq!(verify_attestor_pubkey(&garbage), Err(E_INVALID_ATTESTOR_PUBKEY));
        // non-plain scheme flags: multisig (3), bls (4), zklogin (5), passkey (6)
        for flag in [3u8, 4, 5, 6] {
            let mut k = vec![flag];
            k.extend_from_slice(ed.as_ref());
            assert_eq!(verify_attestor_pubkey(&k), Err(E_INVALID_ATTESTOR_PUBKEY));
        }
    }

    #[test]
    fn attestor_set_lookups_agree() {
        let entries = vec![
            EpochStartAttestorInfoV1 {
                attestor_address: Address::from_bytes([1u8; 32]).unwrap(),
                attestor_pubkey: vec![0u8; 33],
            },
            EpochStartAttestorInfoV1 {
                attestor_address: Address::from_bytes([2u8; 32]).unwrap(),
                attestor_pubkey: vec![1u8; 34],
            },
        ];
        let set = AttestorSet::new(9, entries.clone());
        assert_eq!(set.epoch(), 9);
        assert_eq!(set.len(), 2);
        let (idx, entry) = set.by_address(&entries[1].attestor_address).unwrap();
        assert_eq!(idx, 1);
        assert_eq!(entry.attestor_pubkey, entries[1].attestor_pubkey);
        assert_eq!(
            set.by_index(0).unwrap().attestor_address,
            entries[0].attestor_address
        );
        assert!(set.by_index(2).is_none());
        let empty = AttestorSet::empty(3);
        assert!(empty.is_empty());
        assert_eq!(empty.epoch(), 3);
    }
}
