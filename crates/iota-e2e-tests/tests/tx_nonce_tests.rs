// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The nonce that Move reads with `tx_context::tx_nonce` is the SDK's
//! `GasPayment::nonce` of the transaction in a transaction executed by the
//! validators, whether an address or an account sends it, and an
//! authenticator function can't read one in either transaction flow.
//!
//! The tests use the `./tests/tx_nonce` Move package.

use std::path::PathBuf;

use iota_macros::sim_test;
use iota_sdk_types::{
    Address, ExecutionError, ExecutionStatus, GasPayment, Identifier, MoveAuthenticatorV1,
    MoveLocation, ObjectId, ObjectReference, Owner, ProgrammableTransaction, SharedObjectReference,
    Transaction, TypeTag, UserSignature,
};
use iota_test_transaction_builder::publish_package;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    effects::TransactionEffectsAPI,
    move_package,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{
        CallArg, TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE, TransactionAPI,
        TransactionEnvelope,
    },
};
use test_cluster::{TestCluster, TestClusterBuilder, override_pcool_flow};

const PACKAGE_PATH: &str = "tests/tx_nonce";
const MODULE_NAME: &str = "tx_nonce_account";
const ACCOUNT_NAME: &str = "TxNonceAccount";

/// Abort code of `iota::tx_context::native_tx_nonce` when the transaction has
/// no nonce.
const NO_TX_NONCE_ABORT_CODE: u64 = 3;

/// Every command of a transaction executed by the validators reads the SDK's
/// nonce of its gas payment, and the check in Move rejects any other value.
#[sim_test]
async fn tx_nonce_read_in_an_executed_transaction_is_the_sdk_nonce() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let package_id = publish_package(&test_cluster.wallet, package_path())
        .await
        .object_id;
    let sender = test_cluster.get_address_0();
    let rgp = test_cluster.get_reference_gas_price().await;

    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let nonce = sdk_nonce(sender, gas, rgp);
    let tx = Transaction::new_programmable(
        sender,
        vec![gas],
        assert_tx_nonce_twice_pt(package_id, nonce.to_vec()),
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    test_cluster.sign_and_execute_transaction(&tx).await;

    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let tx = Transaction::new_programmable(
        sender,
        vec![gas],
        assert_tx_nonce_twice_pt(package_id, vec![0; 32]),
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let (effects, _) = test_cluster
        .execute_transaction_return_raw_effects(test_cluster.wallet.sign_transaction(&tx))
        .await
        .unwrap();
    assert!(
        matches!(
            effects.status(),
            ExecutionStatus::Failure {
                error: ExecutionError::MoveAbort {
                    location: MoveLocation { module, .. },
                    ..
                },
                ..
            } if module.as_str() == MODULE_NAME
        ),
        "expected an abort in {MODULE_NAME}, got {:?}",
        effects.status()
    );
}

/// Every command of a transaction sent by an account, whose authenticator
/// does not read the nonce, reads the SDK's nonce of its gas payment when the
/// validators authenticate the transaction before consensus.
#[sim_test]
async fn tx_nonce_read_in_a_transaction_sent_by_an_account_is_the_sdk_nonce_pre_consensus_flow() {
    transaction_sent_by_an_account_reads_the_sdk_nonce(false).await;
}

/// As above, with the P-COOL flow.
#[sim_test]
async fn tx_nonce_read_in_a_transaction_sent_by_an_account_is_the_sdk_nonce_pcool_flow() {
    transaction_sent_by_an_account_reads_the_sdk_nonce(true).await;
}

async fn transaction_sent_by_an_account_reads_the_sdk_nonce(pcool: bool) {
    let _pcool_guard = override_pcool_flow(pcool);
    let (test_cluster, tx) = transaction_of_an_account("authenticate_without_tx_nonce").await;

    // Panics unless the transaction executes successfully.
    test_cluster.execute_transaction(tx).await;
}

/// An authenticator function can't read a nonce, so the validators reject
/// the transaction when they authenticate it before consensus.
#[sim_test]
async fn tx_nonce_read_in_an_authenticator_aborts_pre_consensus_flow() {
    authenticator_that_reads_the_nonce_is_rejected(false).await;
}

/// As above, with the P-COOL flow, where the validators also run the
/// authenticator when the transaction is submitted.
#[sim_test]
async fn tx_nonce_read_in_an_authenticator_aborts_pcool_flow() {
    authenticator_that_reads_the_nonce_is_rejected(true).await;
}

async fn authenticator_that_reads_the_nonce_is_rejected(pcool: bool) {
    let _pcool_guard = override_pcool_flow(pcool);
    let (test_cluster, tx) = transaction_of_an_account("authenticate").await;

    let error = test_cluster
        .execute_transaction_return_raw_effects(tx)
        .await
        .expect_err("the validators must reject the transaction")
        .to_string();

    assert!(
        error.contains("tx_context::native_tx_nonce")
            && error.contains(&format!("Abort Code: {NO_TX_NONCE_ABORT_CODE}")),
        "expected the abort of tx_context::native_tx_nonce, got {error}"
    );
}

/// Starts a cluster and returns a transaction sent by a `TxNonceAccount`
/// whose authenticator is the package's `authenticator_function`. The
/// transaction calls `assert_tx_nonce` with the SDK's nonce of its gas payment
/// in two commands.
async fn transaction_of_an_account(
    authenticator_function: &str,
) -> (TestCluster, TransactionEnvelope) {
    let test_cluster = TestClusterBuilder::new().build().await;
    let package_id = publish_package(&test_cluster.wallet, package_path())
        .await
        .object_id;
    let account = create_account(&test_cluster, package_id, authenticator_function).await;
    let sender: Address = account.object_id.into();

    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), sender)
        .await;
    let nonce = sdk_nonce(sender, gas, rgp);
    let tx = Transaction::new_programmable(
        sender,
        vec![gas],
        assert_tx_nonce_twice_pt(package_id, nonce.to_vec()),
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let authenticator = UserSignature::MoveAuthenticator(
        MoveAuthenticatorV1::new(
            vec![],
            vec![],
            SharedObjectReference::new(account.object_id, account.version, false),
        )
        .into(),
    );

    (
        test_cluster,
        TransactionEnvelope::from_user_sig_data(tx, vec![authenticator]),
    )
}

