// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests for the verdict reached when an attested transaction's Move
//! authenticator *function* aborts at execution.
//!
//! The issuer is charged only when authentication provably succeeds at the
//! versions the attestor recorded; every other outcome is charged to the
//! attestor as `InvalidAttestation`. Both cases need the account's key to be
//! rotated after the transaction is built, so that the same transaction
//! authenticates differently at two versions of the same account.

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{ExecutionError, ExecutionStatus};
use iota_types::{crypto::get_account_key_pair, effects::TransactionEffectsAPI};

use crate::authority::abstract_account_test_utils::{
    AA_AUTHENTICATE_ED25519_VIA_SIGNING_DIGEST, AbstractAccountTestEnv,
};

fn attestation_config() -> impl Drop {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config
    })
}

/// An attestor that vouched for a transaction which authenticated at the time
/// is not accountable for a later key rotation: re-running authentication at
/// the recorded versions proves the attestation honest, so the failure is
/// charged to the issuer.
#[tokio::test]
async fn honest_attestation_of_a_rotated_account_charges_the_issuer() {
    let _guard = attestation_config();

    let mut env = AbstractAccountTestEnv::new().await;
    let tx = env.account_transaction();

    // Attest while authentication still passes, so the attestation is genuine
    // and records the account at the version it authenticated against.
    let attestation = env.attest(&tx);
    let attested_version = env.account_ref().version();

    env.rotate_owner_key().await;
    assert_ne!(
        env.account_ref().version(),
        attested_version,
        "the rotation must supersede the attested version"
    );

    let effects = env.submit(tx, Some(attestation)).await;

    let ExecutionStatus::Failure { error, .. } = effects.status() else {
        panic!("expected an execution failure, got {:?}", effects.status());
    };
    assert!(
        matches!(error, ExecutionError::MoveAuthenticationError { .. }),
        "authentication succeeding at the attested versions must charge the issuer, got {error:?}"
    );
}

/// An attestor that vouched for a transaction which fails authentication at the
/// very versions it recorded is accountable for it, even though the account
/// moved on in the meantime.
#[tokio::test]
async fn attestation_that_fails_at_its_own_versions_is_invalid() {
    let _guard = attestation_config();

    let mut env = AbstractAccountTestEnv::new().await;

    // The version the attestor will claim, superseded by the rotation below so
    // the drift is real and the recorded version is still re-runnable.
    let claimed_version = env.account_ref();
    env.rotate_owner_key().await;

    // Signed with a key the account has never held, so authentication fails
    // both at the current version and at the claimed one.
    let (_, unrelated_key) = get_account_key_pair();
    let tx = env.account_transaction_signed_with(&unrelated_key);
    let attestation = env.claim_versions(vec![claimed_version]);
    let effects = env.submit(tx, Some(attestation)).await;

    let ExecutionStatus::Failure { error, .. } = effects.status() else {
        panic!("expected an execution failure, got {:?}", effects.status());
    };
    assert!(
        matches!(error, ExecutionError::InvalidAttestation),
        "an attestation whose own versions fail authentication must be invalid, got {error:?}"
    );
}

/// The account can rotate to a different authenticator, not just a new key. An
/// attestor that vouched for the transaction under the previous authenticator
/// is not accountable for the rotation: the re-run resolves the authenticator
/// recorded in the attestation rather than the one the account switched to, so
/// authentication still passes there and the failure is charged to the issuer.
#[tokio::test]
async fn honest_attestation_of_a_rotated_authenticator_charges_the_issuer() {
    let _guard = attestation_config();

    let mut env = AbstractAccountTestEnv::new().await;
    let tx = env.account_transaction();

    let attestation = env.attest(&tx);

    // Rotate to an authenticator that rejects the signature the transaction
    // carries, so the live authentication run fails and triggers the re-run.
    env.rotate_authenticator_function(AA_AUTHENTICATE_ED25519_VIA_SIGNING_DIGEST)
        .await;

    let effects = env.submit(tx, Some(attestation)).await;

    let ExecutionStatus::Failure { error, .. } = effects.status() else {
        panic!("expected an execution failure, got {:?}", effects.status());
    };
    assert!(
        matches!(error, ExecutionError::MoveAuthenticationError { .. }),
        "re-running the authenticator the attestor recorded must charge the issuer, got {error:?}"
    );
}
