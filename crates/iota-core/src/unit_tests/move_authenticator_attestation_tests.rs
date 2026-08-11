// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Direct-execution tests for the attestation misbehavior routing: a
//! Move-authentication failure that reaches execution must resolve to an
//! `InvalidAttestation` failure effect instead of panicking the validator, and
//! the object versions the attestor recorded decide who is charged for it.

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{Address, Command, ExecutionError, ExecutionStatus, ObjectId};
use iota_types::{
    attestation::AttestedObjectVersionReader,
    crypto::get_account_key_pair,
    effects::TransactionEffectsAPI,
    executable_transaction::VerifiedExecutableTransaction,
    move_authenticator::{MoveAuthenticator, MoveAuthenticatorV1},
    object::Object,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    signature::UserSignature,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, Transaction, TransactionData, TransactionDataAPI,
    },
    utils::to_sender_signed_transaction,
};

use super::AttestedObjectVersions;
use crate::{
    authority::test_authority_builder::TestAuthorityBuilder,
    transaction_manager::VerifiedExecutableAttestedTransaction,
};

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
        .try_execute_immediately(
            &VerifiedExecutableAttestedTransaction::new(executable, None),
            None,
            &epoch_store,
        )
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

/// Deciding whether an attested version can be re-run at comes down to the
/// epoch it stopped being current in. Exercised against a real store, with the
/// supersession committed so the answer comes from the database.
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
    // version stops being current and gets recorded as superseded this epoch.
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
        .try_execute_immediately(
            &VerifiedExecutableAttestedTransaction::new(executable, None),
            None,
            &epoch_store,
        )
        .unwrap();
    assert!(
        effects.status().is_success(),
        "the transfer must succeed, got {:?}",
        effects.status()
    );

    // Commit so the lookup is served from the database rather than the cache.
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
        current_epoch,
    };

    assert!(
        !versions_as_of(epoch).superseded_in_current_epoch(&attested_id, current_version),
        "the version the object is still at has not been superseded"
    );
    assert!(
        versions_as_of(epoch).superseded_in_current_epoch(&attested_id, attested_ref.version()),
        "a version this epoch superseded can still be re-run at"
    );
    assert!(
        !versions_as_of(epoch + 1)
            .superseded_in_current_epoch(&attested_id, attested_ref.version()),
        "the same version is out of reach once the superseding epoch has passed"
    );
    assert!(
        !versions_as_of(epoch)
            .superseded_in_current_epoch(&ObjectId::random(), attested_ref.version()),
        "an unknown object has no supersession record"
    );
}

/// Validators flush to the database on their own schedule, so a verdict that
/// depended on whether the supersession had been written back would differ
/// between honest validators and fork the checkpoint. The state must be the
/// same read from the cache and read from the database.
#[tokio::test]
async fn attested_object_version_state_does_not_depend_on_flush_state() {
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
        .try_execute_immediately(
            &VerifiedExecutableAttestedTransaction::new(executable, None),
            None,
            &epoch_store,
        )
        .unwrap();
    assert!(effects.status().is_success());

    let versions = AttestedObjectVersions {
        object_cache: authority.get_object_cache_reader().as_ref(),
        current_epoch: epoch,
    };

    // Nothing has been written back yet, so this is answered from the cache.
    let before_commit = versions.superseded_in_current_epoch(&attested_id, attested_ref.version());
    assert!(
        before_commit,
        "a supersession still in the cache must already count"
    );

    let digests = [*effects.transaction_digest()];
    let cache_commit = authority.get_cache_commit();
    let batch = cache_commit.build_db_batch(epoch, 0, &digests);
    cache_commit.commit_transaction_outputs(epoch, batch, &digests);

    let after_commit = versions.superseded_in_current_epoch(&attested_id, attested_ref.version());
    assert_eq!(
        before_commit, after_commit,
        "the verdict must not change when the supersession reaches the database"
    );
}

/// Deleting the object supersedes the attested version just like overwriting
/// it: the object can no longer be read, but the version it held was superseded
/// this epoch and is still re-runnable rather than being written off as stale.
#[tokio::test]
async fn attested_object_version_state_judges_a_deleted_object() {
    let (sender, sender_key) = get_account_key_pair();

    let attested_id = ObjectId::random();
    let sink_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let objects = vec![
        Object::with_id_owner_for_testing(attested_id, sender),
        Object::with_id_owner_for_testing(sink_id, sender),
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
    let sink_ref = authority.get_object(&sink_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

    // Merging the coin into another deletes it, so the attested version stops
    // being current and is recorded as superseded this epoch.
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        let sink = builder.obj(CallArg::ImmutableOrOwned(sink_ref)).unwrap();
        let merged = builder
            .obj(CallArg::ImmutableOrOwned(attested_ref))
            .unwrap();
        builder.command(Command::new_merge_coins(sink, vec![merged]));
        builder.finish()
    };
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_ref],
        pt,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER * 10,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx_data, &sender_key);
    let verified_tx = epoch_store.verify_transaction(tx).unwrap();
    let executable = VerifiedExecutableTransaction::new_from_checkpoint(verified_tx, epoch, 1);
    let (effects, _execution_error) = authority
        .try_execute_immediately(
            &VerifiedExecutableAttestedTransaction::new(executable, None),
            None,
            &epoch_store,
        )
        .unwrap();
    assert!(
        effects.status().is_success(),
        "the merge must succeed, got {:?}",
        effects.status()
    );

    // Commit so the lookup is served from the database rather than the cache.
    let digests = [*effects.transaction_digest()];
    let cache_commit = authority.get_cache_commit();
    let batch = cache_commit.build_db_batch(epoch, 0, &digests);
    cache_commit.commit_transaction_outputs(epoch, batch, &digests);

    assert!(
        authority.get_object(&attested_id).is_none(),
        "the merge must have deleted the attested object"
    );

    let versions_as_of = |current_epoch| AttestedObjectVersions {
        object_cache: authority.get_object_cache_reader().as_ref(),
        current_epoch,
    };
    assert!(
        versions_as_of(epoch).superseded_in_current_epoch(&attested_id, attested_ref.version()),
        "a version a deletion superseded this epoch can still be re-run at"
    );
    assert!(
        !versions_as_of(epoch + 1)
            .superseded_in_current_epoch(&attested_id, attested_ref.version()),
        "the same version is out of reach once the superseding epoch has passed"
    );
}
