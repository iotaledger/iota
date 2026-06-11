// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use rand::{SeedableRng, rngs::StdRng};

use crate::{
    account_abstraction::public_key::MovePublicKey,
    base_types::IotaAddress,
    crypto::{IotaKeyPair, PublicKey, SignatureScheme, get_key_pair_from_rng},
    multisig::MultiSigPublicKey,
};

// === scheme() ===

#[test]
fn scheme_ed25519() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut rng).1);
    assert_eq!(
        MovePublicKey::from(&key_pair).scheme(),
        SignatureScheme::ED25519
    );
}

#[test]
fn scheme_secp256k1() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    assert_eq!(
        MovePublicKey::from(&key_pair).scheme(),
        SignatureScheme::Secp256k1
    );
}

#[test]
fn scheme_secp256r1() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut rng).1);
    assert_eq!(
        MovePublicKey::from(&key_pair).scheme(),
        SignatureScheme::Secp256r1
    );
}

#[test]
fn scheme_multisig() {
    let mut rng = seeded_rng();
    let key_pair1 = IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut rng).1);
    let key_pair2 = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    let multisig_public_key =
        MultiSigPublicKey::new(vec![key_pair1.public(), key_pair2.public()], vec![1, 1], 1)
            .unwrap();
    assert_eq!(
        MovePublicKey::new(
            SignatureScheme::MultiSig,
            bcs::to_bytes(&multisig_public_key).unwrap()
        )
        .unwrap()
        .scheme(),
        SignatureScheme::MultiSig
    );
}

#[test]
fn scheme_passkey() {
    // Passkey uses Secp256r1 key bytes under the passkey flag.
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut rng).1);
    assert_eq!(
        MovePublicKey::new(
            SignatureScheme::PasskeyAuthenticator,
            key_pair.public().as_ref().to_vec(),
        )
        .unwrap()
        .scheme(),
        SignatureScheme::PasskeyAuthenticator
    );
}

// === new() errors ===

#[test]
fn new_error_on_empty_bytes() {
    assert_eq!(
        MovePublicKey::new(SignatureScheme::ED25519, vec![])
            .unwrap_err()
            .to_string(),
        "Public key bytes are empty"
    );
}

#[test]
fn new_error_on_unsupported_scheme() {
    // BLS12381, ZkLoginAuthenticatorDeprecated, and MoveAuthenticator are
    // recognized by SignatureScheme but not valid for account public keys.
    for scheme in [
        SignatureScheme::BLS12381,
        #[allow(deprecated)]
        SignatureScheme::ZkLoginAuthenticatorDeprecated,
        SignatureScheme::MoveAuthenticator,
    ] {
        let err = MovePublicKey::new(scheme, vec![0x00])
            .unwrap_err()
            .to_string();
        assert!(
            err.starts_with("Unsupported signature scheme for account public key:"),
            "unexpected error for {scheme:?}: {err}"
        );
    }
}

#[test]
fn new_error_on_invalid_key_bytes() {
    // Valid ED25519 scheme but garbage raw bytes.
    let err = MovePublicKey::new(SignatureScheme::ED25519, vec![0x01, 0x02])
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("Invalid public key bytes:"),
        "unexpected error: {err}"
    );
}

// === address() ===

#[test]
fn address_ed25519_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut rng).1);
    let expected = IotaAddress::from(&key_pair.public());
    assert_eq!(MovePublicKey::from(&key_pair).address().unwrap(), expected);
}

#[test]
fn address_secp256k1_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    let expected = IotaAddress::from(&key_pair.public());
    assert_eq!(MovePublicKey::from(&key_pair).address().unwrap(), expected);
}

#[test]
fn address_secp256r1_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut rng).1);
    let expected = IotaAddress::from(&key_pair.public());
    assert_eq!(MovePublicKey::from(&key_pair).address().unwrap(), expected);
}

#[test]
fn address_multisig_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair1 = IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut rng).1);
    let key_pair2 = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    let multisig_public_key =
        MultiSigPublicKey::new(vec![key_pair1.public(), key_pair2.public()], vec![1, 1], 1)
            .unwrap();
    let expected = IotaAddress::from(&multisig_public_key);
    assert_eq!(
        MovePublicKey::new(
            SignatureScheme::MultiSig,
            bcs::to_bytes(&multisig_public_key).unwrap()
        )
        .unwrap()
        .address()
        .unwrap(),
        expected
    );
}

