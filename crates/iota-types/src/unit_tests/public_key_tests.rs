// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::{
    hash::{HashFunction, Sha256},
    rsa::{Base64UrlUnpadded, Encoding as _},
};
use iota_protocol_config::ProtocolConfig;
use iota_sdk_crypto::{
    Signer, ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey,
    secp256r1::Secp256r1PrivateKey, simple::SimpleKeypair,
};
use iota_sdk_types::{
    Address, Ed25519PublicKey, MoveAuthenticator, MoveAuthenticatorV1, ObjectDigest, ObjectId,
    ObjectReference, Secp256k1PublicKey, Secp256r1PublicKey, SharedObjectReference,
    SignatureScheme, SimpleSignature, Transaction, TypeTag, UserSignature, Version,
    crypto::{
        Intent, IntentMessage, MultisigAggregatedSignature, MultisigCommittee, MultisigMember,
        PasskeyAuthenticator, PersonalMessage,
    },
};
use rand::{SeedableRng, rngs::StdRng};

use crate::{
    crypto::PublicKey,
    error::IotaError,
    public_key::{
        MovePublicKey, verify_builtin_authenticator_signature, verify_personal_message_signature,
    },
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI},
};

// === scheme() ===

#[test]
fn scheme_ed25519() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    assert_eq!(
        MovePublicKey::from(&key_pair).scheme(),
        SignatureScheme::Ed25519
    );
}

#[test]
fn scheme_secp256k1() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Secp256k1PrivateKey::random_with(&mut rng));
    assert_eq!(
        MovePublicKey::from(&key_pair).scheme(),
        SignatureScheme::Secp256k1
    );
}

#[test]
fn scheme_secp256r1() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(&mut rng));
    assert_eq!(
        MovePublicKey::from(&key_pair).scheme(),
        SignatureScheme::Secp256r1
    );
}

#[test]
fn scheme_multisig() {
    let mut rng = seeded_rng();
    let kp1 = Secp256k1PrivateKey::random_with(&mut rng);
    let kp2 = Secp256r1PrivateKey::random_with(&mut rng);
    let multisig_public_key = MultisigCommittee::new(
        vec![
            MultisigMember::new(kp1.public_key(), 1),
            MultisigMember::new(kp2.public_key(), 1),
        ],
        1,
    )
    .unwrap();
    assert_eq!(
        MovePublicKey::new(
            SignatureScheme::Multisig,
            bcs::to_bytes(&multisig_public_key).unwrap()
        )
        .unwrap()
        .scheme(),
        SignatureScheme::Multisig
    );
}

#[test]
fn scheme_passkey() {
    // Passkey uses Secp256r1 key bytes under the passkey flag.
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(&mut rng));
    assert_eq!(
        MovePublicKey::new(
            SignatureScheme::PasskeyAuthenticator,
            key_pair.public_key().as_ref().to_vec(),
        )
        .unwrap()
        .scheme(),
        SignatureScheme::PasskeyAuthenticator
    );
}

// === new() ===

#[test]
fn new_accepts_multisig_member_not_on_curve() {
    // A compressed secp256k1 key must start with 0x02 or 0x03, so these bytes
    // are the right length but off the curve. Member keys are not checked
    // against their curves, as in the SDK's committee validation.
    let committee = MultisigCommittee::new_unchecked(
        vec![MultisigMember::new(Secp256k1PublicKey::new([0xff; 33]), 1)],
        1,
    );
    assert!(
        MovePublicKey::new(
            SignatureScheme::Multisig,
            bcs::to_bytes(&committee).unwrap()
        )
        .is_ok()
    );
}

// === new() errors ===

#[test]
fn new_error_on_empty_bytes() {
    assert_eq!(
        MovePublicKey::new(SignatureScheme::Ed25519, vec![])
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
        SignatureScheme::Bls12381,
        #[allow(deprecated)]
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
    let err = MovePublicKey::new(SignatureScheme::Ed25519, vec![0x01, 0x02])
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("Invalid public key bytes:"),
        "unexpected error: {err}"
    );
}

#[test]
fn new_error_on_multisig_zero_threshold() {
    let committee = MultisigCommittee::new_unchecked(multisig_members(), 0);
    assert_eq!(
        multisig_error(committee),
        "Invalid MultisigCommittee: Multisig threshold must be non-zero"
    );
}

