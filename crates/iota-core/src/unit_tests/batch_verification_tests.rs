// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use fastcrypto::{
    hash::{HashFunction, Sha256},
    rsa::{Base64UrlUnpadded, Encoding as _},
    traits::KeyPair,
};
use futures::future::join_all;
use iota_macros::sim_test;
use iota_protocol_config::ProtocolConfig;
use iota_sdk_crypto::{Signer, secp256r1::Secp256r1PrivateKey};
use iota_sdk_types::{
    Address, CheckpointContents, CheckpointSummary, GasCostSummary, PasskeyPublicKey,
    SimpleSignature, Transaction, UserSignature,
    crypto::{
        MultisigAggregatedSignature, MultisigCommittee, MultisigMember, PasskeyAuthenticator,
    },
};
use iota_types::{
    base_types::random_object_ref,
    committee::Committee,
    crypto::{AccountPrivateKey, AuthorityKeyPair, AuthorityPublicKeyBytes, get_key_pair},
    messages_checkpoint::{CheckpointContentsExt, CheckpointSummaryExt, SignedCheckpointSummary},
    transaction::{
        CertifiedTransaction, SignedTransaction, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI,
        TransactionEnvelope,
    },
};
use itertools::Itertools as _;
use prometheus_filtered::Registry;
use rand::{RngExt, rng};

use crate::{
    signature_verifier::*,
    test_utils::{make_cert_with_large_committee, make_dummy_tx},
};

// TODO consolidate with `gen_certs` in batch_verification_bench.rs
fn gen_certs(
    committee: &Committee,
    key_pairs: &[AuthorityKeyPair],
    count: usize,
) -> Vec<CertifiedTransaction> {
    let receiver = Address::random();

    let senders: Vec<_> = (0..count)
        .map(|_| get_key_pair::<AccountPrivateKey>())
        .collect();

    let txns: Vec<_> = senders
        .iter()
        .map(|(sender, sender_sec)| make_dummy_tx(receiver, *sender, sender_sec))
        .collect();

    txns.iter()
        .map(|t| make_cert_with_large_committee(committee, key_pairs, t))
        .collect()
}

fn gen_ckpts(
    committee: &Committee,
    key_pairs: &[AuthorityKeyPair],
    count: usize,
) -> Vec<SignedCheckpointSummary> {
    (0..count)
        .map(|i| {
            let k = &key_pairs[i % key_pairs.len()];
            let name = k.public().into();
            SignedCheckpointSummary::new(
                committee.epoch,
                CheckpointSummary::new_with_protocol_config(
                    &ProtocolConfig::get_for_max_version_UNSAFE(),
                    committee.epoch,
                    // insert different data for each checkpoint so that we can swap sigs later
                    // and get a failure. (otherwise every checkpoint is the same so the
                    // AuthoritySignInfos are interchangeable).
                    i as u64,
                    0,
                    &CheckpointContents::new_with_digests_only_for_tests(vec![]),
                    None,
                    GasCostSummary::default(),
                    None,
                    0,
                    Vec::new(),
                ),
                k,
                name,
            )
        })
        .collect()
}

