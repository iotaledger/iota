// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Rust mirrors of `iota_system::attestor_registry` plus helpers to read
//! the registry from the object store. The registry lives as a dynamic
//! field on the `IotaSystemState` wrapper object under
//! `AttestorRegistryKey`, so its object ID is deterministic.

use std::collections::HashMap;

use fastcrypto::{
    ed25519::{Ed25519PublicKey, Ed25519Signature},
    hash::HashFunction,
    secp256k1::{Secp256k1PublicKey, Secp256k1Signature},
    secp256r1::{Secp256r1PublicKey, Secp256r1Signature},
};
use iota_sdk_types::{
    Address, Identifier, ObjectId, StructTag,
    crypto::{Intent, IntentMessage, IntentScope},
};
use serde::{Deserialize, Serialize};

use crate::{
    IOTA_SYSTEM_STATE_OBJECT_ID, MoveTypeTagTrait, TypeTag,
    balance::Balance,
    crypto::{
        DefaultHash, IotaKeyPair, IotaSignature, PublicKey, Signature, SignatureScheme,
        ToFromBytes, VerifyingKey,
    },
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
    /// At-stake part of the escrow, capped at the joining bond.
    pub bond: Balance,
    /// Escrow above the joining bond; folded into `bond` at epoch
    /// boundaries.
    pub excess_bond: Balance,
    pub activation_epoch: u64,
    pub last_active_epoch: u64,
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
    let scheme = SignatureScheme::from_byte(flag).map_err(|_| E_INVALID_ATTESTOR_PUBKEY)?;
    match scheme {
        SignatureScheme::Ed25519 | SignatureScheme::Secp256k1 | SignatureScheme::Secp256r1 => {
            PublicKey::try_from_bytes(scheme, key_bytes)
                .map(|_| ())
                .map_err(|_| E_INVALID_ATTESTOR_PUBKEY)
        }
        _ => Err(E_INVALID_ATTESTOR_PUBKEY),
    }
}

/// Abort code for a proof-of-possession that does not verify; matches
/// `EInvalidProofOfPossession` in `iota_system::attestor_registry`.
pub const E_INVALID_PROOF_OF_POSSESSION: u64 = 8;

fn attestor_pop_digest(pubkey: &[u8], sender: Address) -> [u8; 32] {
    let mut msg = pubkey.to_vec();
    msg.extend_from_slice(sender.as_ref());
    let intent_msg = IntentMessage::new(Intent::iota_app(IntentScope::ProofOfPossession), msg);
    let mut hasher = DefaultHash::default();
    hasher.update(bcs::to_bytes(&intent_msg).expect("BCS serialization of bytes cannot fail"));
    hasher.finalize().digest
}

/// Verify a raw-signature proof of possession for an attestor signing key.
/// The signed payload is
/// `bcs(IntentMessage(ProofOfPossession, pubkey || sender))`, mirroring the
/// validator proof of possession. Expects `pubkey` to have already passed
/// [`verify_attestor_pubkey`].
pub fn verify_attestor_pop(pubkey: &[u8], pop: &[u8], sender: Address) -> Result<(), u64> {
    let Some((&flag, raw_key)) = pubkey.split_first() else {
        return Err(E_INVALID_PROOF_OF_POSSESSION);
    };
    let scheme = SignatureScheme::from_byte(flag).map_err(|_| E_INVALID_PROOF_OF_POSSESSION)?;
    let digest = attestor_pop_digest(pubkey, sender);
    let verified = match scheme {
        SignatureScheme::Ed25519 => {
            let pk = Ed25519PublicKey::from_bytes(raw_key);
            let sig = Ed25519Signature::from_bytes(pop);
            matches!((pk, sig), (Ok(pk), Ok(sig)) if pk.verify(&digest, &sig).is_ok())
        }
        SignatureScheme::Secp256k1 => {
            let pk = Secp256k1PublicKey::from_bytes(raw_key);
            let sig = Secp256k1Signature::from_bytes(pop);
            matches!((pk, sig), (Ok(pk), Ok(sig)) if pk.verify(&digest, &sig).is_ok())
        }
        SignatureScheme::Secp256r1 => {
            let pk = Secp256r1PublicKey::from_bytes(raw_key);
            let sig = Secp256r1Signature::from_bytes(pop);
            matches!((pk, sig), (Ok(pk), Ok(sig)) if pk.verify(&digest, &sig).is_ok())
        }
        _ => false,
    };
    if verified {
        Ok(())
    } else {
        Err(E_INVALID_PROOF_OF_POSSESSION)
    }
}