#[test]
fn new_error_on_multisig_empty_committee() {
    let committee = MultisigCommittee::new_unchecked(vec![], 1);
    assert_eq!(
        multisig_error(committee),
        "Invalid MultisigCommittee: Multisig committee must have at least one member"
    );
}

#[test]
fn new_error_on_multisig_duplicate_member() {
    let mut members = multisig_members();
    members[1] = members[0].clone();
    let committee = MultisigCommittee::new_unchecked(members, 1);
    assert_eq!(
        multisig_error(committee),
        "Invalid MultisigCommittee: Duplicate public key"
    );
}

#[test]
fn new_error_on_multisig_weight_below_threshold() {
    // Two members of weight 1 each, so a threshold of 3 cannot be met.
    let committee = MultisigCommittee::new_unchecked(multisig_members(), 3);
    assert_eq!(
        multisig_error(committee),
        "Invalid MultisigCommittee: Insufficient total weight 2 for threshold 3"
    );
}

#[test]
fn new_error_on_multisig_trailing_bytes() {
    let committee = MultisigCommittee::new(multisig_members(), 1).unwrap();
    let mut bytes = bcs::to_bytes(&committee).unwrap();
    bytes.push(0x00);

    let err = MovePublicKey::new(SignatureScheme::Multisig, bytes)
        .unwrap_err()
        .to_string();
    assert_eq!(err, "Invalid MultisigCommittee: remaining input");
}

#[test]
fn new_error_on_multisig_mixed_ed25519() {
    // Ed25519 members carry no scheme flag in the address preimage, so two
    // different committees mixing Ed25519 with other key schemes can derive
    // the same address. The SDK's committee validation rejects such committees.
    let ed25519_pk =
        Ed25519PublicKey::new(hex::decode(ED25519_PK_HEX).unwrap().try_into().unwrap());
    let secp256k1_pk =
        Secp256k1PublicKey::new(hex::decode(SECP256K1_PK_HEX).unwrap().try_into().unwrap());
    let committee = MultisigCommittee::new_unchecked(
        vec![
            MultisigMember::new(ed25519_pk, 1),
            MultisigMember::new(secp256k1_pk, 1),
        ],
        1,
    );
    assert_eq!(
        multisig_error(committee),
        "Invalid MultisigCommittee: Multisig committee mixes Ed25519 with other key schemes"
    );
}

// === address() ===

#[test]
fn address_ed25519_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let expected = key_pair.public_key().derive_address();
    assert_eq!(MovePublicKey::from(&key_pair).address().unwrap(), expected);
}

#[test]
fn address_secp256k1_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Secp256k1PrivateKey::random_with(&mut rng));
    let expected = key_pair.public_key().derive_address();
    assert_eq!(MovePublicKey::from(&key_pair).address().unwrap(), expected);
}

#[test]
fn address_secp256r1_matches_iota_address() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(&mut rng));
    let expected = key_pair.public_key().derive_address();
    assert_eq!(MovePublicKey::from(&key_pair).address().unwrap(), expected);
}

#[test]
fn address_multisig_matches_iota_address() {
    let mut rng = seeded_rng();
    let kp1 = Secp256k1PrivateKey::random_with(&mut rng);
    let kp2 = Secp256r1PrivateKey::random_with(&mut rng);
    let multisig_public_key = MultisigCommittee::new(
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
            SignatureScheme::Multisig,
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
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(&mut rng));
    let raw = key_pair.public_key().as_ref().to_vec();
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
    assert_ne!(expected, key_pair.public_key().derive_address());
}

#[test]
fn address_error_on_secp256k1_zero_bytes() {
    // 33 zero bytes have the correct length for Secp256k1 but are not a valid
    // compressed curve point (prefix 0x00 is neither 0x02 nor 0x03).
    let mut bcs_bytes = vec![SignatureScheme::Secp256k1.to_u8()];
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
    let mut bcs_bytes = vec![SignatureScheme::Ed25519.to_u8()];
    bcs_bytes.extend(bcs::to_bytes(&vec![0u8; 1]).unwrap());
    let invalid: MovePublicKey = bcs::from_bytes(&bcs_bytes).unwrap();

    let err = invalid.address().unwrap_err().to_string();
    assert!(
        err.contains("Invalid public key bytes"),
        "unexpected error: {err}"
    );
}

