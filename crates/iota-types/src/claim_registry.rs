// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::hash::HashFunction;
use iota_sdk_types::{Address, Identifier, ObjectId, Owner};

use crate::{
    base_types::SequenceNumber, crypto::DefaultHash, error::IotaResult, storage::ObjectStore,
};

pub const CLAIM_REGISTRY_CREATE_FUNCTION_NAME: Identifier = Identifier::from_static("create");

/// Returns the `initial_shared_version` of the `ClaimRegistry` object if it
/// exists in the object store, or `None` if it has not yet been created.
pub fn get_claim_registry_obj_initial_shared_version(
    object_store: &dyn ObjectStore,
) -> IotaResult<Option<SequenceNumber>> {
    Ok(object_store
        .try_get_object(&ObjectId::CLAIM_REGISTRY)?
        .map(|obj| match obj.owner {
            Owner::Shared(initial_shared_version) => initial_shared_version,
            _ => unreachable!("ClaimRegistry object must be shared"),
        }))
}

/// Returns the canonical identity hash of a public key:
/// `Blake2b256(scheme_flag || raw_key_bytes)`.
///
/// `raw_key_bytes` is the key material **without** the scheme flag prefix.
///
/// This mirrors `iota::public_key::key_id` in the Move framework: the flag byte
/// is part of the hash input for every scheme, Ed25519 included, so a `key_id`
/// is not the same thing as an address. It is the identity under which the
/// account-discoverability events index a key, and the two implementations must
/// not drift.
pub fn key_id(scheme_flag: u8, raw_key_bytes: &[u8]) -> Address {
    let mut hasher = DefaultHash::default();
    hasher.update([scheme_flag]);
    hasher.update(raw_key_bytes);
    Address::new(hasher.finalize().digest)
}

/// Returns the [`key_id`] of a flag-prefixed public key (`flag || raw_bytes`),
/// the wire format accepted by `iota::public_key::from_prefixed_bytes`.
///
/// Returns `None` if `prefixed_bytes` is empty and therefore carries no flag.
pub fn key_id_from_prefixed_bytes(prefixed_bytes: &[u8]) -> Option<Address> {
    let (flag, raw_bytes) = prefixed_bytes.split_first()?;
    Some(key_id(*flag, raw_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The same flag-prefixed test vectors, and the same expected key ids, as
    // `iota::public_key_tests::key_id_vectors` in the Move framework. Pinning
    // both sides against one literal table is what keeps the two
    // implementations from drifting apart.
    const KEY_ID_VECTORS: &[(&str, &str)] = &[
        (
            "00cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88",
            "0x43541042c153e0e498a08a8db868f1614c9366694fa730bd8a07fc5d7c931f0d",
        ),
        (
            "0102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c",
            "0x2fecbdf2652b089c64d127158d388621fdbbd156533fbcca5a0082aa0d2939fa",
        ),
        (
            "020227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a",
            "0x318f591092f10b67a81963954fb9539ea3919444417726be4e1b95ce44fe2fc0",
        ),
        (
            "060227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a",
            "0xa2f90cd2552d45ab5ba157dacf19597e2018108c6a80e4d7a4a5680d1542a7e8",
        ),
        (
            "030100cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010100",
            "0x37330e88388d526046696b5b5113cd64e81eb1b1bcd403372666cc54970ddbf4",
        ),
        (
            "030200cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88010102337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c010100",
            "0xdd22eb5c98cdc27de98174a69b68ca1603bdda8aeb226c5232273cfdc9655811",
        ),
    ];

    #[test]
    fn key_id_matches_move_vectors() {
        for (prefixed_hex, expected_hex) in KEY_ID_VECTORS {
            let prefixed = hex::decode(prefixed_hex).unwrap();
            let expected = Address::from_hex(expected_hex).unwrap();

            assert_eq!(key_id_from_prefixed_bytes(&prefixed).unwrap(), expected);
            assert_eq!(key_id(prefixed[0], &prefixed[1..]), expected);
        }
    }

    #[test]
    fn key_id_covers_the_scheme_flag() {
        // Secp256r1 (0x02) and Passkey (0x06) share the same raw key material,
        // so only the flag can tell their key ids apart.
        let raw = hex::decode("0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a")
            .unwrap();

        assert_ne!(key_id(0x02, &raw), key_id(0x06, &raw));
    }

    #[test]
    fn key_id_from_prefixed_bytes_rejects_empty_input() {
        assert!(key_id_from_prefixed_bytes(&[]).is_none());
    }
}