/// Generate the raw-signature proof of possession accepted by
/// `iota_system::register_attestor` / `rotate_attestor_key` for `keypair`'s
/// public key bound to `sender`.
pub fn generate_attestor_proof_of_possession(keypair: &IotaKeyPair, sender: Address) -> Vec<u8> {
    let pk = keypair.public();
    let mut pubkey = vec![pk.flag()];
    pubkey.extend_from_slice(pk.as_ref());
    let mut msg = pubkey;
    msg.extend_from_slice(sender.as_ref());
    let intent_msg = IntentMessage::new(Intent::iota_app(IntentScope::ProofOfPossession), msg);
    let sig = Signature::new_secure(&intent_msg, keypair);
    // Strip the composite `flag || sig || pubkey` down to the raw signature.
    sig.to_bytes()[1..65].to_vec()
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

/// Mirror of `iota_system::attestor_registry::AttestorMetadataKey`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AttestorMetadataKey {
    pub attestor_address: Address,
}

impl MoveTypeTagTrait for AttestorMetadataKey {
    fn get_type_tag() -> TypeTag {
        TypeTag::Struct(Box::new(StructTag::new(
            Address::SYSTEM,
            Identifier::new("attestor_registry").unwrap(),
            Identifier::new("AttestorMetadataKey").unwrap(),
            vec![],
        )))
    }
}

/// Mirror of `iota_system::attestor_registry::AttestorMetadataV1`.
/// `url`/`logo` are Move `Url`s, which serialize as their inner string.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AttestorMetadataV1 {
    pub name: String,
    pub description: String,
    pub url: String,
    pub logo: String,
}

/// Deterministic object id of an attestor's metadata dynamic field on the
/// system state object.
pub fn derive_attestor_metadata_object_id(
    attestor_address: Address,
) -> Result<ObjectId, IotaError> {
    derive_dynamic_field_id(
        IOTA_SYSTEM_STATE_OBJECT_ID,
        &AttestorMetadataKey::get_type_tag(),
        &bcs::to_bytes(&AttestorMetadataKey { attestor_address })
            .map_err(|e| IotaError::DynamicFieldRead(e.to_string()))?,
    )
    .map_err(|e| IotaError::DynamicFieldRead(e.to_string()))
}

