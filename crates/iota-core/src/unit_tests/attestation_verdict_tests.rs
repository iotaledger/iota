// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests for the verdict reached when an attested transaction's Move
//! authentication fails at execution, and for who pays for it.
//!
//! The effects carry the issuer's [`ExecutionError::MoveAuthentication`],
//! unless a check before the body replaces it, as for a deleted input; the
//! verdict is recorded separately as [`AttestationRecord::verdict`]. An
//! attestation is refuted when authentication failed at exactly the recorded
//! state, a recorded version is ahead of the executed one or was not superseded
//! this epoch, an authenticator input is deleted, or the re-run rejects. A
//! refuted or undecided attestation charges nobody. The gas owner pays only
//! when the account is at fault, because authentication passes at the attested
//! state. Most tests rotate the account after the transaction is built - its
//! key or its authenticator function - so the same transaction authenticates
//! differently at two versions of the same account.

use iota_protocol_config::{PerObjectCongestionControlMode, ProtocolConfig};
use iota_sdk_types::{ExecutionError, ExecutionStatus, GasCostSummary, Owner, TransactionEffects};
use iota_types::{
    attestation::{AttestationRecord, AttestationVerdict},
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    execution::is_cancellation,
    object::Object,
    transaction::{TransactionAPI, TransactionEnvelope},
};

use crate::{
    authority::{
        abstract_account_test_utils::{
            AA_AUTHENTICATE_ED25519_VIA_SIGNING_DIGEST, AbstractAccountTestEnv,
        },
        pre_execution_failure_tests::{
            assert_moved_unchanged, assert_no_object_created_deleted_or_wrapped,
        },
    },
    execution_scheduler::transaction_manager::AttestationSource,
};

fn attestation_config(tolerance_percentage: Option<u64>) -> impl Drop {
    attestation_config_with(tolerance_percentage, |_| {})
}

/// Like [`attestation_config()`], with `adjust` applied on top.
fn attestation_config_with(
    tolerance_percentage: Option<u64>,
    adjust: impl Fn(&mut ProtocolConfig) + Send + Sync + 'static,
) -> impl Drop {
    ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config.set_checkpoint_summary_version_specific_data_for_testing(2);
        // Every submission is a commit of its own, so the previous commit's
        // writes must already be visible to the next one.
        config.set_pcool_deterministic_validation_horizon_distance_for_testing(1);
        if let Some(tolerance) = tolerance_percentage {
            config.set_attestor_accuracy_tolerance_percentage_for_testing(tolerance);
        }
        adjust(&mut config);

        config
    })
}

fn assert_move_authentication_error(effects: &TransactionEffects) {
    let ExecutionStatus::Failure { error, .. } = effects.status() else {
        panic!("expected an execution failure, got {:?}", effects.status());
    };
    assert!(
        matches!(error, ExecutionError::MoveAuthentication { .. }),
        "the effects carry the issuer's error regardless of the verdict, got {error:?}"
    );
}

/// The record is keyed by the block author, which
/// `SequencedConsensusTransaction::new_test` fixes at index 0.
fn assert_record(
    env: &AbstractAccountTestEnv,
    effects: &TransactionEffects,
    verdict: AttestationVerdict,
) {
    assert_eq!(
        env.attestation_record(effects.transaction_digest()),
        Some(AttestationRecord {
            attestor: 0,
            verdict,
        })
    );
}

/// The gas rounding step: every bucketized run is a whole number of these
/// units.
fn gas_rounding_step(env: &AbstractAccountTestEnv) -> u64 {
    env.authority
        .epoch_store_for_testing()
        .protocol_config()
        .gas_rounding_step()
}

/// The gas coin the transaction pays with, as the authority holds it now.
fn gas_coin(env: &AbstractAccountTestEnv, tx: &TransactionEnvelope) -> Object {
    env.object(&tx.data().transaction().gas()[0].object_id)
}

