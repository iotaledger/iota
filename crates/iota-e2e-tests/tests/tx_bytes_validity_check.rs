// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_macros::sim_test;
use iota_protocol_config::ProtocolVersion;
use iota_sdk_types::{
    Command, ExecutionError, ExecutionStatus, Identifier, ObjectId, ProgrammableTransaction,
    TransactionKind,
};
use iota_types::{
    effects::TransactionEffectsAPI,
    error::{IotaError, UserInputError},
    transaction::CallArg,
};
use test_cluster::TestClusterBuilder;

// Build an invalid raw transaction byte sequence for sending in through the raw
// API.
//
// Most user facing APIs/clients/tools bar the user from even being able to
// construct an invalid transaction byte sequence.
// But, with enough determination/or coding error they can and the system must
// reject these.
// Prior to protocol version 9 faulty transactions sequences could have been
// accepted as valid, but got rejected during execution. Logging them on-chain,
// enforcing other tools the need to handle such invalid transactions.
// Since protocol version 10 these transactions should be rejected outright.
//
// For the purposes of this discussion an invalid transaction byte sequence is,
// which contains an invalid module or function name identifier. Ex:
// iota::clock::timestamp_ms -> iota::_::timestamp_ms
//
fn build_faulty_transaction_kind() -> TransactionKind {
    let inputs = vec![CallArg::CLOCK_IMMUTABLE];
    // In case the ProgrammableMoveCall API is fixed such that it does not
    // accept invalid inputs and there are no other easily accessible interfaces
    // for constructing invalid transaction byte sequences, then serialize one
    // out and put it into the test here.
    // Even if there is no easy interface for such things, we must protect against
    // as long as there are user facing interfaces that can accept raw transactional
    // bytes.
    let commands = vec![Command::new_move_call(
        ObjectId::FRAMEWORK,
        Identifier::new_unchecked("_"),
        Identifier::new_unchecked("timestamp_ms"),
        vec![],
        vec![iota_sdk_types::Argument::Input(0)],
    )];
    let pt = ProgrammableTransaction { inputs, commands };
    TransactionKind::new_programmable(pt)
}

#[sim_test]
async fn version_9_accepts() {
    let test_cluster = TestClusterBuilder::new()
        .with_protocol_version(ProtocolVersion::new(9))
        .build()
        .await;

    let sender = test_cluster.get_address_0();
    let txn = build_faulty_transaction_kind();

    let (effects, ..) = test_cluster
        .dev_inspect_transaction_kind(sender, txn)
        .await
        .expect("transaction should have been considered valid");

    assert!(
        matches!(
            effects.status(),
            ExecutionStatus::Failure {
                error: ExecutionError::VmVerificationOrDeserializationError,
                command: Some(0),
            }
        ),
        "expected a bytecode-verification failure in command 0, got {:?}",
        effects.status()
    );
}

#[sim_test]
async fn above_version_9_it_fails() {
    let test_cluster = TestClusterBuilder::new().build().await;

    let sender = test_cluster.get_address_0();
    let txn = build_faulty_transaction_kind();

    let err = test_cluster
        .dev_inspect_transaction_kind(sender, txn)
        .await
        .expect_err("transaction should have been considered invalid");

    assert!(
        matches!(
            &err,
            IotaError::UserInput {
                error: UserInputError::InvalidIdentifier { error },
            } if error == "_"
        ),
        "unexpected error: {err}"
    );
}
