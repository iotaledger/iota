// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_crypto::Signer;
use iota_sdk_types::{
    Address,
    crypto::{Intent, IntentMessage, PersonalMessage, Secp256k1Signature},
};

use crate::{
    error::IotaError,
    multisig::{MultiSig, MultiSigPublicKey, MultisigMember, MultisigMemberSignature},
    signature::{AuthenticatorTrait, VerifyParams},
    utils::multisig_keys,
};

#[test]
fn verify_rejects_signature_pubkey_scheme_mismatch() {
    // Build a multisig whose single committee member holds an Ed25519 public
    // key, but whose accompanying member signature is Secp256k1. The committee
    // and bitmap are otherwise well-formed, so `validate()` passes and the
    // mismatch is only observable inside `verify_claims`.
    let (kp1, kp2, _) = multisig_keys();

    let multisig_pk =
        MultiSigPublicKey::new(vec![MultisigMember::new(kp1.public_key(), 1)], 1).unwrap();
    let multisig_address: Address = (&multisig_pk).into();

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