/// Asserts that the transaction's record is refuted and returns its units:
/// execution's own authentication run, whether or not the effects charge it,
/// plus the judge's re-run, both a whole number of rounding steps.
fn refuted_units(env: &AbstractAccountTestEnv, effects: &TransactionEffects) -> u64 {
    let record = env
        .attestation_record(effects.transaction_digest())
        .expect("an attested transaction has a record");
    assert_eq!(record.attestor, 0);
    let AttestationVerdict::Refuted { units } = record.verdict else {
        panic!("expected a refuted record, got {:?}", record.verdict);
    };
    let step = gas_rounding_step(env);
    assert!(
        units > 0 && units % step == 0,
        "expected whole steps of {step} units, got {units}"
    );

    units
}

/// Asserts that `effects` charge nobody: the gas coin moves to its next
/// version with its contents unchanged, and the effects carry no charge.
fn assert_not_charged(
    effects: &TransactionEffects,
    gas_coin_before: &Object,
    gas_coin_after: &Object,
) {
    assert_eq!(effects.gas_cost_summary(), &GasCostSummary::default());
    assert_moved_unchanged(gas_coin_before, gas_coin_after, effects.lamport_version());
    assert_no_object_created_deleted_or_wrapped(effects, &[]);
}

/// Asserts that `effects` charge the gas owner, as for a failed transaction it
/// authorized.
fn assert_charged(effects: &TransactionEffects) {
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the gas owner must be charged for the failed authentication, got {:?}",
        effects.gas_cost_summary()
    );
}

/// An attestor that vouched for a transaction which authenticated at the time
/// is not accountable for a later key rotation: re-running authentication at
/// the recorded versions proves the attestation honest, and the account, which
/// authorized the transaction at that state, pays for the failure.
#[tokio::test]
async fn honest_attestation_of_a_rotated_account_is_recorded_valid_and_charged() {
    let _guard = attestation_config(None);

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

    assert_move_authentication_error(&effects);
    assert_record(&env, &effects, AttestationVerdict::Valid);
    assert_charged(&effects);
}

/// An attestation at the very versions execution ran against: the failure
/// reproduces at the recorded state, so nothing is re-run, nobody is charged,
/// and the record carries the units of execution's own authentication run,
/// which the effects do not charge.
#[tokio::test]
async fn attestation_that_fails_at_the_executed_versions_is_recorded_refuted_and_not_charged() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;

    // Signed with a key the account has never held, so authentication fails
    // at the one version the attestor recorded, the account's current one.
    let (_, unrelated_key): (_, AccountPrivateKey) = get_key_pair();
    let tx = env.account_transaction_signed_with(&unrelated_key);
    let attestation = env.attest_with_versions(vec![env.account_ref()]);
    let gas_coin_before = gas_coin(&env, &tx);
    let effects = env.submit(tx, Some(attestation)).await;

    assert_move_authentication_error(&effects);
    let gas_coin_after = env.object(&gas_coin_before.id());
    assert_not_charged(&effects, &gas_coin_before, &gas_coin_after);
    let units = refuted_units(&env, &effects);
    assert!(
        units <= env.budget() / env.rgp(),
        "the units of one failed run cannot exceed the budget, got {units}"
    );
}

