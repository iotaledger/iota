// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_crypto::{ToFromBytes, ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey};
use iota_sdk_types::Address;
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    account_abstraction::public_key::MovePublicKey,
    crypto::{IotaKeyPair, PublicKey, SignatureScheme, get_key_pair_from_rng},
    multisig::{MultiSigPublicKey, MultisigMember},
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
    let kp1 = Ed25519PrivateKey::from_bytes(key_pair1.to_bytes_no_flag()).unwrap();
    let kp2 = Secp256k1PrivateKey::from_bytes(key_pair2.to_bytes_no_flag()).unwrap();
    let multisig_public_key = MultiSigPublicKey::new(
        vec![
            MultisigMember::new(kp1.public_key(), 1),
            MultisigMember::new(kp2.public_key(), 1),
        ],
        1,
    )
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
    let expected = Address::from(&key_pair.public());
    assert_eq!(MovePublicKey::from(&key_pair).address().unwrap(), expected);
}

#[test]
fn address_secp256k1_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    let expected = Address::from(&key_pair.public());
    assert_eq!(MovePublicKey::from(&key_pair).address().unwrap(), expected);
}

#[test]
fn address_secp256r1_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair = IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut rng).1);
    let expected = Address::from(&key_pair.public());
    assert_eq!(MovePublicKey::from(&key_pair).address().unwrap(), expected);
}

#[test]
fn address_multisig_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair1 = IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut rng).1);
    let key_pair2 = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rng).1);
    let kp1 = Ed25519PrivateKey::from_bytes(key_pair1.to_bytes_no_flag()).unwrap();
    let kp2 = Secp256k1PrivateKey::from_bytes(key_pair2.to_bytes_no_flag()).unwrap();
    let multisig_public_key = MultiSigPublicKey::new(
        vec![
            MultisigMember::new(kp1.public_key(), 1),
            MultisigMember::new(kp2.public_key(), 1),
        ],
        1,
    )
    .unwrap();
    let expected = Address::from(&multisig_public_key);
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
    let expected = Address::from(&passkey_public_key);

    assert_eq!(
        MovePublicKey::new(SignatureScheme::PasskeyAuthenticator, raw)
            .unwrap()
            .address()
            .unwrap(),
        expected
    );
    // Sanity-check: passkey address is distinct from the Secp256r1 address.
    assert_ne!(expected, Address::from(&key_pair.public()));
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

// === Helpers ===

fn seeded_rng() -> StdRng {
    StdRng::from_seed([0; 32])
}