#[sim_test]
async fn test_batch_verify() {
    let (committee, key_pairs) = Committee::new_simple_test_committee();

    let certs = gen_certs(&committee, &key_pairs, 16);
    let ckpts = gen_ckpts(&committee, &key_pairs, 16);

    batch_verify_all_certificates_and_checkpoints(
        &committee,
        &certs.iter().collect_vec(),
        &ckpts.iter().collect_vec(),
    )
    .unwrap();

    {
        let mut ckpts = gen_ckpts(&committee, &key_pairs, 16);
        *ckpts[0].auth_sig_mut_for_testing() = ckpts[1].auth_sig().clone();
        batch_verify_all_certificates_and_checkpoints(
            &committee,
            &certs.iter().collect_vec(),
            &ckpts.iter().collect_vec(),
        )
        .unwrap_err();
    }

    let (other_sender, other_sender_sec): (_, AccountPrivateKey) = get_key_pair();
    // this test is a bit much for the current implementation - it was originally
    // written to verify a bisecting fall back approach.
    for i in 0..16 {
        let receiver = Address::random();
        let mut certs = certs.clone();
        let other_tx = make_dummy_tx(receiver, other_sender, &other_sender_sec);
        let other_cert = make_cert_with_large_committee(&committee, &key_pairs, &other_tx);
        *certs[i].auth_sig_mut_for_testing() = other_cert.auth_sig().clone();
        batch_verify_all_certificates_and_checkpoints(
            &committee,
            &certs.iter().collect_vec(),
            &ckpts.iter().collect_vec(),
        )
        .unwrap_err();

        let results = batch_verify_certificates(&committee, &certs.iter().collect_vec());
        results[i].as_ref().unwrap_err();
        for (_, r) in results.iter().enumerate().filter(|(j, _)| *j != i) {
            r.as_ref().unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_async_verifier() {
    let (committee, key_pairs) = Committee::new_simple_test_committee();
    let committee = Arc::new(committee);
    let key_pairs = Arc::new(key_pairs);

    let registry = Registry::new();
    let metrics = SignatureVerifierMetrics::new(&registry);
    let verifier = Arc::new(SignatureVerifier::new(
        committee.clone(),
        BTreeSet::new(),
        metrics,
        true, // accept_passkey_in_multisig
        true, // additional_multisig_checks
    ));

    let tasks: Vec<_> = (0..32)
        .map(|_| {
            let verifier = verifier.clone();
            let committee = committee.clone();
            let key_pairs = key_pairs.clone();
            tokio::task::spawn(async move {
                let certs = gen_certs(&committee, &key_pairs, 100);

                let receiver = Address::random();
                let (other_sender, other_sender_sec): (_, AccountPrivateKey) = get_key_pair();
                let other_tx = make_dummy_tx(receiver, other_sender, &other_sender_sec);
                let other_cert = make_cert_with_large_committee(&committee, &key_pairs, &other_tx);

                for mut c in certs.into_iter() {
                    if rng().random_range(0..20) == 0 {
                        *c.auth_sig_mut_for_testing() = other_cert.auth_sig().clone();
                        verifier.verify_cert(c).await.unwrap_err();
                    } else {
                        verifier.verify_cert(c).await.unwrap();
                    }
                }
            })
        })
        .collect();

    join_all(tasks).await;
}

/// Makes a transaction signed by a multisig whose only member is a passkey.
fn make_passkey_multisig_tx() -> TransactionEnvelope {
    let passkey_key = Secp256r1PrivateKey::random();
    let multisig_committee = MultisigCommittee::new(
        vec![MultisigMember::new(
            PasskeyPublicKey::new(passkey_key.public_key()),
            1,
        )],
        1,
    )
    .unwrap();
    let tx = Transaction::new_transfer(
        Address::random(),
        random_object_ref(),
        Address::from(&multisig_committee),
        random_object_ref(),
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER * 10,
        10,
    );

    // A WebAuthn authenticator signs `authenticator_data ||
    // sha256(client_data_json)`, where the client data carries the transaction
    // signing digest as challenge.
    let client_data_json = format!(
        r#"{{"type":"webauthn.get","challenge":"{}","origin":"https://test.iota.org"}}"#,
        Base64UrlUnpadded::encode_string(&tx.signing_digest())
    );
    let authenticator_data = vec![0u8; 37];
    let mut passkey_message = authenticator_data.clone();
    passkey_message.extend_from_slice(&Sha256::digest(client_data_json.as_bytes()).digest);
    let signature: SimpleSignature = passkey_key.sign(&passkey_message);
    let passkey =
        PasskeyAuthenticator::new(authenticator_data, client_data_json, signature).unwrap();

    let multisig =
        MultisigAggregatedSignature::new(vec![passkey.into()], multisig_committee).unwrap();
    TransactionEnvelope::from_user_sig_data(tx, vec![UserSignature::Multisig(multisig)])
}

/// Certifies `transaction` with a quorum of `key_pairs` without checking the
/// user signatures, which `make_cert_with_large_committee` checks with
/// default verification params.
fn certify(
    committee: &Committee,
    key_pairs: &[AuthorityKeyPair],
    transaction: &TransactionEnvelope,
) -> CertifiedTransaction {
    let count = (key_pairs.len() * 2).div_ceil(3);
    let sigs = key_pairs
        .iter()
        .take(count)
        .map(|key_pair| {
            SignedTransaction::new(
                committee.epoch(),
                transaction.clone().into_data(),
                key_pair,
                AuthorityPublicKeyBytes::from(key_pair.public()),
            )
            .auth_sig()
            .clone()
        })
        .collect();
    CertifiedTransaction::new(transaction.clone().into_data(), sigs, committee).unwrap()
}

/// Returns a valid passkey multisig certificate and a certificate with a valid
/// user signature but an authority signature over different data.
fn make_passkey_multisig_cert_and_invalid_cert(
    committee: &Committee,
    key_pairs: &[AuthorityKeyPair],
) -> (CertifiedTransaction, CertifiedTransaction) {
    let passkey_cert = certify(committee, key_pairs, &make_passkey_multisig_tx());

    let certs = gen_certs(committee, key_pairs, 2);
    let mut invalid_cert = certs[0].clone();
    *invalid_cert.auth_sig_mut_for_testing() = certs[1].auth_sig().clone();

    (passkey_cert, invalid_cert)
}

#[tokio::test]
async fn test_batch_verify_fallback_accepts_passkey_multisig() {
    let (committee, key_pairs) = Committee::new_simple_test_committee();
    let (passkey_cert, invalid_cert) =
        make_passkey_multisig_cert_and_invalid_cert(&committee, &key_pairs);

    // A failing batch is re-verified one certificate at a time; the valid
    // certificate must not be rejected because of the other one.
    let results = batch_verify_certificates(&committee, &[&passkey_cert, &invalid_cert]);
    assert_eq!(results.len(), 2);
    results[0].as_ref().unwrap();
    results[1].as_ref().unwrap_err();
}

#[tokio::test]
async fn test_async_verifier_batch_with_invalid_cert_accepts_passkey_multisig() {
    let (committee, key_pairs) = Committee::new_simple_test_committee();
    let committee = Arc::new(committee);
    let (passkey_cert, invalid_cert) =
        make_passkey_multisig_cert_and_invalid_cert(&committee, &key_pairs);

    let verifier = |accept_passkey_in_multisig| {
        SignatureVerifier::new_with_batch_size(
            committee.clone(),
            BTreeSet::new(),
            2,
            SignatureVerifierMetrics::new(&Registry::new()),
            accept_passkey_in_multisig,
            true, // additional_multisig_checks
        )
    };

    // A batch size of 2 puts both certificates into the same batch.
    let accepting = verifier(true);
    let (passkey_result, invalid_result) = futures::join!(
        accepting.verify_cert(passkey_cert.clone()),
        accepting.verify_cert(invalid_cert.clone()),
    );
    passkey_result.unwrap();
    invalid_result.unwrap_err();

    let rejecting = verifier(false);
    let err = rejecting.verify_cert(passkey_cert).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("Passkey sig not supported inside multisig"),
        "unexpected error: {err}"
    );
}