// === Cross-language pin tests (Move ↔ Rust address parity) ===

// Key material shared with the Move test vectors in public_key_tests.move.
const ED25519_PK_HEX: &str = "cc62332e34bb2d5cd69f60efbb2a36cb916c7eb458301ea36636c4dbb012bd88";
const SECP256K1_PK_HEX: &str = "02337cca2171fdbfcfd657fa59881f46269f1e590b5ffab6023686c7ad2ecc2c1c";
const SECP256R1_PK_HEX: &str = "0227322b3a891a0a280d6bc1fb2cbb23d28f54906fd6407f5f741f6def5762609a";

#[test]
fn address_multisig_ed25519_only_matches_move_vector() {
    // 1-of-1 Ed25519 MultiSig. Ed25519 members have no scheme-flag prefix in
    // the hash input — the IOTA legacy rule mirrored by update_hasher_with_flag.
    let ed25519_pk =
        Ed25519PublicKey::new(hex::decode(ED25519_PK_HEX).unwrap().try_into().unwrap());
    let multisig_pk = MultisigCommittee::new(vec![MultisigMember::new(ed25519_pk, 1)], 1).unwrap();

    let addr = Address::from(&multisig_pk);
    let expected = Address::new(
        hex::decode("1cc23b51b2e3c8641eea35b29114a53ad7a76643dcb2763d12290a7b83cac525")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    assert_eq!(addr, expected);

    let move_pk = MovePublicKey::new(
        SignatureScheme::Multisig,
        bcs::to_bytes(&multisig_pk).unwrap(),
    )
    .unwrap();
    assert_eq!(move_pk.address().unwrap(), expected);
}

#[test]
fn address_multisig_secp256k1_secp256r1_matches_move_vector() {
    // 1-of-2 Secp256k1 + Secp256r1 MultiSig: pins the scheme-flag prefix of both
    // flagged member types (Secp256k1 = 0x01, Secp256r1 = 0x02).
    let secp256k1_pk =
        Secp256k1PublicKey::new(hex::decode(SECP256K1_PK_HEX).unwrap().try_into().unwrap());
    let secp256r1_pk =
        Secp256r1PublicKey::new(hex::decode(SECP256R1_PK_HEX).unwrap().try_into().unwrap());
    let multisig_pk = MultisigCommittee::new(
        vec![
            MultisigMember::new(secp256k1_pk, 1),
            MultisigMember::new(secp256r1_pk, 1),
        ],
        1,
    )
    .unwrap();

    let addr = Address::from(&multisig_pk);
    let expected = Address::new(
        hex::decode("16cc5f64810dd2ca27d4bf4a884ffd3570800349238b403c072fd6ca6a9a4c97")
            .unwrap()
            .try_into()
            .unwrap(),
    );
    assert_eq!(addr, expected);

    let move_pk = MovePublicKey::new(
        SignatureScheme::Multisig,
        bcs::to_bytes(&multisig_pk).unwrap(),
    )
    .unwrap();
    assert_eq!(move_pk.address().unwrap(), expected);
}

// === verify_builtin_authenticator_signature() happy path ===

#[test]
fn verify_builtin_authenticator_signature_ok_ed25519() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let verifier = builtin_signature_verifier(&key_pair);
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_builtin_authenticator_signature_ok_secp256k1() {
    let key_pair = SimpleKeypair::from(Secp256k1PrivateKey::random_with(seeded_rng()));
    let verifier = builtin_signature_verifier(&key_pair);
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_builtin_authenticator_signature_ok_secp256r1() {
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(seeded_rng()));
    let verifier = builtin_signature_verifier(&key_pair);
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_builtin_authenticator_signature_ok_multisig() {
    let mut rng = seeded_rng();
    let kp1 = Secp256k1PrivateKey::random_with(&mut rng);
    let kp2 = Secp256r1PrivateKey::random_with(&mut rng);
    let multisig_public_key = MultisigCommittee::new(
        vec![
            MultisigMember::new(kp1.public_key(), 1),
            MultisigMember::new(kp2.public_key(), 1),
        ],
        1,
    )
    .unwrap();
    let sender = Address::from(&multisig_public_key);

    let tx_data = dummy_tx_data(sender);
    let tx_data_bytes = bcs::to_bytes(&tx_data).unwrap();
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);

    let msg = intent_msg.signing_digest();
    let sig1: SimpleSignature = kp1.sign(&msg);
    let multisig = UserSignature::Multisig(
        MultisigAggregatedSignature::new(vec![sig1.into()], multisig_public_key.clone()).unwrap(),
    );

    let verifier = BuiltinSignatureVerifier {
        public_key: MovePublicKey::new(
            SignatureScheme::Multisig,
            bcs::to_bytes(&multisig_public_key).unwrap(),
        )
        .unwrap(),
        signature: multisig.to_bytes(),
        tx_data_bytes,
    };
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_builtin_authenticator_signature_ok_passkey() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(&mut rng));

    // Passkey address is derived from the Secp256r1 key under the Passkey flag.
    let raw_public_key = key_pair.public_key().as_ref().to_vec();
    let passkey_public_key =
        PublicKey::try_from_bytes(SignatureScheme::PasskeyAuthenticator, &raw_public_key).unwrap();
    let sender = Address::from(&passkey_public_key);

    let tx_data = dummy_tx_data(sender);
    let tx_data_bytes = bcs::to_bytes(&tx_data).unwrap();
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);

    // Challenge = Blake2b256 hash of the BCS-encoded intent message.
    let signature = passkey_signature(&key_pair, &intent_msg.signing_digest());

    let verifier = BuiltinSignatureVerifier {
        public_key: MovePublicKey::new(SignatureScheme::PasskeyAuthenticator, raw_public_key)
            .unwrap(),
        signature: signature.to_bytes(),
        tx_data_bytes,
    };
    assert!(verifier.verify().is_ok());
}