/// An attestor that vouched for a transaction which fails authentication at the
/// very versions it recorded is accountable for it, even though the account
/// moved on in the meantime: the re-run at the recorded versions fails as the
/// execution run did, and the record carries the units of both.
#[tokio::test]
async fn attestation_that_fails_at_its_own_versions_is_recorded_refuted_and_not_charged() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;
    let (_, unrelated_key): (_, AccountPrivateKey) = get_key_pair();

    // The same failure at the executed versions first, to learn what one
    // failed authentication run costs.
    let tx = env.account_transaction_signed_with(&unrelated_key);
    let attestation = env.attest_with_versions(vec![env.account_ref()]);
    let effects = env.submit(tx, Some(attestation)).await;
    let one_run_units = refuted_units(&env, &effects);

    // The version the attestor will claim, superseded by the rotation below so
    // the drift is real and the recorded version is still re-runnable.
    let claimed_version = env.account_ref();
    env.rotate_owner_key().await;

    // Signed with a key the account has never held, so authentication fails
    // both at the current version and at the claimed one.
    let tx = env.account_transaction_signed_with(&unrelated_key);
    let attestation = env.attest_with_versions(vec![claimed_version]);
    let gas_coin_before = gas_coin(&env, &tx);
    let effects = env.submit(tx, Some(attestation)).await;

    assert_move_authentication_error(&effects);
    let gas_coin_after = env.object(&gas_coin_before.id());
    assert_not_charged(&effects, &gas_coin_before, &gas_coin_after);
    assert_eq!(
        refuted_units(&env, &effects),
        2 * one_run_units,
        "the record must count the execution run and the re-run, each the cost of one failed run"
    );
}

/// The account can rotate to a different authenticator, not just a new key. An
/// attestor that vouched for the transaction under the previous authenticator
/// is not accountable for the rotation: the re-run resolves the authenticator
/// recorded in the attestation rather than the one the account switched to, so
/// authentication still passes there, and the account pays.
#[tokio::test]
async fn honest_attestation_of_a_rotated_authenticator_is_recorded_valid_and_charged() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;
    let tx = env.account_transaction();

    let attestation = env.attest(&tx);

    // Rotate to an authenticator that rejects the signature the transaction
    // carries, so the live authentication run fails and triggers the re-run.
    env.rotate_authenticator_function(AA_AUTHENTICATE_ED25519_VIA_SIGNING_DIGEST)
        .await;

    let effects = env.submit(tx, Some(attestation)).await;

    assert_move_authentication_error(&effects);
    assert_record(&env, &effects, AttestationVerdict::Valid);
    assert_charged(&effects);
}

/// An authenticator input deleted before execution refutes the attestation
/// without a re-run: nobody is charged, the effects report the deleted input,
/// and the validator goes on. The deletion is ordered in the same commit, just
/// before the attested transaction: a deletion at or below the horizon of the
/// commit is seen by post-consensus validation, which drops the transaction
/// before execution.
#[tokio::test]
async fn deleted_authenticator_input_is_recorded_refuted_and_not_charged() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;
    let tx = env.account_transaction();

    // Attested while the account, the authenticator's input, still exists.
    let attestation = env.attest(&tx);
    let gas_coin_before = gas_coin(&env, &tx);
    let delete_account = env.delete_account_transaction();

    let [deletion_effects, effects]: [TransactionEffects; 2] = env
        .submit_in_one_commit(vec![
            (delete_account, None, None),
            (tx, Some(attestation), None),
        ])
        .await
        .try_into()
        .expect("the effects of both transactions");
    assert!(
        deletion_effects.status().is_success(),
        "deleting the account must succeed, got {:?}",
        deletion_effects.status()
    );

    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionError::InputObjectDeleted,
            command: None,
        }
    );
    let gas_coin_after = env.object(&gas_coin_before.id());
    assert_not_charged(&effects, &gas_coin_before, &gas_coin_after);
    // Only for its assertions: the record is refuted.
    refuted_units(&env, &effects);
}

/// A sponsor that signed with its key authorized the transaction, so the
/// engine charges it for the sender's failed authentication like any failed
/// transaction, without asking the judge. The judge is asked afterwards, for
/// the record alone: the authentication fails at the recorded versions, so the
/// attestation is refuted although the effects charge the failure.
#[tokio::test]
async fn sponsored_failed_authentication_charges_the_sponsor_and_is_recorded_refuted() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;

    // Signed with a key the account has never held, so authentication fails
    // at the one version the attestor recorded, the account's current one.
    let (_, unrelated_key): (_, AccountPrivateKey) = get_key_pair();
    let tx = env.sponsored_account_transaction_signed_with(&unrelated_key);
    let attestation = env.attest_with_versions(vec![env.account_ref()]);
    let effects = env.submit(tx, Some(attestation)).await;

    assert_move_authentication_error(&effects);
    assert_charged(&effects);
    assert_eq!(
        effects.gas_object().owner(),
        &Owner::Address(env.funder()),
        "the sponsor's coin pays"
    );
    // Only for its assertions: the record is refuted.
    refuted_units(&env, &effects);
}

