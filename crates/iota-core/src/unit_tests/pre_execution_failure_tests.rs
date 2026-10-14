// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for failing a transaction with effects when a check before
//! execution fails, instead of halting the validator. They cover a Move
//! authenticator whose account cannot be resolved, both when a sponsor pays gas
//! and when the account pays its own, and the executor handling a resolution
//! failure. With deterministic validation on, they also cover who pays: nobody,
//! when the gas owner is an account that did not authorize the transaction.

use std::{collections::HashSet, sync::Arc};

use iota_protocol_config::{OverrideGuard, ProtocolConfig};
use iota_sdk_types::{
    Address, ExecutionStatus, GasCostSummary, MoveAuthenticator, MoveAuthenticatorV1, ObjectId,
    Owner, SharedObjectReference, Transaction, TransactionEffects, UserSignature,
    VersionAssignment, crypto::Intent,
};
use iota_types::{
    account_abstraction::authenticator_function::MoveAuthenticatorsForExecution,
    auth_context::AuthContextData,
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    error::{ExecutionError, ExecutionErrorKind, IotaResult},
    executable_transaction::VerifiedExecutableTransaction,
    object::{OBJECT_START_VERSION, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI, TransactionEnvelope},
    utils::{to_sender_signed_transaction, to_sender_signed_transaction_with_optional_sponsor},
};

use crate::authority::{
    AuthorityState, ExecutionEnv, test_authority_builder::TestAuthorityBuilder,
};