// === verify_builtin_authenticator_signature() errors ===

#[test]
fn verify_builtin_authenticator_signature_error_invalid_sig_bytes() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let mut verifier = builtin_signature_verifier(&key_pair);
    // An unrecognized scheme flag so UserSignature rejects it.
    verifier.signature = vec![0xAB, 0xCD, 0xEF];

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Invalid signature bytes")
    ));
}

#[test]
fn verify_builtin_authenticator_signature_error_unsupported_sig_type() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let mut verifier = builtin_signature_verifier(&key_pair);
    // A MoveAuthenticator in wire format parses as
    // UserSignature::MoveAuthenticator, which hits the unsupported branch in
    // verify_builtin_authenticator_signature.
    verifier.signature = make_authenticator(vec![], vec![]).to_bytes();

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Unsupported signature type")
    ));
}

#[test]
fn verify_builtin_authenticator_signature_error_sig_scheme_mismatch() {
    let mut rng = seeded_rng();
    // Signature is ED25519 but the public key is Secp256k1.
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let secp256k1_key_pair = SimpleKeypair::from(Secp256k1PrivateKey::random_with(&mut rng));
    let mut verifier = builtin_signature_verifier(&key_pair);
    verifier.public_key = MovePublicKey::from(&secp256k1_key_pair);

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Signature scheme mismatch")
                && error.contains("Secp256k1")
                && error.contains("Ed25519")
    ));
}

#[test]
fn verify_builtin_authenticator_signature_error_invalid_public_key_bytes() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let mut verifier = builtin_signature_verifier(&key_pair);
    // Construct a MovePublicKey with 1 raw byte for ED25519 (requires 32) by
    // bypassing new() via BCS deserialization.
    let mut bcs_bytes = vec![SignatureScheme::Ed25519.to_u8()];
    bcs_bytes.extend(bcs::to_bytes(&vec![0u8; 1]).unwrap());
    verifier.public_key = bcs::from_bytes(&bcs_bytes).unwrap();

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Invalid public key bytes")
    ));
}

#[test]
fn verify_builtin_authenticator_signature_error_other_public_key() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let other_key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let mut verifier = builtin_signature_verifier(&key_pair);
    verifier.public_key = MovePublicKey::from(&other_key_pair);

    assert!(verifier.verify().is_err());
}

#[test]
fn verify_builtin_authenticator_signature_error_invalid_tx_data_bytes() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let mut verifier = builtin_signature_verifier(&key_pair);
    verifier.tx_data_bytes = vec![];

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Failed to deserialize transaction data")
    ));
}