/// A transaction cancelled at the consensus commit never runs its
/// authenticators, and the cancellation is reported in their place. The engine
/// asks nobody about a cancellation, so nobody pays, and the judge, asked
/// afterwards for the record, blames nobody: the attestation stays valid.
#[tokio::test]
async fn cancelled_attested_transaction_charges_nobody_and_is_recorded_valid() {
    let _guard = attestation_config_with(None, |config| {
        // One transaction per shared object and commit; the next one on the
        // same object is cancelled rather than deferred.
        config.set_per_object_congestion_control_mode_for_testing(
            PerObjectCongestionControlMode::TotalTxCount,
        );
        config.set_max_accumulated_txn_cost_per_object_in_mysticeti_commit_for_testing(1);
        config.set_max_congestion_limit_overshoot_per_commit_for_testing(0);
        config.set_max_deferral_rounds_for_congestion_control_for_testing(0);
    });

    let mut env = AbstractAccountTestEnv::new().await;
    let first = env.account_transaction();
    let tx = env.account_transaction();
    let attestation = env.attest(&tx);
    let gas_coin_before = gas_coin(&env, &tx);

    // Both touch the account, so the second one is cancelled.
    let [first_effects, effects]: [TransactionEffects; 2] = env
        .submit_in_one_commit(vec![(first, None, None), (tx, Some(attestation), None)])
        .await
        .try_into()
        .expect("the effects of both transactions");
    assert!(
        first_effects.status().is_success(),
        "the first transaction must run, got {:?}",
        first_effects.status()
    );

    let ExecutionStatus::Failure { error, .. } = effects.status() else {
        panic!("expected a cancellation, got {:?}", effects.status());
    };
    assert!(
        is_cancellation(error),
        "expected a cancellation, got {error:?}"
    );
    let gas_coin_after = env.object(&gas_coin_before.id());
    assert_not_charged(&effects, &gas_coin_before, &gas_coin_after);
    assert_record(&env, &effects, AttestationVerdict::Valid);
}

/// A node executing from a checkpoint applies the certified record and never
/// judges: a refuted record charges nobody, although the re-run would have
/// passed here.
#[tokio::test]
async fn certified_refuted_record_charges_nobody_without_a_judge() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;
    let tx = env.account_transaction();
    let attestation = env.attest(&tx);
    env.rotate_owner_key().await;

    let record = AttestationRecord {
        attestor: 0,
        verdict: AttestationVerdict::Refuted { units: 0 },
    };
    let gas_coin_before = gas_coin(&env, &tx);
    let effects = env
        .submit_with_source(
            tx,
            Some(attestation),
            Some(AttestationSource::Certified(Some(record))),
        )
        .await;

    assert_move_authentication_error(&effects);
    let gas_coin_after = env.object(&gas_coin_before.id());
    assert_not_charged(&effects, &gas_coin_before, &gas_coin_after);
    assert_eq!(
        env.attestation_record(effects.transaction_digest()),
        Some(record),
        "the certified record is stored as is"
    );
}

/// A node executing from a checkpoint applies a certified undecided record as
/// the validators did when their judge could not decide: nobody is charged,
/// although the re-run would have passed here.
#[tokio::test]
async fn certified_undecided_record_charges_nobody_without_a_judge() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;
    let tx = env.account_transaction();
    let attestation = env.attest(&tx);
    env.rotate_owner_key().await;

    let record = AttestationRecord {
        attestor: 0,
        verdict: AttestationVerdict::Undecided,
    };
    let gas_coin_before = gas_coin(&env, &tx);
    let effects = env
        .submit_with_source(
            tx,
            Some(attestation),
            Some(AttestationSource::Certified(Some(record))),
        )
        .await;

    assert_move_authentication_error(&effects);
    let gas_coin_after = env.object(&gas_coin_before.id());
    assert_not_charged(&effects, &gas_coin_before, &gas_coin_after);
    assert_eq!(
        env.attestation_record(effects.transaction_digest()),
        Some(record),
        "the certified record is stored as is"
    );
}

