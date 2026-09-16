// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A failure decided before a transaction runs ends in failure effects that
//! charge gas, not in an error that halts the validator.

use std::{collections::HashSet, sync::Arc};

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, ExecutionStatus, MoveAuthenticator, MoveAuthenticatorV1, ObjectId, Owner, Transaction,
    TransactionEffects, UserSignature,
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

use super::unresolved_account_failure;
use crate::authority::{
    AuthorityState, ExecutionEnv, test_authority_builder::TestAuthorityBuilder,
};

/// A transaction whose Move authenticator names an immutable object that has
/// no authenticator function field, so resolving the account fails at
/// execution with `MoveAuthenticatorNotFound`. The transaction is executed
/// directly, which is how such a transaction reaches execution once the
/// account checks leave the consensus handler.
struct UnresolvedAccountScenario {
    authority: Arc<AuthorityState>,
    account_id: ObjectId,
    transfer_id: ObjectId,
    sender: Address,
    executable: VerifiedExecutableTransaction,
}

impl UnresolvedAccountScenario {
    async fn new(
        account_id: ObjectId,
        transfer_id: ObjectId,
        gas_id: ObjectId,
        recipient: Address,
    ) -> Self {
        let sender: Address = account_id.into();
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

        let tx = Transaction::new_transfer(
            recipient,
            transfer_ref,
            sender,
            gas_ref,
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER * 10,
            rgp,
        );
        let authenticator = UserSignature::MoveAuthenticator(MoveAuthenticator::from(
            MoveAuthenticatorV1::new_with_immutable_account_object(vec![], vec![], account_ref),
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

    fn execute(&self) -> IotaResult<(TransactionEffects, Option<ExecutionError>)> {
        self.authority.try_execute_immediately(
            &self.executable,
            ExecutionEnv::new(),
            &self.authority.epoch_store_for_testing(),
        )
    }
}

#[tokio::test]
async fn unresolved_authenticator_account_fails_with_effects() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_pcool_pre_execution_failure_effects_for_testing(true);
        config
    });
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let scenario = UnresolvedAccountScenario::new(
        ObjectId::random(),
        ObjectId::random(),
        ObjectId::random(),
        recipient,
    )
    .await;

    let (effects, execution_error) = scenario.execute().unwrap();

    let expected = ExecutionErrorKind::MoveAuthenticatorAccountUnresolved {
        account_object_id: scenario.account_id,
    };
    assert_eq!(
        effects.status(),
        &ExecutionStatus::Failure {
            error: expected.clone(),
            command: None,
        }
    );
    assert_eq!(execution_error.unwrap().kind(), &expected);
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the failed transaction must be charged gas"
    );
    assert!(effects.created().is_empty());
    let transfer = scenario
        .authority
        .get_object(&scenario.transfer_id)
        .unwrap();
    assert_eq!(
        transfer.owner,
        Owner::Address(scenario.sender),
        "the transfer must not have run"
    );
}

#[tokio::test]
async fn unresolved_authenticator_account_is_an_error_without_the_flag() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_pcool_pre_execution_failure_effects_for_testing(false);
        config
    });
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let scenario = UnresolvedAccountScenario::new(
        ObjectId::random(),
        ObjectId::random(),
        ObjectId::random(),
        recipient,
    )
    .await;

    let error = scenario.execute().unwrap_err();

    let IotaError::UserInput {
        error:
            UserInputError::MoveAuthenticatorNotFound {
                account_object_id, ..
            },
    } = error
    else {
        panic!("got {error:?}");
    };
    assert_eq!(account_object_id, scenario.account_id);
    // Nothing is written: the execution driver halts on this error instead.
    assert!(
        scenario
            .authority
            .get_transaction_cache_reader()
            .try_get_executed_effects(scenario.executable.digest())
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn unresolved_authenticator_account_effects_are_the_same_on_every_validator() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_pcool_pre_execution_failure_effects_for_testing(true);
        config
    });
    let (account_id, transfer_id, gas_id) =
        (ObjectId::random(), ObjectId::random(), ObjectId::random());
    let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
    let first = UnresolvedAccountScenario::new(account_id, transfer_id, gas_id, recipient).await;
    let second = UnresolvedAccountScenario::new(account_id, transfer_id, gas_id, recipient).await;
    assert_eq!(first.executable.digest(), second.executable.digest());

    let (first_effects, _) = first.execute().unwrap();
    let (second_effects, _) = second.execute().unwrap();

    assert_eq!(first_effects.digest(), second_effects.digest());
}