// === verify_personal_message_signature() happy path ===

#[test]
fn verify_personal_message_signature_ok_ed25519() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let verifier = personal_message_signature_verifier(&key_pair);
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_personal_message_signature_ok_secp256k1() {
    let key_pair = SimpleKeypair::from(Secp256k1PrivateKey::random_with(seeded_rng()));
    let verifier = personal_message_signature_verifier(&key_pair);
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_personal_message_signature_ok_secp256r1() {
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(seeded_rng()));
    let verifier = personal_message_signature_verifier(&key_pair);
    assert!(verifier.verify().is_ok());
}

#[test]
fn verify_personal_message_signature_ok_passkey() {
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(seeded_rng()));
    // The challenge is the signing digest of the personal message.
    let challenge = personal_message_intent(PERSONAL_MESSAGE).signing_digest();

    let verifier = PersonalMessageSignatureVerifier {
        public_key: passkey_public_key(&key_pair),
        signature: passkey_signature(&key_pair, &challenge).to_bytes(),
        message: PERSONAL_MESSAGE.to_vec(),
    };
    assert!(verifier.verify().is_ok());
}

// === verify_personal_message_signature() errors ===

#[test]
fn verify_personal_message_signature_error_other_public_key() {
    let mut rng = seeded_rng();
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let other_key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let mut verifier = personal_message_signature_verifier(&key_pair);
    verifier.public_key = MovePublicKey::from(&other_key_pair);

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::IncorrectSigner { error }
            if error.contains("Incorrect signer")
    ));
}

#[test]
fn verify_personal_message_signature_error_other_message() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let mut verifier = personal_message_signature_verifier(&key_pair);
    verifier.message = b"another message".to_vec();

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Fail to verify user sig")
    ));
}

#[test]
fn verify_personal_message_signature_error_transaction_intent() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let mut verifier = personal_message_signature_verifier(&key_pair);
    // The same bytes, signed with the transaction intent.
    let intent_msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage(PERSONAL_MESSAGE.into()),
    );
    let sig: SimpleSignature = key_pair.sign(&intent_msg.signing_digest());
    verifier.signature = UserSignature::Simple(sig).to_bytes();

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Fail to verify user sig")
    ));
}

#[test]
fn verify_personal_message_signature_error_passkey_transaction_intent_challenge() {
    let key_pair = SimpleKeypair::from(Secp256r1PrivateKey::random_with(seeded_rng()));
    // The challenge is the digest of the same bytes with the transaction
    // intent.
    let challenge = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage(PERSONAL_MESSAGE.into()),
    )
    .signing_digest();

    let verifier = PersonalMessageSignatureVerifier {
        public_key: passkey_public_key(&key_pair),
        signature: passkey_signature(&key_pair, &challenge).to_bytes(),
        message: PERSONAL_MESSAGE.to_vec(),
    };
    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("passkey challenge does not match expected message")
    ));
}

#[test]
fn verify_personal_message_signature_error_sig_scheme_mismatch() {
    let mut rng = seeded_rng();
    // Signature is ED25519 but the public key is Secp256k1.
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(&mut rng));
    let secp256k1_key_pair = SimpleKeypair::from(Secp256k1PrivateKey::random_with(&mut rng));
    let mut verifier = personal_message_signature_verifier(&key_pair);
    verifier.public_key = MovePublicKey::from(&secp256k1_key_pair);

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Signature scheme mismatch")
    ));
}

#[test]
fn verify_personal_message_signature_error_sig_scheme_mismatch_malformed_signature() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let mut verifier = personal_message_signature_verifier(&key_pair);
    // The Passkey flag followed by bytes that do not decode: a decode before
    // the scheme comparison would return another error.
    verifier.signature = vec![SignatureScheme::PasskeyAuthenticator.to_u8(), 0xFF, 0xFF];

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Signature scheme mismatch")
    ));
}

#[test]
fn verify_personal_message_signature_error_invalid_sig_bytes() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let mut verifier = personal_message_signature_verifier(&key_pair);
    // An unrecognized scheme flag so UserSignature rejects it.
    verifier.signature = vec![0xAB, 0xCD, 0xEF];

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Invalid signature bytes")
    ));
}