/// Only the records other than [`AttestationVerdict::Valid`] are certified, so
/// a checkpoint without a record for the transaction means a valid
/// attestation: the account is charged, although the judge would have refuted
/// the attestation here.
#[tokio::test]
async fn certified_valid_attestation_charges_the_gas_owner_without_a_judge() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;
    let (_, unrelated_key): (_, AccountPrivateKey) = get_key_pair();
    let tx = env.account_transaction_signed_with(&unrelated_key);
    let attestation = env.attest_with_versions(vec![env.account_ref()]);

    let effects = env
        .submit_with_source(
            tx,
            Some(attestation),
            Some(AttestationSource::Certified(None)),
        )
        .await;

    assert_move_authentication_error(&effects);
    assert_charged(&effects);
    assert_eq!(env.attestation_record(effects.transaction_digest()), None);
}

/// The transaction authenticates and runs, but the attestation claims the
/// largest computation estimate consensus accepts for a trivial transaction, so
/// it is recorded inaccurate rather than valid.
#[tokio::test]
async fn overclaimed_units_are_recorded_inaccurate() {
    let _guard = attestation_config(Some(10));

    let mut env = AbstractAccountTestEnv::new().await;
    let tx = env.account_transaction();
    let attestation = env.attest_with_versions(vec![env.account_ref()]);

    let effects = env.submit(tx, Some(attestation)).await;

    assert!(
        matches!(effects.status(), ExecutionStatus::Success),
        "got {:?}",
        effects.status()
    );
    assert_record(&env, &effects, AttestationVerdict::Inaccurate);
}

/// The same overclaim on a transaction whose authentication fails after an
/// honest attestation is not assessed: the body never ran, so the executed
/// units measure only the authenticator and say nothing about the claim. The
/// account still pays for the failure.
#[tokio::test]
async fn overclaim_is_not_assessed_when_the_body_never_ran() {
    let _guard = attestation_config(Some(10));

    let mut env = AbstractAccountTestEnv::new().await;
    let tx = env.account_transaction();
    let attestation = env.attest_with_versions(vec![env.account_ref()]);
    env.rotate_owner_key().await;

    let effects = env.submit(tx, Some(attestation)).await;

    assert_move_authentication_error(&effects);
    assert_record(&env, &effects, AttestationVerdict::Valid);
    assert_charged(&effects);
}

#[tokio::test]
async fn unattested_submission_leaves_no_record() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;
    let tx = env.account_transaction();

    let effects = env.submit(tx, None).await;

    assert!(
        matches!(effects.status(), ExecutionStatus::Success),
        "got {:?}",
        effects.status()
    );
    assert_eq!(env.attestation_record(effects.transaction_digest()), None);
}

/// Without an attestation there is no judge to ask, so a gas owner whose
/// authenticator fails is not charged.
#[tokio::test]
async fn unattested_failed_authentication_charges_nobody() {
    let _guard = attestation_config(None);

    let mut env = AbstractAccountTestEnv::new().await;
    let (_, unrelated_key): (_, AccountPrivateKey) = get_key_pair();
    let tx = env.account_transaction_signed_with(&unrelated_key);
    let gas_coin_before = gas_coin(&env, &tx);

    let effects = env.submit(tx, None).await;

    assert_move_authentication_error(&effects);
    let gas_coin_after = env.object(&gas_coin_before.id());
    assert_not_charged(&effects, &gas_coin_before, &gas_coin_after);
    assert_eq!(env.attestation_record(effects.transaction_digest()), None);
}