/// Who pays gas for the transfer of an [`UnresolvedAccountSetup`].
#[derive(Clone, Copy)]
enum GasPayer<'a> {
    /// The account, which sends the transfer.
    Account,
    /// A sponsor that signs, for the account sending the transfer.
    Sponsor(&'a (Address, AccountPrivateKey)),
    /// The account, as the sponsor of a sender that signs.
    AccountAsSponsorOf(&'a (Address, AccountPrivateKey)),
}

/// An authority and a transaction whose Move authenticator names an object
/// the account check rejects: a shared object with no authenticator function
/// field, which fails with `MoveAuthenticatorNotFound`.
///
/// The P-COOL consensus handler cannot keep this check because it answers from
/// its own load of the account, which can differ between validators. Once the
/// check leaves the handler, the transaction reaches execution, where the
/// failure to resolve the account produces failure effects instead of halting
/// the validator. The setup skips the consensus handler and hands the
/// transaction straight to execution, so execution is the first place the
/// account is checked.
struct UnresolvedAccountSetup {
    authority: Arc<AuthorityState>,
    account_id: ObjectId,
    transfer_id: ObjectId,
    gas_id: ObjectId,
    shared_id: Option<ObjectId>,
    sender: Address,
    executable: VerifiedExecutableTransaction,
}

impl UnresolvedAccountSetup {
    /// Builds the authority and the transfer that `account` authenticates, with
    /// gas paid as `gas_payer` says. A `shared` object becomes a mutable input
    /// that no command uses.
    async fn new(
        account: Object,
        transfer_id: ObjectId,
        gas_id: ObjectId,
        recipient: Address,
        gas_payer: GasPayer<'_>,
        shared: Option<Object>,
    ) -> Self {
        let account_id = account.id();
        let account_address: Address = account_id.into();
        let sender = match gas_payer {
            GasPayer::Account | GasPayer::Sponsor(_) => account_address,
            GasPayer::AccountAsSponsorOf((sender, _)) => *sender,
        };
        let gas_owner = match gas_payer {
            GasPayer::Account | GasPayer::AccountAsSponsorOf(_) => account_address,
            GasPayer::Sponsor((sponsor, _)) => *sponsor,
        };
        let shared_id = shared.as_ref().map(|shared| shared.id());
        let mut objects = vec![
            account,
            Object::with_id_owner_for_testing(transfer_id, sender),
            Object::with_id_owner_for_testing(gas_id, gas_owner),
        ];
        objects.extend(shared);
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(&objects)
            .build()
            .await;
        let epoch_store = authority.epoch_store_for_testing();
        let rgp = authority.reference_gas_price_for_testing().unwrap();

        let transfer_ref = authority.get_object(&transfer_id).unwrap().object_ref();
        let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

        let mut builder = ProgrammableTransactionBuilder::new();
        builder.transfer_object(recipient, transfer_ref).unwrap();
        if let Some(shared_id) = shared_id {
            builder
                .obj(CallArg::Shared(SharedObjectReference::new(
                    shared_id,
                    OBJECT_START_VERSION,
                    true,
                )))
                .unwrap();
        }
        let tx = Transaction::new_programmable_allow_sponsor(
            sender,
            vec![gas_ref],
            builder.finish(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER * 10,
            rgp,
            gas_owner,
        );
        let authenticator = MoveAuthenticatorV1::new(
            vec![],
            vec![],
            SharedObjectReference::new(account_id, OBJECT_START_VERSION, false),
        );
        let authenticator =
            UserSignature::MoveAuthenticator(MoveAuthenticator::from(authenticator));
        let tx = match gas_payer {
            GasPayer::Account => to_sender_signed_transaction_with_optional_sponsor(
                tx,
                authenticator,
                None::<&AccountPrivateKey>,
            ),
            GasPayer::Sponsor((_, sponsor_key)) => {
                to_sender_signed_transaction_with_optional_sponsor(
                    tx,
                    authenticator,
                    Some(sponsor_key),
                )
            }
            GasPayer::AccountAsSponsorOf((_, sender_key)) => {
                let sender_signature = TransactionEnvelope::signature_from_signer(
                    tx.clone(),
                    Intent::iota_transaction(),
                    sender_key,
                );
                TransactionEnvelope::from_user_sig_data(
                    tx,
                    vec![sender_signature.into(), authenticator],
                )
            }
        };
        let verified = epoch_store.verify_transaction(tx).unwrap();
        let executable =
            VerifiedExecutableTransaction::new_from_checkpoint(verified, epoch_store.epoch(), 1);

        Self {
            authority,
            account_id,
            transfer_id,
            gas_id,
            shared_id,
            sender,
            executable,
        }
    }

    /// Executes the transfer on the authority, skipping the consensus handler.
    fn execute(&self) -> IotaResult<(TransactionEffects, Option<ExecutionError>)> {
        // Shared inputs, the account included, need the versions consensus
        // would have assigned them.
        let assigned_versions = std::iter::once(self.account_id)
            .chain(self.shared_id)
            .map(|id| VersionAssignment::new(id, OBJECT_START_VERSION))
            .collect();
        let env = ExecutionEnv::new().with_assigned_versions(assigned_versions);

        // With the bookkeeping on, an execution belongs to a commit the
        // consensus handler assigned; here the test assigns it.
        let epoch_store = self.authority.epoch_store_for_testing();
        if epoch_store.pcool_bookkeeping_enabled() {
            epoch_store.assign_commit_to_transactions(1, vec![self.executable.key()]);
        }
        self.authority
            .try_execute_immediately(&self.executable, env, &epoch_store)
    }
}

/// Turns deterministic validation on for authorities built while the returned
/// guard lives.
fn enable_deterministic_validation() -> OverrideGuard {
    ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.enable_pcool_deterministic_validation_for_testing();
        config
    })
}

/// A sender's account that cannot be resolved fails the transaction, with gas
/// charged to the sponsor.
#[tokio::test]
async fn unresolved_authenticator_account_fails_with_effects() {
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let sponsor: (Address, AccountPrivateKey) = get_key_pair();
    let setup = UnresolvedAccountSetup::new(
        Object::shared_for_testing(),
        ObjectId::random(),
        ObjectId::random(),
        recipient,
        GasPayer::Sponsor(&sponsor),
        None,
    )
    .await;

    let (effects, execution_error) = setup.execute().unwrap();

    // The execution engine wraps the status in `MoveAuthentication`, as it
    // does any other authenticator failure; the status inside names the
    // account that failed.
    let expected = ExecutionErrorKind::MoveAuthentication {
        error: Box::new(ExecutionErrorKind::AuthenticatorFunctionNotFound {
            object_id: setup.account_id,
        }),
    };
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: expected.clone(),
            command: None,
        },
        "the failure must happen before any command runs",
    );
    assert_eq!(
        execution_error
            .expect("execution must report the failure")
            .kind(),
        &expected
    );
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the failed transaction must be charged gas"
    );
    assert!(
        effects.created().is_empty(),
        "a failed transaction must not create objects"
    );
    assert_eq!(
        effects.gas_object().owner(),
        &Owner::Address(sponsor.0),
        "the sponsor must be the one charged"
    );
    let transfer = setup.authority.get_object(&setup.transfer_id).unwrap();
    assert_eq!(
        transfer.owner,
        Owner::Address(setup.sender),
        "the transfer must not have run"
    );
}

