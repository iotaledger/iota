// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// TODO move tests to SDK?

use std::str::FromStr;

use fastcrypto::traits::ToFromBytes;
use iota_sdk_crypto::{Signer, ed25519::Ed25519PrivateKey};
use iota_sdk_types::{
    SimpleSignature,
    crypto::{
        Ed25519Signature, Intent, IntentMessage, MULTISIG_COMMITTEE_SIZE_MAX, PersonalMessage,
        Secp256k1Signature, Secp256r1Signature, UserSignature,
    },
};

use super::{MultiSigPublicKey, ThresholdUnit, WeightUnit};
use crate::{
    base_types::IotaAddress,
    crypto::{Ed25519IotaSignature, IotaSignatureInner},
    error::IotaError,
    multisig::{MultiSig, MultisigMember, MultisigMemberSignature},
    signature::{AuthenticatorTrait, GenericSignature, VerifyParams},
    utils::multisig_keys,
};

#[test]
fn test_combine_sigs() {
    let (kp1, kp2, kp3) = multisig_keys();

    let pk1 = kp1.public_key();
    let pk2 = kp2.public_key();

    let multisig_pk = MultiSigPublicKey::new(
        vec![MultisigMember::new(pk1, 1), MultisigMember::new(pk2, 1)],
        2,
    )
    .unwrap();

    let msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Hello".as_bytes().to_vec().into()),
    )
    .signing_digest();
    let sig1: SimpleSignature = kp1.sign(&*msg);
    let sig2: SimpleSignature = kp2.sign(&*msg);
    let sig3: SimpleSignature = kp3.sign(&*msg);

    // MultiSigPublicKey contains only 2 public key but 3 signatures are passed,
    // fails to combine.
    assert!(
        MultiSig::new(
            vec![sig1.clone().into(), sig2.into(), sig3.into()],
            multisig_pk.clone()
        )
        .is_err()
    );

    // Cannot create malformed MultiSig.
    assert!(MultiSig::new(vec![], multisig_pk.clone()).is_err());
    assert!(MultiSig::new(vec![sig1.clone().into(), sig1.into()], multisig_pk).is_err());
}

#[test]
fn test_serde_roundtrip() {
    let (kp1, kp2, kp3) = multisig_keys();
    let msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Hello".as_bytes().to_vec().into()),
    )
    .signing_digest();

    let check_roundtrip = |multisig: MultiSig| {
        let user_sig = UserSignature::Multisig(multisig);
        let user_sig_bytes = user_sig.to_bytes();
        let user_sig_roundtrip = UserSignature::from_bytes(&user_sig_bytes).unwrap();
        assert_eq!(user_sig, user_sig_roundtrip);

        // The serialized form is prefixed with the MultiSig flag 0x03.
        assert_eq!(user_sig_bytes.first().unwrap(), &0x03);
    };

    let pk1 = kp1.public_key();
    let multisig_pk = MultiSigPublicKey::new(vec![MultisigMember::new(pk1, 1)], 1).unwrap();
    let sig: Ed25519Signature = kp1.sign(&*msg);
    check_roundtrip(MultiSig::new_unchecked(vec![sig.into()], 1, multisig_pk));

    let pk2 = kp2.public_key();
    let multisig_pk = MultiSigPublicKey::new(vec![MultisigMember::new(pk2, 1)], 1).unwrap();
    let sig: Secp256k1Signature = kp2.sign(&*msg);
    check_roundtrip(MultiSig::new_unchecked(vec![sig.into()], 1, multisig_pk));

    let pk3 = kp3.public_key();
    let multisig_pk = MultiSigPublicKey::new(vec![MultisigMember::new(pk3, 1)], 1).unwrap();
    let sig: Secp256r1Signature = kp3.sign(&*msg);
    check_roundtrip(MultiSig::new_unchecked(vec![sig.into()], 1, multisig_pk));

    // Malformed multisig cannot be deserialized
    let multisig_pk =
        MultiSigPublicKey::new_unchecked(vec![MultisigMember::new(kp1.public_key(), 1)], 1);
    let multisig = MultiSig::new_unchecked(vec![], 0, multisig_pk);
    let user_sig = UserSignature::Multisig(multisig);
    assert!(UserSignature::from_bytes(user_sig.to_bytes()).is_err());

    // Malformed multisig_pk cannot be deserialized
    let multisig_pk_1 = MultiSigPublicKey::new_unchecked(vec![], 0);
    let multisig_1 = MultiSig::new_unchecked(vec![], 0, multisig_pk_1);
    let user_sig_1 = UserSignature::Multisig(multisig_1);
    assert!(UserSignature::from_bytes(user_sig_1.to_bytes()).is_err());

    // Single sig serialization unchanged.
    let sig = Ed25519IotaSignature::default();
    let single_sig = GenericSignature::Signature(sig.clone().into());
    let single_sig_bytes = single_sig.as_bytes();
    let single_sig_roundtrip = GenericSignature::from_bytes(single_sig_bytes).unwrap();
    assert_eq!(single_sig, single_sig_roundtrip);
    assert_eq!(single_sig_bytes.len(), Ed25519IotaSignature::LENGTH);
    assert_eq!(
        single_sig_bytes.first().unwrap(),
        &Ed25519IotaSignature::SCHEME.flag()
    );
    assert_eq!(sig.as_bytes().len(), single_sig_bytes.len());
}

