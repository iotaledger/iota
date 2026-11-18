// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The chain identifier that Move reads with `tx_context::chain_identifier`
//! equals the digest of the genesis checkpoint on every path that builds a
//! transaction context: dev-inspect, a transaction executed by the validators,
//! and an authenticator function in both transaction flows.
//!
//! The tests use the `./tests/chain_identifier` Move package.

use std::path::PathBuf;

use iota_macros::sim_test;
use iota_sdk_types::{
    ExecutionError, ExecutionStatus, Identifier, MoveAuthenticatorV1, MoveLocation, ObjectId,
    ObjectReference, Owner, ProgrammableTransaction, SharedObjectReference, Transaction,
    TransactionKind, TypeTag, UserSignature,
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

const PACKAGE_PATH: &str = "tests/chain_identifier";
const MODULE_NAME: &str = "chain_identifier_account";
const ACCOUNT_NAME: &str = "ChainIdentifierAccount";

#[sim_test]
async fn chain_identifier_read_in_dev_inspect_is_genesis_checkpoint_digest() {
    let test_cluster = TestClusterBuilder::new().build().await;

    let mut builder = ProgrammableTransactionBuilder::new();
    builder.programmable_move_call(
        IOTA_FRAMEWORK_PACKAGE_ID,
        Identifier::from_static("tx_context"),
        Identifier::from_static("chain_identifier"),
        vec![],
        vec![],
    );
    let kind = TransactionKind::new_programmable(builder.finish());

    let results = test_cluster
        .fullnode_handle
        .iota_client
        .read_api()
        .dev_inspect_transaction_block(test_cluster.get_address_0(), kind, None, None, None)
        .await
        .unwrap();
    assert_eq!(results.error, None);
    let call_results = results.results.unwrap();
    let (return_value, _) = &call_results[0].return_values[0];
    let chain_identifier: Vec<u8> = bcs::from_bytes(return_value).unwrap();

    assert_eq!(chain_identifier, genesis_checkpoint_digest(&test_cluster));
}

/// A transaction executed by the validators reads the genesis checkpoint
/// digest, and the check in Move rejects any other value.
#[sim_test]
async fn chain_identifier_read_in_an_executed_transaction_is_genesis_checkpoint_digest() {
    let test_cluster = TestClusterBuilder::new().build().await;
    let package_id = publish_package(&test_cluster.wallet, package_path())
        .await
        .object_id;

    let tx = test_cluster
        .test_transaction_builder()
        .await
        .programmable(assert_chain_identifier_pt(
            package_id,
            genesis_checkpoint_digest(&test_cluster),
        ))
        .build();
    test_cluster.sign_and_execute_transaction(&tx).await;

    let tx = test_cluster
        .test_transaction_builder()
        .await
        .programmable(assert_chain_identifier_pt(package_id, vec![0; 32]))
        .build();
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

/// An authenticator function, and the transaction it authenticates, read the
/// genesis checkpoint digest when the validators authenticate the transaction
/// before consensus.
#[sim_test]
async fn chain_identifier_read_in_an_authenticator_is_genesis_checkpoint_digest_pre_consensus_flow()
{
    authenticator_reads_genesis_checkpoint_digest(false).await;
}

/// As above, with the P-COOL flow.
#[sim_test]
async fn chain_identifier_read_in_an_authenticator_is_genesis_checkpoint_digest_pcool_flow() {
    authenticator_reads_genesis_checkpoint_digest(true).await;
}

async fn authenticator_reads_genesis_checkpoint_digest(pcool: bool) {
    let _pcool_guard = override_pcool_flow(pcool);
    let test_cluster = TestClusterBuilder::new().build().await;
    let package_id = publish_package(&test_cluster.wallet, package_path())
        .await
        .object_id;
    let account = create_account(&test_cluster, package_id).await;

    let rgp = test_cluster.get_reference_gas_price().await;
    let gas = test_cluster
        .fund_address_and_return_gas(rgp, Some(20_000_000_000), account.object_id.into())
        .await;
    let chain_identifier = genesis_checkpoint_digest(&test_cluster);
    let tx = Transaction::new_programmable(
        account.object_id.into(),
        vec![gas],
        assert_chain_identifier_pt(package_id, chain_identifier.clone()),
        rgp * TEST_ONLY_GAS_UNIT_FOR_HEAVY_COMPUTATION_STORAGE,
        rgp,
    );
    let authenticator = UserSignature::MoveAuthenticator(
        MoveAuthenticatorV1::new(
            vec![CallArg::Pure(bcs::to_bytes(&chain_identifier).unwrap())],
            vec![],
            SharedObjectReference::new(account.object_id, account.version, false),
        )
        .into(),
    );

    // Panics unless the transaction executes successfully.
    test_cluster
        .execute_transaction(TransactionEnvelope::from_user_sig_data(
            tx,
            vec![authenticator],
        ))
        .await;
}

/// Creates a `ChainIdentifierAccount` whose authenticator is the package's
/// `authenticate` function, and returns its reference.
async fn create_account(test_cluster: &TestCluster, package_id: ObjectId) -> ObjectReference {
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
        builder.pure("authenticate").unwrap(),
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

/// A transaction that calls `assert_chain_identifier(expected)`.
fn assert_chain_identifier_pt(package_id: ObjectId, expected: Vec<u8>) -> ProgrammableTransaction {
    let mut builder = ProgrammableTransactionBuilder::new();
    let expected = builder.pure(expected).unwrap();
    builder.programmable_move_call(
        package_id,
        Identifier::from_static(MODULE_NAME),
        Identifier::from_static("assert_chain_identifier"),
        vec![],
        vec![expected],
    );
    builder.finish()
}

fn genesis_checkpoint_digest(test_cluster: &TestCluster) -> Vec<u8> {
    test_cluster
        .swarm
        .config()
        .genesis
        .checkpoint()
        .digest()
        .bytes()
        .to_vec()
}

fn package_path() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), PACKAGE_PATH].iter().collect()
}