/// Two validators write the same effects for the same failing transaction.
#[tokio::test]
async fn unresolved_authenticator_account_effects_are_the_same_on_every_validator() {
    let account = Object::shared_for_testing();
    let (transfer_id, gas_id) = (ObjectId::random(), ObjectId::random());
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let sponsor: (Address, AccountPrivateKey) = get_key_pair();
    let first = UnresolvedAccountSetup::new(
        account.clone(),
        transfer_id,
        gas_id,
        recipient,
        GasPayer::Sponsor(&sponsor),
        None,
    )
    .await;
    let second = UnresolvedAccountSetup::new(
        account,
        transfer_id,
        gas_id,
        recipient,
        GasPayer::Sponsor(&sponsor),
        None,
    )
    .await;
    assert_eq!(
        first.executable.digest(),
        second.executable.digest(),
        "both validators must execute the same transaction"
    );

    let (first_effects, _) = first.execute().unwrap();
    let (second_effects, _) = second.execute().unwrap();

    assert_eq!(
        first_effects.digest(),
        second_effects.digest(),
        "both validators must write the same failure effects"
    );
}

/// With deterministic validation off, the gas payer's own account that cannot
/// be resolved fails the transaction too, with gas charged to that account, as
/// for any account whose authentication fails at execution.
#[tokio::test]
async fn unresolved_gas_payer_account_fails_with_effects() {
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let setup = UnresolvedAccountSetup::new(
        Object::shared_for_testing(),
        ObjectId::random(),
        ObjectId::random(),
        recipient,
        GasPayer::Account,
        None,
    )
    .await;

    let (effects, _) = setup.execute().unwrap();

    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: ExecutionErrorKind::MoveAuthentication {
                error: Box::new(ExecutionErrorKind::AuthenticatorFunctionNotFound {
                    object_id: setup.account_id,
                }),
            },
            command: None,
        },
    );
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the failed transaction must be charged gas"
    );
    assert_eq!(
        effects.gas_object().owner(),
        &Owner::Address(setup.sender),
        "the account must be the one charged"
    );
}

/// The status of a transaction whose account `account_id` has no authenticator
/// function.
fn authenticator_function_not_found(account_id: ObjectId) -> ExecutionStatus {
    ExecutionStatus::Failure {
        error: ExecutionErrorKind::MoveAuthentication {
            error: Box::new(ExecutionErrorKind::AuthenticatorFunctionNotFound {
                object_id: account_id,
            }),
        },
        command: None,
    }
}

/// With deterministic validation on, a gas owner whose account cannot be
/// resolved did not authorize the transaction, so nobody is charged. Owned
/// inputs, the gas coin included, keep their versions, so the account can use
/// them again. A mutable shared input still moves to the version consensus
/// assigned, with its contents unchanged.
#[tokio::test]
async fn unresolved_gas_payer_account_is_not_charged_under_deterministic_validation() {
    let _guard = enable_deterministic_validation();
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let setup = UnresolvedAccountSetup::new(
        Object::shared_for_testing(),
        ObjectId::random(),
        ObjectId::random(),
        recipient,
        GasPayer::Account,
        Some(Object::shared_for_testing()),
    )
    .await;
    let shared_id = setup.shared_id.unwrap();
    let object = |id: &ObjectId| setup.authority.get_object(id).unwrap();
    let (gas_before, transfer_before, shared_before) = (
        object(&setup.gas_id),
        object(&setup.transfer_id),
        object(&shared_id),
    );

    let (effects, _) = setup.execute().unwrap();

    assert_eq!(
        effects.status(),
        &authenticator_function_not_found(setup.account_id)
    );
    assert_eq!(
        effects.gas_cost_summary(),
        &GasCostSummary::default(),
        "nobody may be charged"
    );
    assert_eq!(
        effects.as_v1().gas_object(),
        None,
        "the effects must name no gas object"
    );
    assert_eq!(
        effects
            .old_object_metadata()
            .iter()
            .map(|old| old.reference().object_id)
            .collect::<Vec<_>>(),
        vec![shared_id],
        "the mutable shared input must be the only object the effects change"
    );
    assert_eq!(
        object(&setup.gas_id),
        gas_before,
        "the gas coin must keep its version and balance"
    );
    assert_eq!(
        object(&setup.transfer_id),
        transfer_before,
        "the transfer must not have run"
    );
    let shared_after = object(&shared_id);
    assert_eq!(
        shared_after.version(),
        effects.lamport_version(),
        "the shared input must move to the version consensus assigned"
    );
    let contents = |object: &Object| object.data.as_opt_struct().unwrap().contents().to_vec();
    assert_eq!(
        (contents(&shared_after), shared_after.storage_rebate),
        (contents(&shared_before), shared_before.storage_rebate),
        "the shared input's contents and storage rebate must not change"
    );
}