/// A `PreExecutionResult::Fail` alone must turn a transaction that would
/// otherwise succeed into failure effects with the given status, gas charged
/// and nothing else changed.
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

    let expected = ExecutionErrorKind::ReceivingObjectMismatch {
        object_id: transfer_id,
    };
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
        }
    );
    assert_eq!(result.unwrap_err().kind(), &expected);
    assert!(
        effects.gas_cost_summary().gas_used() > 0,
        "the failed transaction must be charged gas"
    );
    assert!(effects.created().is_empty());
    for mutated in effects.mutated() {
        assert_eq!(
            mutated.owner(),
            &Owner::Address(sender),
            "no object may change hands"
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
            "no object may change hands"
        );
    }

    let bytes = bcs::to_bytes(&effects).unwrap();
    assert_eq!(
        bcs::from_bytes::<TransactionEffects>(&bytes).unwrap(),
        effects
    );
}

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
    let mut protocol_config = ProtocolConfig::get_for_max_version_UNSAFE();
    protocol_config.set_pcool_pre_execution_failure_effects_for_testing(true);

    let error =
        unresolved_account_failure(account_object_id, not_found(), &protocol_config).unwrap();
    assert_eq!(
        error.kind(),
        &ExecutionErrorKind::MoveAuthenticatorAccountUnresolved { account_object_id }
    );

    let undecodable = IotaError::UserInput {
        error: UserInputError::InvalidAuthenticatorFunctionRefField { account_object_id },
    };
    assert!(unresolved_account_failure(account_object_id, undecodable, &protocol_config).is_ok());

    // Any other failure means an earlier check was wrong and stays an error.
    let not_supported = IotaError::UserInput {
        error: UserInputError::AccountObjectNotSupported {
            object_id: account_object_id,
        },
    };
    assert!(matches!(
        unresolved_account_failure(account_object_id, not_supported, &protocol_config),
        Err(IotaError::UserInput {
            error: UserInputError::AccountObjectNotSupported { .. }
        })
    ));

    protocol_config.set_pcool_pre_execution_failure_effects_for_testing(false);
    assert!(matches!(
        unresolved_account_failure(account_object_id, not_found(), &protocol_config),
        Err(IotaError::UserInput {
            error: UserInputError::MoveAuthenticatorNotFound { .. }
        })
    ));
}

/// The two statuses are appended at fixed positions of the on-chain enum, and
/// the position is the byte every stored effect starts its status with.
#[test]
fn new_failure_statuses_keep_their_bcs_discriminants() {
    let object_id = ObjectId::random();
    for (status, discriminant) in [
        (
            ExecutionErrorKind::ReceivingObjectMismatch { object_id },
            43u8,
        ),
        (
            ExecutionErrorKind::MoveAuthenticatorAccountUnresolved {
                account_object_id: object_id,
            },
            44u8,
        ),
    ] {
        let bytes = bcs::to_bytes(&status).unwrap();
        assert_eq!(bytes[0], discriminant, "{status:?}");
        assert_eq!(bytes.len(), 1 + ObjectId::LENGTH, "{status:?}");
        assert_eq!(
            bcs::from_bytes::<ExecutionErrorKind>(&bytes).unwrap(),
            status
        );
    }
}