#[test]
fn test_multisig_pk_new() {
    let (kp1, kp2, kp3) = multisig_keys();
    let pk1 = kp1.public_key();
    let pk2 = kp2.public_key();
    let pk3 = kp3.public_key();

    // Fails on weight 0.
    assert!(
        MultiSigPublicKey::new(
            vec![
                MultisigMember::new(pk1, 0),
                MultisigMember::new(pk2, 1),
                MultisigMember::new(pk3, 1)
            ],
            2
        )
        .is_err()
    );

    // Fails on threshold 0.
    assert!(
        MultiSigPublicKey::new(
            vec![
                MultisigMember::new(pk1, 1),
                MultisigMember::new(pk2, 1),
                MultisigMember::new(pk3, 1)
            ],
            0
        )
        .is_err()
    );

    // Fails on empty array length.
    assert!(MultiSigPublicKey::new(vec![], 2).is_err());

    // Fails on dup pks.
    assert!(
        MultiSigPublicKey::new(
            vec![
                MultisigMember::new(pk1, 1),
                MultisigMember::new(pk1, 2),
                MultisigMember::new(pk1, 3)
            ],
            4
        )
        .is_err()
    );
}

#[test]
fn test_multisig_address() {
    // Pin an hardcoded multisig address generation here. If this fails, the address
    // generation logic may have changed. If this is intended, update the hardcoded
    // value below.
    let (kp1, kp2, kp3) = multisig_keys();
    let pk1 = kp1.public_key();
    let pk2 = kp2.public_key();
    let pk3 = kp3.public_key();

    let threshold: ThresholdUnit = 2;
    let w1: WeightUnit = 1;
    let w2: WeightUnit = 2;
    let w3: WeightUnit = 3;

    let multisig_pk = MultiSigPublicKey::new(
        vec![
            MultisigMember::new(pk1, w1),
            MultisigMember::new(pk2, w2),
            MultisigMember::new(pk3, w3),
        ],
        threshold,
    )
    .unwrap();
    let address: IotaAddress = (&multisig_pk).into();
    assert_eq!(
        IotaAddress::from_str("0x25c72ac38e59084e0c8263489f810f50b2d1a38bbb8128a5d1474317af7c8eb3")
            .unwrap(),
        address
    );
}

#[test]
fn test_max_sig() {
    let msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Hello".as_bytes().to_vec().into()),
    )
    .signing_digest();
    let mut keys = Vec::new();
    let mut pks = Vec::new();

    for _ in 0..11 {
        let kp = Ed25519PrivateKey::generate(rand::thread_rng());
        pks.push(kp.public_key());
        keys.push(kp);
    }

    let members_with_weight = |count: usize, weight: WeightUnit| -> Vec<MultisigMember> {
        pks[..count]
            .iter()
            .cloned()
            .map(|pk| MultisigMember::new(pk, weight))
            .collect()
    };

    // multisig_pk with unreachable threshold fails.
    assert!(
        MultiSigPublicKey::new_unchecked(members_with_weight(5, 3), 16)
            .validate()
            .is_err()
    );

    // multisig_pk with max weights for each pk and max reachable threshold is ok.
    assert!(
        MultiSigPublicKey::new_unchecked(
            members_with_weight(MULTISIG_COMMITTEE_SIZE_MAX, WeightUnit::MAX),
            (WeightUnit::MAX as ThresholdUnit) * (MULTISIG_COMMITTEE_SIZE_MAX as ThresholdUnit),
        )
        .validate()
        .is_ok()
    );

    // multisig_pk with unreachable threshold fails.
    assert!(
        MultiSigPublicKey::new_unchecked(
            members_with_weight(MULTISIG_COMMITTEE_SIZE_MAX, WeightUnit::MAX),
            (WeightUnit::MAX as ThresholdUnit) * (MULTISIG_COMMITTEE_SIZE_MAX as ThresholdUnit) + 1,
        )
        .validate()
        .is_err()
    );

    // multisig_pk with max weights for each pk with threshold is 1x max weight
    // validates ok.
    let low_threshold_pk = MultiSigPublicKey::new(
        members_with_weight(MULTISIG_COMMITTEE_SIZE_MAX, WeightUnit::MAX),
        WeightUnit::MAX.into(),
    )
    .unwrap();
    let sig: SimpleSignature = keys[0].sign(&*msg);
    assert!(
        MultiSig::new(vec![sig.into()], low_threshold_pk)
            .unwrap()
            .validate()
            .is_ok()
    );
}

