// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for failing a transaction with effects when a check before
//! execution fails, instead of halting the validator. They cover a Move
//! authenticator whose account cannot be resolved, the executor handling
//! `PreExecutionResult::Fail`, and which account failures convert.

use std::{collections::HashSet, sync::Arc};

use iota_sdk_types::{
    Address, ExecutionStatus, MoveAuthenticator, MoveAuthenticatorV1, ObjectId, Owner,
    SharedObjectReference, Transaction, TransactionEffects, UserSignature, VersionAssignment,
};
use iota_types::{
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsAPI,
    error::{ExecutionError, ExecutionErrorKind, IotaError, IotaResult, UserInputError},
    executable_transaction::VerifiedExecutableTransaction,
    execution::PreExecutionResult,
    object::{OBJECT_START_VERSION, Object},
    transaction::{TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI, TransactionEnvelope},
    utils::to_sender_signed_transaction,
};

use super::account_failure_as_execution_error;
use crate::authority::{
    AuthorityState, ExecutionEnv, test_authority_builder::TestAuthorityBuilder,
};

/// An authority and a transaction whose Move authenticator names a shared
/// object with no authenticator function field, so resolving the account fails
/// with `MoveAuthenticatorNotFound`.
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
    sender: Address,
    executable: VerifiedExecutableTransaction,
}

impl UnresolvedAccountSetup {
    /// Builds the authority and the transfer that `account` authenticates.
    async fn new(
        account: Object,
        transfer_id: ObjectId,
        gas_id: ObjectId,
        recipient: Address,
    ) -> Self {
        let account_id = account.id();
        let sender: Address = account_id.into();
        let objects = vec![
            account,
            Object::with_id_owner_for_testing(transfer_id, sender),
            Object::with_id_owner_for_testing(gas_id, sender),
        ];
        let authority = TestAuthorityBuilder::new()
            .with_starting_objects(&objects)
            .build()
            .await;
        let epoch_store = authority.epoch_store_for_testing();
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
        let authenticator = UserSignature::MoveAuthenticator(MoveAuthenticator::from(
            MoveAuthenticatorV1::new_with_shared_account_object(
                vec![],
                vec![],
                SharedObjectReference::new(account_id, OBJECT_START_VERSION, false),
            ),
        ));
        let tx = TransactionEnvelope::from_user_sig_data(tx, vec![authenticator]);
        let verified = epoch_store.verify_transaction(tx).unwrap();
        let executable =
            VerifiedExecutableTransaction::new_from_checkpoint(verified, epoch_store.epoch(), 1);

        Self {
            authority,
            account_id,
            transfer_id,
            sender,
            executable,
        }
    }

    /// Executes the transfer on the authority, skipping the consensus handler.
    fn execute(&self) -> IotaResult<(TransactionEffects, Option<ExecutionError>)> {
        // The account is a shared input, so execution needs the version
        // consensus would have assigned it.
        let env = ExecutionEnv::new().with_assigned_versions(vec![VersionAssignment::new(
            self.account_id,
            OBJECT_START_VERSION,
        )]);

        self.authority.try_execute_immediately(
            &self.executable,
            env,
            &self.authority.epoch_store_for_testing(),
        )
    }
}

/// An account that cannot be resolved fails the transaction with gas charged.
#[tokio::test]
async fn unresolved_authenticator_account_fails_with_effects() {
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let setup = UnresolvedAccountSetup::new(
        Object::shared_for_testing(),
        ObjectId::random(),
        ObjectId::random(),
        recipient,
    )
    .await;

    let (effects, execution_error) = setup.execute().unwrap();

    // The execution engine wraps `FunctionNotFound` in `MoveAuthentication`,
    // as it does any other authenticator failure, so a client cannot
    // distinguish a resolution failure from one the authenticator produced.
    let expected = ExecutionErrorKind::MoveAuthentication {
        error: Box::new(ExecutionErrorKind::FunctionNotFound),
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
    let first = UnresolvedAccountSetup::new(account.clone(), transfer_id, gas_id, recipient).await;
    let second = UnresolvedAccountSetup::new(account, transfer_id, gas_id, recipient).await;
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

/// With `PreExecutionResult::Fail`, a plain transfer transaction skips its
/// commands and is still charged gas. Nothing else is wrong with the transfer
/// transaction, so the failure effects can only come from `Fail`.
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

    let expected = ExecutionErrorKind::CertificateDenied;
    let (inner_store, _, effects, _, result) =
        epoch_store.executor().execute_transaction_to_effects(
            authority.get_backing_store().as_ref(),
            protocol_config,
            authority.metrics.limits_metrics.clone(),
            false,
            &HashSet::new(),
            PreExecutionResult::Fail(ExecutionError::new(expected.clone(), None)),
            &epoch_store.epoch(),
            epoch_store
                .epoch_start_config()
                .epoch_data()
                .epoch_start_timestamp(),
            checked_input_objects,
            gas_data,
            gas_status,
            kind,
            signer,
            *executable.digest(),
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

/// Only the two failures that mean the account cannot be resolved become
/// `FunctionNotFound`; every other failure is returned unchanged.
#[test]
fn only_an_unresolved_account_becomes_a_pre_execution_failure() {
    let account_object_id = ObjectId::random();
    let not_found = || IotaError::UserInput {
        error: UserInputError::MoveAuthenticatorNotFound {
            authenticator_function_ref_id: ObjectId::random(),
            account_object_id,
            account_object_version: OBJECT_START_VERSION,
        },
    };
    let error = account_failure_as_execution_error(not_found()).unwrap();
    assert_eq!(error.kind(), &ExecutionErrorKind::FunctionNotFound);
    assert!(
        format!("{error:?}").contains(&account_object_id.to_string()),
        "the account must still be named by the source"
    );

    let undecodable = IotaError::UserInput {
        error: UserInputError::InvalidAuthenticatorFunctionRefField { account_object_id },
    };
    assert!(account_failure_as_execution_error(undecodable).is_ok());

    // Every other failure means an earlier check was wrong and stays an error,
    // so the execution driver still halts the validator on it.
    for error in [
        UserInputError::AccountObjectNotSupported {
            object_id: account_object_id,
        },
        UserInputError::ImmutableAccountObjectNotSupported {
            object_id: account_object_id,
        },
    ] {
        let expected = format!("{error:?}");
        let returned =
            account_failure_as_execution_error(IotaError::UserInput { error }).unwrap_err();
        assert!(
            format!("{returned:?}").contains(&expected),
            "got {returned:?}"
        );
    }
}