#[test]
fn verify_personal_message_signature_error_empty_sig_bytes() {
    let key_pair = SimpleKeypair::from(Ed25519PrivateKey::random_with(seeded_rng()));
    let mut verifier = personal_message_signature_verifier(&key_pair);
    verifier.signature = vec![];

    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("Invalid signature bytes")
    ));
}

#[test]
fn verify_personal_message_signature_error_multisig_public_key() {
    let (secp256k1_member, committee) = multisig_committee();

    // A MultiSig signature of the personal message that reaches the threshold.
    let msg = personal_message_intent(PERSONAL_MESSAGE).signing_digest();
    let sig: SimpleSignature = secp256k1_member.sign(&msg);
    let multisig = UserSignature::Multisig(
        MultisigAggregatedSignature::new(vec![sig.into()], committee.clone()).unwrap(),
    );

    let verifier = PersonalMessageSignatureVerifier {
        public_key: multisig_public_key(&committee),
        signature: multisig.to_bytes(),
        message: PERSONAL_MESSAGE.to_vec(),
    };
    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("MultiSig public key is not accepted")
    ));
}

#[test]
fn verify_personal_message_signature_error_multisig_public_key_malformed_signature() {
    let (_, committee) = multisig_committee();

    let verifier = PersonalMessageSignatureVerifier {
        public_key: multisig_public_key(&committee),
        // The MultiSig flag followed by bytes that do not decode: a decode
        // before the MultiSig check would return another error.
        signature: vec![SignatureScheme::Multisig.to_u8(), 0xFF, 0xFF],
        message: PERSONAL_MESSAGE.to_vec(),
    };
    assert!(matches!(
        verifier.verify().unwrap_err(),
        IotaError::InvalidSignature { error }
            if error.contains("MultiSig public key is not accepted")
    ));
}

// === Helpers ===

/// The error from building a `MovePublicKey` around `committee`.
fn multisig_error(committee: MultisigCommittee) -> String {
    MovePublicKey::new(
        SignatureScheme::Multisig,
        bcs::to_bytes(&committee).unwrap(),
    )
    .unwrap_err()
    .to_string()
}

/// A valid two-member committee's members, for tests that need to assemble an
/// invalid committee around them.
fn multisig_members() -> Vec<MultisigMember> {
    let mut rng = seeded_rng();
    let kp1 = Secp256k1PrivateKey::random_with(&mut rng);
    let kp2 = Secp256r1PrivateKey::random_with(&mut rng);
    vec![
        MultisigMember::new(kp1.public_key(), 1),
        MultisigMember::new(kp2.public_key(), 1),
    ]
}

fn seeded_rng() -> StdRng {
    StdRng::from_seed([0; 32])
}

const PERSONAL_MESSAGE: &[u8] = b"a personal message";

/// Calls `verify_builtin_authenticator_signature` with the arguments for a signed
/// transaction.
struct BuiltinSignatureVerifier {
    public_key: MovePublicKey,
    signature: Vec<u8>,
    tx_data_bytes: Vec<u8>,
}

impl BuiltinSignatureVerifier {
    fn verify(&self) -> Result<(), IotaError> {
        verify_builtin_authenticator_signature(
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            &self.public_key,
            &self.signature,
            &self.tx_data_bytes,
        )
    }
}

/// Calls `verify_personal_message_signature` with the arguments for a signed
/// personal message.
struct PersonalMessageSignatureVerifier {
    public_key: MovePublicKey,
    signature: Vec<u8>,
    message: Vec<u8>,
}

impl PersonalMessageSignatureVerifier {
    fn verify(&self) -> Result<(), IotaError> {
        verify_personal_message_signature(
            &ProtocolConfig::get_for_max_version_UNSAFE(),
            &self.public_key,
            &self.signature,
            &self.message,
        )
    }
}

fn make_authenticator(call_args: Vec<CallArg>, type_args: Vec<TypeTag>) -> MoveAuthenticator {
    MoveAuthenticatorV1::new(
        call_args,
        type_args,
        SharedObjectReference::new(ObjectId::ZERO, Version::default(), false),
    )
    .into()
}