/// Two validators write the same effects for a transaction that charges
/// nobody.
#[tokio::test]
async fn effects_without_a_gas_charge_are_the_same_on_every_validator() {
    let _guard = enable_deterministic_validation();
    let (account, shared) = (Object::shared_for_testing(), Object::shared_for_testing());
    let (transfer_id, gas_id) = (ObjectId::random(), ObjectId::random());
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let first = UnresolvedAccountSetup::new(
        account.clone(),
        transfer_id,
        gas_id,
        recipient,
        GasPayer::Account,
        Some(shared.clone()),
    )
    .await;
    let second = UnresolvedAccountSetup::new(
        account,
        transfer_id,
        gas_id,
        recipient,
        GasPayer::Account,
        Some(shared),
    )
    .await;

    let (first_effects, _) = first.execute().unwrap();
    let (second_effects, _) = second.execute().unwrap();

    assert_eq!(first_effects.gas_cost_summary(), &GasCostSummary::default());
    assert_eq!(
        first_effects.digest(),
        second_effects.digest(),
        "both validators must write the same effects"
    );
}

/// With deterministic validation on, a sponsor that signed the transaction
/// authorized paying for it, so it is still charged when the sender's account
/// cannot be resolved.
#[tokio::test]
async fn signing_sponsor_is_charged_under_deterministic_validation() {
    let _guard = enable_deterministic_validation();
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let sponsor: (Address, AccountPrivateKey) = get_key_pair();
    let setup = UnresolvedAccountSetup::new(
        Object::shared_for_testing(),
        ObjectId::random(),
        ObjectId::random(),
        recipient,
        GasPayer::Sponsor(&sponsor),
        None,
    )
    .await;

    let (effects, _) = setup.execute().unwrap();

    assert_eq!(
        effects.status(),
        &authenticator_function_not_found(setup.account_id)
    );
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the sponsor that signed must be charged"
    );
    assert_eq!(effects.gas_object().owner(), &Owner::Address(sponsor.0));
}

/// With deterministic validation on, the gas owner decides who pays, not the
/// sender: an account sponsoring a sender that signs is not charged when the
/// account cannot be resolved.
#[tokio::test]
async fn unresolved_sponsoring_account_is_not_charged_under_deterministic_validation() {
    let _guard = enable_deterministic_validation();
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let sender: (Address, AccountPrivateKey) = get_key_pair();
    let setup = UnresolvedAccountSetup::new(
        Object::shared_for_testing(),
        ObjectId::random(),
        ObjectId::random(),
        recipient,
        GasPayer::AccountAsSponsorOf(&sender),
        None,
    )
    .await;
    let gas_before = setup.authority.get_object(&setup.gas_id).unwrap();

    let (effects, _) = setup.execute().unwrap();

    assert_eq!(
        effects.status(),
        &authenticator_function_not_found(setup.account_id)
    );
    assert_eq!(
        effects.gas_cost_summary(),
        &GasCostSummary::default(),
        "nobody may be charged"
    );
    assert_eq!(
        setup.authority.get_object(&setup.gas_id).unwrap(),
        gas_before,
        "the account's gas coin must keep its version and balance"
    );
    assert_eq!(
        setup
            .authority
            .get_object(&setup.transfer_id)
            .unwrap()
            .owner,
        Owner::Address(setup.sender),
        "the transfer must not have run"
    );
}