/// Creates a `TxNonceAccount` whose authenticator is the package's
/// `authenticator_function`, and returns its reference.
async fn create_account(
    test_cluster: &TestCluster,
    package_id: ObjectId,
    authenticator_function: &str,
) -> ObjectReference {
    let package_metadata = test_cluster
        .get_latest_object_ref(&move_package::derive_package_metadata_id(package_id))
        .await;
    let account_type: TypeTag = format!("{package_id}::{MODULE_NAME}::{ACCOUNT_NAME}")
        .parse()
        .unwrap();

    let mut builder = ProgrammableTransactionBuilder::new();
    let arguments = vec![
        builder
            .obj(CallArg::ImmutableOrOwned(package_metadata))
            .unwrap(),
        builder.pure(MODULE_NAME).unwrap(),
        builder.pure(authenticator_function).unwrap(),
    ];
    let authenticator = builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("authenticator_function"),
        Identifier::from_static("create_auth_function_ref_v1"),
        vec![account_type],
        arguments,
    );
    builder.programmable_move_call(
        package_id,
        Identifier::from_static(MODULE_NAME),
        Identifier::from_static("create"),
        vec![],
        vec![authenticator],
    );
    let tx = test_cluster
        .test_transaction_builder()
        .await
        .programmable(builder.finish())
        .build();
    let effects = test_cluster.sign_and_execute_transaction(&tx).await;

    // The account is the only shared object the transaction creates.
    *effects
        .created()
        .iter()
        .find(|created| matches!(created.owner(), Owner::Shared(_)))
        .unwrap()
        .reference()
}

/// A transaction that calls `assert_tx_nonce(expected)` in two commands.
fn assert_tx_nonce_twice_pt(package_id: ObjectId, expected: Vec<u8>) -> ProgrammableTransaction {
    let mut builder = ProgrammableTransactionBuilder::new();
    let expected = builder.pure(expected).unwrap();
    for _ in 0..2 {
        builder.programmable_move_call(
            package_id,
            Identifier::from_static(MODULE_NAME),
            Identifier::from_static("assert_tx_nonce"),
            vec![],
            vec![expected],
        );
    }
    builder.finish()
}

/// The SDK's nonce of a transaction paid by `sender` with `gas`.
fn sdk_nonce(sender: Address, gas: ObjectReference, rgp: u64) -> [u8; 32] {
    GasPayment {
        objects: vec![gas],
        owner: sender,
        price: rgp,
        budget: rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
    }
    .nonce()
    .expect("a gas payment with a gas coin has a nonce")
}

fn package_path() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), PACKAGE_PATH].iter().collect()
}
