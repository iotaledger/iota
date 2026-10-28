// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Unit tests for executing a transaction authenticated by a Move account when
//! consensus has cancelled the transaction.

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::{
    Address, ExecutionStatus, GasCostSummary, MoveAuthenticator, MoveAuthenticatorV1, MoveStruct,
    ObjectId, Owner, SharedObjectReference, StructTag, Transaction, TransactionDigest,
    TransactionEffects, UserSignature, Version, VersionAssignment,
};
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRefV1, derive_authenticator_function_ref_v1_dynamic_field_id,
    },
    crypto::AccountPrivateKey,
    effects::TransactionEffectsAPI,
    error::ExecutionError,
    executable_transaction::VerifiedExecutableTransaction,
    object::{OBJECT_START_VERSION, Object},
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{CallArg, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI},
    utils::to_sender_signed_transaction_with_optional_sponsor,
};

use crate::authority::{ExecutionEnv, test_authority_builder::TestAuthorityBuilder};

/// Returns the dynamic field object holding `account`'s authenticator function
/// reference.
fn authenticator_function_ref_field(account: &Object) -> Object {
    let account_id = account.id();
    let field_id = derive_authenticator_function_ref_v1_dynamic_field_id(account_id).unwrap();
    let account_type = account.struct_tag().unwrap();
    let field_type = StructTag::new_dynamic_field(
        StructTag::new_authenticator_function_ref_v1_key(),
        AuthenticatorFunctionRefV1::type_(account_type),
    );
    let authenticator_function_ref = AuthenticatorFunctionRefV1 {
        package: IOTA_FRAMEWORK_PACKAGE_ID,
        module: "authenticator".to_string(),
        function: "authenticate".to_string(),
    };
    // `Field { id: UID, name: AuthenticatorFunctionRefV1Key, value }`, where the
    // key is an empty struct encoded as one `bool`.
    let contents = bcs::to_bytes(&(field_id, false, authenticator_function_ref)).unwrap();
    let field = MoveStruct::new(field_type.into(), OBJECT_START_VERSION, contents).unwrap();
    Object::new_move(
        field,
        Owner::Object(account_id),
        TransactionDigest::GENESIS_MARKER,
    )
}

/// Executes a transaction that uses `Random` and is sent by a Move account,
/// which also pays for gas, with the versions consensus assigns when it
/// cancels the transaction because randomness is unavailable.
async fn execute_cancelled_move_account_transaction() -> (TransactionEffects, Option<ExecutionError>)
{
    let account = Object::shared_for_testing();
    let account_id = account.id();
    let sender: Address = account_id.into();
    let gas_id = ObjectId::random();
    let objects = vec![
        authenticator_function_ref_field(&account),
        account,
        Object::with_id_owner_for_testing(gas_id, sender),
    ];
    let authority = TestAuthorityBuilder::new()
        .with_starting_objects(&objects)
        .build()
        .await;
    let epoch_store = authority.epoch_store_for_testing();
    let rgp = authority.reference_gas_price_for_testing().unwrap();

    let randomness_state = authority.get_object(&ObjectId::RANDOMNESS_STATE).unwrap();
    let Owner::Shared(randomness_initial_version) = randomness_state.owner else {
        panic!("the randomness state must be a shared object");
    };
    let gas_ref = authority.get_object(&gas_id).unwrap().object_ref();

    let mut builder = ProgrammableTransactionBuilder::new();
    builder
        .input(CallArg::Shared(SharedObjectReference::new(
            ObjectId::RANDOMNESS_STATE,
            randomness_initial_version,
            false,
        )))
        .unwrap();
    let tx = Transaction::new_programmable_allow_sponsor(
        sender,
        vec![gas_ref],
        builder.finish(),
        rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER * 10,
        rgp,
        sender,
    );
    let authenticator = MoveAuthenticatorV1::new(
        vec![],
        vec![],
        SharedObjectReference::new(account_id, OBJECT_START_VERSION, false),
    );
    let authenticator = UserSignature::MoveAuthenticator(MoveAuthenticator::from(authenticator));
    let tx = to_sender_signed_transaction_with_optional_sponsor(
        tx,
        authenticator,
        None::<&AccountPrivateKey>,
    );
    let verified = epoch_store.verify_transaction(tx).unwrap();
    let executable =
        VerifiedExecutableTransaction::new_from_checkpoint(verified, epoch_store.epoch(), 1);

    // The versions consensus assigns to a transaction it cancels because
    // randomness is unavailable.
    let env = ExecutionEnv::new().with_assigned_versions(vec![
        VersionAssignment::new(ObjectId::RANDOMNESS_STATE, Version::RANDOMNESS_UNAVAILABLE),
        VersionAssignment::new(account_id, Version::CANCELED_READ),
    ]);
    // With deterministic validation, a committee validator executes only
    // transactions of commits its consensus handler assigned.
    epoch_store.assign_commit_to_transactions(1, vec![executable.key()]);
    authority
        .try_execute_immediately(&executable, env, &epoch_store)
        .unwrap()
}

/// Without the P-COOL flow, as on Mainnet and Testnet, a transaction that uses
/// `Random` and is sent by a Move account gets the usual cancellation effects
/// when consensus cancels it because randomness is unavailable.
#[tokio::test]
async fn cancelled_move_account_transaction_gets_cancellation_effects() {
    let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
        config.set_enable_pcool_flow_for_testing(false);
        config
    });
    let (effects, execution_error) = execute_cancelled_move_account_transaction().await;
    assert!(
        matches!(
            effects.status(),
            ExecutionStatus::Failure { error, .. }
                if error.is_execution_canceled_due_to_randomness_unavailable()
        ),
        "expected cancellation effects, got {:?}",
        effects.status()
    );
    assert!(execution_error.is_some());
}

/// With the P-COOL flow, the account's authenticator does not run for
/// a cancelled transaction either, so the account, which pays for gas, did not
/// authorize it: the transaction gets the cancellation effects and nobody is
/// charged.
#[tokio::test]
async fn cancelled_move_account_transaction_charges_nobody_with_the_pcool_flow() {
    let (effects, _) = execute_cancelled_move_account_transaction().await;

    assert!(
        matches!(
            effects.status(),
            ExecutionStatus::Failure { error, .. }
                if error.is_execution_canceled_due_to_randomness_unavailable()
        ),
        "expected cancellation effects, got {:?}",
        effects.status()
    );
    assert_eq!(effects.gas_cost_summary(), &GasCostSummary::default());
}