/// Given a resolution failure, a plain transfer transaction skips its
/// commands and is still charged gas. Nothing else is wrong with the transfer
/// transaction, so the failure effects can only come from that failure, which
/// the executor reports like any authenticator failure.
#[tokio::test]
async fn pre_execution_failure_skips_execution_and_charges_gas() {
    let (sender, sender_key): (Address, AccountPrivateKey) = get_key_pair();
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let transfer_id = ObjectId::random();
    let gas_id = ObjectId::random();
    let objects = vec![
        Object::with_id_owner_for_testing(transfer_id, sender),
        Object::with_id_owner_for_testing(gas_id, sender),
    ];
    let authority = TestAuthorityBuilder::new()
        .with_starting_objects(&objects)
        .build()
        .await;
    let epoch_store = authority.epoch_store_for_testing();
    let protocol_config = epoch_store.protocol_config();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let transfer_ref = authority.get_object(&transfer_id).unwrap().object_ref();
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();
    let tx = Transaction::new_transfer(
        recipient,
        transfer_ref,
        sender,
        gas_ref,
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER * 10,
        rgp,
    );
    let tx = to_sender_signed_transaction(tx, &sender_key);
    let verified = epoch_store.verify_transaction(tx).unwrap();
    let executable =
        VerifiedExecutableTransaction::new_from_checkpoint(verified, epoch_store.epoch(), 1);

    let tx_guard = epoch_store.acquire_tx_guard(&executable).unwrap();
    let (input_objects, _) = authority
        .read_objects_for_execution(
            tx_guard.as_lock_guard(),
            &executable,
            ExecutionEnv::new().assigned_versions,
            &epoch_store,
        )
        .unwrap();
    let (gas_status, checked_input_objects) = iota_transaction_checks::check_certificate_input(
        &executable,
        input_objects,
        protocol_config,
        rgp,
    )
    .unwrap();
    let (kind, signer, gas_data) = executable.data().transaction().execution_parts();

    let (sender_auth_digest, sponsor_auth_digest) =
        executable.data().compute_auth_digests().unwrap();
    let auth_context_data = AuthContextData {
        transaction_data_bytes: bcs::to_bytes(executable.data().transaction()).unwrap(),
        sender_auth_digest,
        sponsor_auth_digest,
        sender_authenticator_function_ref: None,
        sponsor_authenticator_function_ref: None,
    };

    let failure = ExecutionErrorKind::FunctionNotFound;
    let expected = ExecutionErrorKind::MoveAuthentication {
        error: Box::new(failure.clone()),
    };
    let (inner_store, _, effects, _, result) = epoch_store
        .executor()
        .authenticate_then_execute_transaction_to_effects(
            authority.get_backing_store().as_ref(),
            protocol_config,
            authority.metrics.limits_metrics.clone(),
            false,
            &HashSet::new(),
            &epoch_store.epoch(),
            epoch_store
                .epoch_start_config()
                .epoch_data()
                .epoch_start_timestamp(),
            gas_data,
            gas_status,
            MoveAuthenticatorsForExecution::ResolutionFailed {
                error: ExecutionError::new(failure, None),
                accounts: vec![],
            },
            checked_input_objects,
            kind,
            signer,
            *executable.digest(),
            auth_context_data,
            &mut None,
        );
    tx_guard.release();

    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: expected.clone(),
            command: None,
        },
        "the failure must happen before any command runs"
    );
    assert_eq!(
        result
            .expect_err("execution must report the failure")
            .kind(),
        &expected
    );
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the failed transaction must be charged gas"
    );
    assert!(
        effects.created().is_empty(),
        "a failed transaction must not create objects"
    );
    for mutated in effects.mutated() {
        assert_eq!(
            mutated.owner(),
            &Owner::Address(sender),
            "the transfer must not have run"
        );
    }
    let mut written: Vec<ObjectId> = inner_store.written.keys().copied().collect();
    written.sort();
    let mut inputs = vec![gas_id, transfer_id];
    inputs.sort();
    assert_eq!(
        written, inputs,
        "a failed transaction rewrites exactly its mutable inputs"
    );
    for object in inner_store.written.values() {
        assert_eq!(
            object.owner,
            Owner::Address(sender),
            "the transfer must not have run"
        );
    }

    let bytes = bcs::to_bytes(&effects).unwrap();
    assert_eq!(
        bcs::from_bytes::<TransactionEffects>(&bytes).expect("failure effects must decode"),
        effects,
        "failure effects must decode to what was encoded"
    );
}