/// Constructs a minimal dummy `Transaction` for `sender`.
fn dummy_tx_data(sender: Address) -> Transaction {
    let gas_ref = ObjectReference::new(ObjectId::ZERO, Version::default(), ObjectDigest::MIN);
    Transaction::new_transfer_iota(
        Address::ZERO,
        sender,
        None,
        gas_ref,
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
        1000,
    )
}

/// Signs a dummy transaction sent by the address of `key_pair` and returns the
/// `BuiltinSignatureVerifier` for it.
fn builtin_signature_verifier(key_pair: &SimpleKeypair) -> BuiltinSignatureVerifier {
    let public_key = key_pair.public_key();
    let tx_data = dummy_tx_data(public_key.derive_address());
    let tx_data_bytes = bcs::to_bytes(&tx_data).unwrap();

    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);
    let sig: SimpleSignature = key_pair.sign(&intent_msg.signing_digest());

    BuiltinSignatureVerifier {
        public_key: MovePublicKey::from(key_pair),
        signature: UserSignature::Simple(sig).to_bytes(),
        tx_data_bytes,
    }
}

fn personal_message_intent(message: &[u8]) -> IntentMessage<PersonalMessage<'_>> {
    IntentMessage::new(Intent::personal_message(), PersonalMessage(message.into()))
}

/// Signs `PERSONAL_MESSAGE` with `key_pair` and returns the
/// `PersonalMessageSignatureVerifier` for it.
fn personal_message_signature_verifier(
    key_pair: &SimpleKeypair,
) -> PersonalMessageSignatureVerifier {
    let intent_msg = personal_message_intent(PERSONAL_MESSAGE);
    let sig: SimpleSignature = key_pair.sign(&intent_msg.signing_digest());

    PersonalMessageSignatureVerifier {
        public_key: MovePublicKey::from(key_pair),
        signature: UserSignature::Simple(sig).to_bytes(),
        message: PERSONAL_MESSAGE.to_vec(),
    }
}

/// Returns a 1-of-2 MultiSig committee of a Secp256k1 and a Secp256r1 key,
/// with the private key of its Secp256k1 member.
fn multisig_committee() -> (Secp256k1PrivateKey, MultisigCommittee) {
    let mut rng = seeded_rng();
    let secp256k1_member = Secp256k1PrivateKey::random_with(&mut rng);
    let secp256r1_member = Secp256r1PrivateKey::random_with(&mut rng);
    let committee = MultisigCommittee::new(
        vec![
            MultisigMember::new(secp256k1_member.public_key(), 1),
            MultisigMember::new(secp256r1_member.public_key(), 1),
        ],
        1,
    )
    .unwrap();
    (secp256k1_member, committee)
}

/// Returns the MultiSig public key of `committee`.
fn multisig_public_key(committee: &MultisigCommittee) -> MovePublicKey {
    MovePublicKey::new(SignatureScheme::Multisig, bcs::to_bytes(committee).unwrap()).unwrap()
}

/// Returns the Passkey public key of the Secp256r1 `key_pair`.
fn passkey_public_key(key_pair: &SimpleKeypair) -> MovePublicKey {
    MovePublicKey::new(
        SignatureScheme::PasskeyAuthenticator,
        key_pair.public_key().as_ref().to_vec(),
    )
    .unwrap()
}

/// Returns the Passkey signature, by the Secp256r1 `key_pair`, of a WebAuthn
/// assertion whose challenge is `challenge`.
fn passkey_signature(key_pair: &SimpleKeypair, challenge: &[u8]) -> UserSignature {
    let challenge_b64 = Base64UrlUnpadded::encode_string(challenge);

    let client_data_json = format!(
        r#"{{"type":"webauthn.get","challenge":"{challenge_b64}","origin":"https://iota.org","crossOrigin":false}}"#
    );
    let authenticator_data = vec![0xAB];

    // WebAuthn message: authenticator_data || sha256(client_data_json).
    let client_data_hash = Sha256::digest(client_data_json.as_bytes()).digest;
    let mut webauthn_msg = authenticator_data.clone();
    webauthn_msg.extend_from_slice(&client_data_hash);

    // Sign the WebAuthn message with the Secp256r1 key.
    let user_sig: SimpleSignature = key_pair.sign(&webauthn_msg);
    let passkey =
        PasskeyAuthenticator::new(authenticator_data, client_data_json, user_sig).unwrap();
    UserSignature::PasskeyAuthenticator(passkey)
}