#[test]
fn address_passkey_matches_iota_address() {
    // Passkey uses Secp256r1 raw bytes but hashes with the Passkey flag (0x06),
    // so the address differs from the Secp256r1 address for the same key.
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut rng).1);
    let raw = key_pair.public().as_ref().to_vec();
    let passkey_public_key =
        PublicKey::try_from_bytes(SignatureScheme::PasskeyAuthenticator, &raw).unwrap();
    let expected = IotaAddress::from(&passkey_public_key);

    assert_eq!(
        MovePublicKey::new(SignatureScheme::PasskeyAuthenticator, raw)
            .unwrap()
            .address()
            .unwrap(),
        expected
    );
    // Sanity-check: passkey address is distinct from the Secp256r1 address.
    assert_ne!(expected, IotaAddress::from(&key_pair.public()));
}

#[test]
fn address_error_on_secp256k1_zero_bytes() {
    // 33 zero bytes have the correct length for Secp256k1 but are not a valid
    // compressed curve point (prefix 0x00 is neither 0x02 nor 0x03).
    let mut bcs_bytes = vec![SignatureScheme::Secp256k1.flag()];
    bcs_bytes.extend(bcs::to_bytes(&vec![0u8; 33]).unwrap());
    let invalid: MovePublicKey = bcs::from_bytes(&bcs_bytes).unwrap();

    let err = invalid.address().unwrap_err().to_string();
    assert!(
        err.contains("Invalid public key bytes"),
        "unexpected error: {err}"
    );
}

#[test]
fn address_error_on_wrong_length_bytes() {
    // Construct a MovePublicKey with 1 raw byte for ED25519 (requires 32) by
    // bypassing new() via BCS deserialization.
    let mut bcs_bytes = vec![SignatureScheme::ED25519.flag()];
    bcs_bytes.extend(bcs::to_bytes(&vec![0u8; 1]).unwrap());
    let invalid: MovePublicKey = bcs::from_bytes(&bcs_bytes).unwrap();

    let err = invalid.address().unwrap_err().to_string();
    assert!(
        err.contains("Invalid public key bytes"),
        "unexpected error: {err}"
    );
}

// === Cross-language pin tests (Move ↔ Rust address parity) ===

// Key material shared with the Move test vectors in public_key_tests.move and
// claim_registry_tests.move.
const ED25519_PK_HEX: &str =
    "cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
const SECP256K1_PK_HEX: &str =
    "02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";

#[test]
fn address_multisig_ed25519_only_matches_move_vector() {
    // 1-of-1 Ed25519 MultiSig. Ed25519 members have no scheme-flag prefix in
    // the hash input — the IOTA legacy rule mirrored by update_hasher_with_flag.
    let ed25519_pk =
        PublicKey::try_from_bytes(SignatureScheme::ED25519, &hex::decode(ED25519_PK_HEX).unwrap())
            .unwrap();
    let multisig_pk = MultiSigPublicKey::new(vec![ed25519_pk], vec![1], 1).unwrap();

    let addr = IotaAddress::from(&multisig_pk);
    let expected = IotaAddress::new(
        hex::decode("1cc23b51b2e3c8641eea35b29114a53ad7a76643dcb2763d12290a7b83cac525")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    assert_eq!(addr, expected);

    let move_pk =
        MovePublicKey::new(SignatureScheme::MultiSig, bcs::to_bytes(&multisig_pk).unwrap())
            .unwrap();
    assert_eq!(move_pk.address().unwrap(), expected);
}

#[test]
fn address_multisig_mixed_matches_move_vector() {
    // Mixed Ed25519 + Secp256k1 MultiSig: pins the per-scheme flag behaviour for
    // both member types (Ed25519 = no flag, Secp256k1 = flag 0x01).
    let ed25519_pk =
        PublicKey::try_from_bytes(SignatureScheme::ED25519, &hex::decode(ED25519_PK_HEX).unwrap())
            .unwrap();
    let secp256k1_pk = PublicKey::try_from_bytes(
        SignatureScheme::Secp256k1,
        &hex::decode(SECP256K1_PK_HEX).unwrap(),
    )
    .unwrap();
    let multisig_pk =
        MultiSigPublicKey::new(vec![ed25519_pk, secp256k1_pk], vec![1, 1], 1).unwrap();

    let addr = IotaAddress::from(&multisig_pk);
    let expected = IotaAddress::new(
        hex::decode("2e6c30799340fef9d382542ff0cad8e2a20f766da8b71a25c2443eda658104e4")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    assert_eq!(addr, expected);

    let move_pk =
        MovePublicKey::new(SignatureScheme::MultiSig, bcs::to_bytes(&multisig_pk).unwrap())
            .unwrap();
    assert_eq!(move_pk.address().unwrap(), expected);
}

// === Helpers ===

fn seeded_rng() -> StdRng {
    StdRng::from_seed([0; 32])
}
