// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_crypto::{Signer, ed25519::Ed25519PrivateKey, secp256k1::Secp256k1PrivateKey};
use iota_sdk_types::{
    Address,
    crypto::{
        Ed25519Signature, Intent, IntentMessage, MultisigAggregatedSignature, MultisigCommittee,
        MultisigMember, MultisigMemberSignature, PersonalMessage, Secp256k1Signature,
    },
};

use crate::{
    error::IotaError,
    signature::{AuthenticatorTrait, VerifyParams},
};

#[test]
fn verify_rejects_signature_pubkey_scheme_mismatch() {
    // Build a multisig whose single committee member holds an Ed25519 public
    // key, but whose accompanying member signature is Secp256k1. The committee
    // and bitmap are otherwise well-formed, so `validate()` passes and the
    // mismatch is only observable inside `verify_claims`.
    let kp1 = Ed25519PrivateKey::random();
    let kp2 = Secp256k1PrivateKey::random();

    let multisig_pk =
        MultisigCommittee::new(vec![MultisigMember::new(kp1.public_key(), 1)], 1).unwrap();
    let multisig_address: Address = (&multisig_pk).into();

    let intent_msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Hello".as_bytes().to_vec().into()),
    );

    // Sign with the Secp256k1 key even though the committee member is Ed25519.
    let secp_sig: Secp256k1Signature = kp2.sign(&intent_msg.signing_digest());
    let multisig = MultisigAggregatedSignature::new_unchecked(
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
            &VerifyParams::new(false, true, true),
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
            &VerifyParams::new(false, false, true),
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

#[test]
fn verify_rejects_partially_signed_mixed_ed25519_multisig() {
    let ed25519_key = Ed25519PrivateKey::random();
    let secp256k1_key = Secp256k1PrivateKey::random();

    // `MultisigCommittee::new` rejects committees mixing Ed25519 with other key
    // schemes, but such committees can still appear in submitted signatures.
    let multisig_pk = MultisigCommittee::new_unchecked(
        vec![
            MultisigMember::new(ed25519_key.public_key(), 1),
            MultisigMember::new(secp256k1_key.public_key(), 1),
        ],
        1,
    );
    let multisig_address: Address = (&multisig_pk).into();

    let intent_msg = IntentMessage::new(
        Intent::iota_transaction(),
        PersonalMessage("Hello".as_bytes().to_vec().into()),
    );
    let ed25519_sig: Ed25519Signature = ed25519_key.sign(&intent_msg.signing_digest());
    let secp256k1_sig: Secp256k1Signature = secp256k1_key.sign(&intent_msg.signing_digest());

    let partial = MultisigAggregatedSignature::new_unchecked(
        vec![MultisigMemberSignature::Ed25519(ed25519_sig)],
        0b01,
        multisig_pk.clone(),
    );
    partial
        .verify_claims(
            &intent_msg,
            multisig_address,
            &VerifyParams::new(false, true, false),
        )
        .unwrap();
    let err = partial
        .verify_claims(
            &intent_msg,
            multisig_address,
            &VerifyParams::new(false, true, true),
        )
        .unwrap_err();
    assert!(
        matches!(
            &err,
            IotaError::InvalidSignature { error }
                if error.contains("requires a signature from every member")
        ),
        "expected a missing-signatures error, got {err:?}"
    );

    let full = MultisigAggregatedSignature::new_unchecked(
        vec![
            MultisigMemberSignature::Ed25519(ed25519_sig),
            MultisigMemberSignature::Secp256k1(secp256k1_sig),
        ],
        0b11,
        multisig_pk,
    );
    full.verify_claims(
        &intent_msg,
        multisig_address,
        &VerifyParams::new(false, true, true),
    )
    .unwrap();
}
