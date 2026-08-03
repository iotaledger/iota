// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Direct-execution tests for the attestation misbehavior routing: a
//! Move-authentication failure that reaches execution must resolve to an
//! `InvalidAttestation` failure effect instead of panicking the validator, and
//! the object versions the attestor recorded decide who is charged for it.

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, ExecutionError, ExecutionStatus, ObjectId};
use iota_types::{
    attestation::{AttestedObjectVersionReader, AttestedObjectVersionState},
    crypto::get_account_key_pair,
    effects::TransactionEffectsAPI,
    executable_transaction::VerifiedExecutableTransaction,
    move_authenticator::{MoveAuthenticator, MoveAuthenticatorV1},
    object::Object,
    signature::UserSignature,
    transaction::{
        TEST_ONLY_GAS_UNIT_FOR_TRANSFER, Transaction, TransactionData, TransactionDataAPI,
    },
    utils::to_sender_signed_transaction,
};

use super::AttestedObjectVersions;
use crate::authority::test_authority_builder::TestAuthorityBuilder;

/// A transaction whose Move authenticator points at an object that is not an
/// abstract account — here an immutable object with no
/// authenticator-function-ref field — fails authentication structurally at
/// execution (`MoveAuthenticatorNotFound`). Under the attestation flow this
/// must resolve to an `InvalidAttestation` failure effect (issuer charged gas,
/// validator does not panic) rather than the previous `.expect()` halt,
/// generalizing iota#12375.
///
/// The transaction is executed directly, bypassing the attestor's own
/// authentication check, to reproduce a transaction that reached execution
/// despite failing authentication.
#[tokio::test]
async fn structural_move_auth_failure_resolves_to_invalid_attestation() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(true);
        config.set_enable_validator_attestation_for_testing(true);
        config
    });

    // The object being authenticated is immutable and has no authenticator
    // function ref, so `check_move_account` fails with `MoveAuthenticatorNotFound`
    // at execution. The transaction's sender is the object's id, since
    // `verify_claims` requires the author to equal the authenticator's address.
    let account_id = ObjectId::random();
    let sender: Address = account_id.into();

    let transfer_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let objects = vec![
        Object::immutable_with_id_for_testing(account_id),
        Object::with_id_owner_for_testing(transfer_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ];
    let authority = TestAuthorityBuilder::new()
        .with_starting_objects(&objects)
        .build()
        .await;

    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let account_ref = authority.get_object(&account_id).unwrap().object_ref();
    let transfer_ref = authority.get_object(&transfer_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

    // A transfer body that never runs: authentication fails first.
    let (recipient, _) = get_account_key_pair();
    let tx_data = TransactionData::new_transfer(
        recipient,
        transfer_ref,
        sender,
        gas_ref,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER * 10,
        rgp,
    );

    // Sign with a Move authenticator over the immutable object. `verify_claims`
    // accepts it (author == authenticator address); the account check fails at
    // execution.
    let authenticator = UserSignature::MoveAuthenticator(MoveAuthenticator::V1(
        MoveAuthenticatorV1::new_with_immutable_account_object(vec![], vec![], account_ref),
    ));
    let tx = Transaction::from_user_sig_data(tx_data, vec![authenticator]);

    let verified_tx = epoch_store.verify_transaction(tx).unwrap();
    let executable =
        VerifiedExecutableTransaction::new_from_checkpoint(verified_tx, epoch_store.epoch(), 1);

    let (effects, _execution_error) = authority
        .try_execute_immediately(&executable.into(), None, &epoch_store)
        .unwrap();

    let ExecutionStatus::Failure { error, .. } = effects.status() else {
        panic!("expected an execution failure, got {:?}", effects.status());
    };
    assert!(
        matches!(error, ExecutionError::InvalidAttestation),
        "a structural Move-authentication failure must resolve to InvalidAttestation, got {error:?}"
    );
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the issuer must be charged gas for the failed execution"
    );
}

/// Deciding whether an attested version can be re-run at comes down to finding
/// the transaction that superseded it and reading the epoch it executed in.
/// Exercised against a real store, since the successor lookup reads the
/// database directly while the current version is served from the cache.
#[tokio::test]
async fn attested_object_version_state_follows_the_superseding_transaction() {
    let (sender, sender_key) = get_account_key_pair();
    let (recipient, _) = get_account_key_pair();

    let attested_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let objects = vec![
        Object::with_id_owner_for_testing(attested_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ];
    let authority = TestAuthorityBuilder::new()
        .with_starting_objects(&objects)
        .build()
        .await;

    let epoch_store = authority.epoch_store_for_testing();
    let epoch = epoch_store.epoch();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let attested_ref = authority.get_object(&attested_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

    // Transferring the object writes a new version of it, so the attested
    // version stops being current and gains the successor the reader looks for.
    let tx_data = TransactionData::new_transfer(
        recipient,
        attested_ref,
        sender,
        gas_ref,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER * 10,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);
    let verified_tx = epoch_store.verify_transaction(tx).unwrap();
    let executable = VerifiedExecutableTransaction::new_from_checkpoint(verified_tx, epoch, 1);
    let (effects, _execution_error) = authority
        .try_execute_immediately(&executable.into(), None, &epoch_store)
        .unwrap();
    assert!(
        effects.status().is_success(),
        "the transfer must succeed, got {:?}",
        effects.status()
    );

    // Without this the successor is still cache-only and the reader would take
    // its not-yet-written-back branch instead of the lookup under test.
    let digests = [*effects.transaction_digest()];
    let cache_commit = authority.get_cache_commit();
    let batch = cache_commit.build_db_batch(epoch, 0, &digests);
    cache_commit.commit_transaction_outputs(epoch, batch, &digests);

    let current_version = authority.get_object(&attested_id).unwrap().version();
    assert_ne!(
        current_version,
        attested_ref.version(),
        "the transfer must have superseded the attested version"
    );

    let versions_as_of = |current_epoch| AttestedObjectVersions {
        object_cache: authority.get_object_cache_reader().as_ref(),
        transaction_cache: authority.get_transaction_cache_reader().as_ref(),
        perpetual_tables: &authority.perpetual_tables,
        current_epoch,
    };

    assert_eq!(
        versions_as_of(epoch).attested_object_version_state(&attested_id, current_version),
        AttestedObjectVersionState::Current,
        "the version the object is still at is current"
    );
    assert_eq!(
        versions_as_of(epoch).attested_object_version_state(&attested_id, attested_ref.version()),
        AttestedObjectVersionState::SupersededInCurrentEpoch,
        "a version this epoch superseded can still be re-run at"
    );
    assert_eq!(
        versions_as_of(epoch + 1)
            .attested_object_version_state(&attested_id, attested_ref.version()),
        AttestedObjectVersionState::Stale,
        "the same version is out of reach once the superseding epoch has passed"
    );
    assert_eq!(
        versions_as_of(epoch)
            .attested_object_version_state(&ObjectId::random(), attested_ref.version()),
        AttestedObjectVersionState::Stale,
        "an object that cannot be read leaves the attestation unproven"
    );
}