/// Read an attestor's metadata; `None` if the address has no entry.
pub fn get_attestor_metadata(
    object_store: &dyn ObjectStore,
    attestor_address: Address,
) -> Result<Option<AttestorMetadataV1>, IotaError> {
    let id = derive_attestor_metadata_object_id(attestor_address)?;
    if object_store.get_object(&id).is_none() {
        return Ok(None);
    }
    get_dynamic_field_from_store(
        object_store,
        IOTA_SYSTEM_STATE_OBJECT_ID,
        &AttestorMetadataKey { attestor_address },
    )
    .map(Some)
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
                excess_bond: Balance::new(500_000_000_000),
                activation_epoch: 7,
                last_active_epoch: 9,
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

    #[test]
    fn attestor_metadata_key_type_tag_and_layout() {
        let key = AttestorMetadataKey {
            attestor_address: Address::from_short_hex("0xA1").unwrap(),
        };
        // Deterministic id derivation must not error for a well-formed key.
        derive_attestor_metadata_object_id(key.attestor_address).unwrap();
        // BCS layout: 32-byte address, nothing else.
        assert_eq!(bcs::to_bytes(&key).unwrap().len(), 32);
    }

    use fastcrypto::{
        ed25519::Ed25519KeyPair, secp256k1::Secp256k1KeyPair, secp256r1::Secp256r1KeyPair,
    };
    use rand::{SeedableRng, rngs::StdRng};

    use crate::crypto::{IotaKeyPair, KeypairTraits, get_key_pair_from_rng};

    /// `flag || raw_key` encoding for a public key.
    fn flagged(pk: &PublicKey) -> Vec<u8> {
        let mut bytes = vec![pk.flag()];
        bytes.extend_from_slice(pk.as_ref());
        bytes
    }

    fn seeded_rng() -> StdRng {
        StdRng::from_seed([7u8; 32])
    }

    fn test_keypairs() -> Vec<IotaKeyPair> {
        let mut rng = StdRng::from_seed([42; 32]);
        vec![
            IotaKeyPair::Ed25519(get_key_pair_from_rng::<Ed25519KeyPair, _>(&mut rng).1),
            IotaKeyPair::Secp256k1(get_key_pair_from_rng::<Secp256k1KeyPair, _>(&mut rng).1),
            IotaKeyPair::Secp256r1(get_key_pair_from_rng::<Secp256r1KeyPair, _>(&mut rng).1),
        ]
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
        garbage.extend(std::iter::repeat_n(0xAB, 32));
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

    #[test]
    fn pop_roundtrip_all_plain_schemes() {
        let sender = Address::from_short_hex("0xA1").unwrap();
        for kp in test_keypairs() {
            let pubkey = flagged(&kp.public());
            let pop = generate_attestor_proof_of_possession(&kp, sender);
            assert_eq!(pop.len(), 64);
            verify_attestor_pubkey(&pubkey).unwrap();
            verify_attestor_pop(&pubkey, &pop, sender).unwrap();
        }
    }

    #[test]
    fn pop_rejects_wrong_sender() {
        let sender = Address::from_short_hex("0xA1").unwrap();
        let other = Address::from_short_hex("0xA2").unwrap();
        for kp in test_keypairs() {
            let pubkey = flagged(&kp.public());
            let pop = generate_attestor_proof_of_possession(&kp, sender);
            assert_eq!(
                verify_attestor_pop(&pubkey, &pop, other),
                Err(E_INVALID_PROOF_OF_POSSESSION)
            );
        }
    }

    #[test]
    fn pop_rejects_wrong_key_and_garbage() {
        let sender = Address::from_short_hex("0xA1").unwrap();
        let kps = test_keypairs();
        for i in 0..kps.len() {
            let pubkey = flagged(&kps[i].public());
            let other_kp = &kps[(i + 1) % kps.len()];
            let pop_other_key = generate_attestor_proof_of_possession(other_kp, sender);
            assert_eq!(
                verify_attestor_pop(&pubkey, &pop_other_key, sender),
                Err(E_INVALID_PROOF_OF_POSSESSION)
            );
            assert_eq!(
                verify_attestor_pop(&pubkey, &[0u8; 64], sender),
                Err(E_INVALID_PROOF_OF_POSSESSION)
            );
            assert_eq!(
                verify_attestor_pop(&pubkey, &[], sender),
                Err(E_INVALID_PROOF_OF_POSSESSION)
            );
        }
    }

    /// Prints the Move test fixtures. The keypairs are derived from the
    /// fixed seed in `test_keypairs`, so the printed values are stable;
    /// run with:
    /// `cargo nextest run -p iota-types --lib print_attestor_move_fixtures
    /// --no-capture`
    #[test]
    fn print_attestor_move_fixtures() {
        let senders = [
            ("A1", Address::from_short_hex("0xA1").unwrap()),
            ("A2", Address::from_short_hex("0xA2").unwrap()),
            ("A3", Address::from_short_hex("0xA3").unwrap()),
        ];
        for kp in test_keypairs() {
            let pubkey = flagged(&kp.public());
            println!(
                "// scheme flag {}: x\"{}\"",
                pubkey[0],
                hex::encode(&pubkey)
            );
            for (name, sender) in senders {
                let pop = generate_attestor_proof_of_possession(&kp, sender);
                println!("//   pop for @0x{name}: x\"{}\"", hex::encode(pop));
            }
        }
        let scenario_sender = Address::from_short_hex("0x42").unwrap();
        let kp = &test_keypairs()[0];
        println!(
            "// scenario pop (ed25519, @0x42): x\"{}\"",
            hex::encode(generate_attestor_proof_of_possession(kp, scenario_sender))
        );
        let secp256k1_kp = &test_keypairs()[1];
        println!(
            "// scenario pop (secp256k1, @0x42): x\"{}\"",
            hex::encode(generate_attestor_proof_of_possession(
                secp256k1_kp,
                scenario_sender
            ))
        );
    }
}