#[test]
fn multisig_get_pk() {
    let (kp1, kp2, _) = multisig_keys();
    let pk1 = kp1.public_key();
    let pk2 = kp2.public_key();

    let multisig_pk = MultiSigPublicKey::new(
        vec![MultisigMember::new(pk1, 1), MultisigMember::new(pk2, 1)],
        2,
    )
    .unwrap();
    let msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Hello".as_bytes().to_vec().into()),
    )
    .signing_digest();
    let sig1: SimpleSignature = kp1.sign(msg.as_ref());
    let sig2: SimpleSignature = kp2.sign(msg.as_ref());

    let multi_sig = MultiSig::new(
        vec![sig1.clone().into(), sig2.clone().into()],
        multisig_pk.clone(),
    )
    .unwrap();

    assert_eq!(multi_sig.committee(), &multisig_pk);
    assert_eq!(
        multi_sig.signatures(),
        [sig1.into(), sig2.into()].as_slice(),
    );
}

#[test]
fn multisig_get_indices() {
    let (kp1, kp2, kp3) = multisig_keys();
    let pk1 = kp1.public_key();
    let pk2 = kp2.public_key();
    let pk3 = kp3.public_key();

    let multisig_pk = MultiSigPublicKey::new(
        vec![
            MultisigMember::new(pk1, 1),
            MultisigMember::new(pk2, 1),
            MultisigMember::new(pk3, 1),
        ],
        2,
    )
    .unwrap();
    let msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Hello".as_bytes().to_vec().into()),
    )
    .signing_digest();
    let sig1: SimpleSignature = kp1.sign(msg.as_ref());
    let sig2: SimpleSignature = kp2.sign(msg.as_ref());
    let sig3: SimpleSignature = kp3.sign(msg.as_ref());

    let multi_sig1 = MultiSig::new(
        vec![sig2.clone().into(), sig3.clone().into()],
        multisig_pk.clone(),
    )
    .unwrap();

    assert!(multi_sig1.indices().unwrap() == vec![1, 2]);

    let multi_sig2 = MultiSig::new(
        vec![
            sig1.clone().into(),
            sig2.clone().into(),
            sig3.clone().into(),
        ],
        multisig_pk.clone(),
    )
    .unwrap();

    assert!(multi_sig2.indices().unwrap() == vec![0, 1, 2]);

    let invalid_multisig = MultiSig::new(vec![sig3.into(), sig2.into(), sig1.into()], multisig_pk);

    // The signatures are in the wrong order, so indices should fail.
    assert!(invalid_multisig.is_err());
}

#[test]
fn verify_rejects_signature_pubkey_scheme_mismatch() {
    // Build a multisig whose single committee member holds an Ed25519 public
    // key, but whose accompanying member signature is Secp256k1. The committee
    // and bitmap are otherwise well-formed, so `validate()` passes and the
    // mismatch is only observable inside `verify_claims`.
    let (kp1, kp2, _) = multisig_keys();

    let multisig_pk =
        MultiSigPublicKey::new(vec![MultisigMember::new(kp1.public_key(), 1)], 1).unwrap();
    let multisig_address: IotaAddress = (&multisig_pk).into();

    let intent_msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Hello".as_bytes().to_vec().into()),
    );

    // Sign with the Secp256k1 key even though the committee member is Ed25519.
    let secp_sig: Secp256k1Signature = kp2.sign(&*intent_msg.signing_digest());
    let multisig = MultiSig::new_unchecked(
        vec![MultisigMemberSignature::Secp256k1(secp_sig)],
        0b1,
        multisig_pk,
    );

    // With the additional multisig checks enabled, the scheme mismatch is
    // rejected explicitly, before any cryptographic verification is attempted.
    let err = multisig
        .verify_claims(
            &intent_msg,
            multisig_address,
            &VerifyParams::new(false, true),
        )
        .unwrap_err();
    assert!(
        matches!(
            &err,
            IotaError::InvalidSignature { error }
                if error.contains("signature/pubkey type mismatch")
        ),
        "expected a signature/pubkey type mismatch error, got {err:?}"
    );

    // The check is gated behind `additional_multisig_checks`: with it disabled
    // the early mismatch error is not raised, and verification only fails later
    // during cryptographic verification.
    let err = multisig
        .verify_claims(
            &intent_msg,
            multisig_address,
            &VerifyParams::new(false, false),
        )
        .unwrap_err();
    assert!(
        matches!(
            &err,
            IotaError::InvalidSignature { error }
                if !error.contains("signature/pubkey type mismatch")
        ),
        "the scheme mismatch must only be checked when additional_multisig_checks is enabled, got {err:?}"
    );
}
