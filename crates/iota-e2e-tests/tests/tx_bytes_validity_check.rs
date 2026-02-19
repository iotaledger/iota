// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::encoding::Base64;
use iota_json_rpc_api::WriteApiClient;
use iota_json_rpc_types::{IotaExecutionStatus, IotaTransactionBlockEffectsAPI};
use iota_macros::sim_test;
use iota_protocol_config::ProtocolVersion;
use iota_types::{
    IOTA_FRAMEWORK_PACKAGE_ID,
    transaction::{CallArg, ProgrammableMoveCall, ProgrammableTransaction, TransactionKind},
};
use jsonrpsee::{core::ClientError, types::ErrorCode};
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
fn build_faulty_transaction_byte_sequence() -> Base64 {
    let inputs = vec![CallArg::CLOCK_IMM];
    // In case the ProgrammableMoveCall API is fixed such that it does not
    // accept invalid inputs and there are no other easily accessible interfaces
    // for constructing invalid transaction byte sequences, then serialize one
    // out and put it into the test here.
    // Even if there is no easy interface for such things, we must protect against
    // as long as there are user facing interfaces that can accept raw transactional
    // bytes.
    let commands = vec![iota_types::transaction::Command::MoveCall(Box::new(
        ProgrammableMoveCall {
            package: IOTA_FRAMEWORK_PACKAGE_ID,
            module: "_".into(),
            function: "timestamp_ms".into(),
            type_arguments: vec![],
            arguments: vec![iota_types::transaction::Argument::Input(0)],
        },
    ))];
    let pt = ProgrammableTransaction { inputs, commands };
    let tx = TransactionKind::programmable(pt);

    Base64::from_bytes(&bcs::to_bytes(&tx).unwrap())
}

#[sim_test]
async fn version_9_accepts() {
    let test_cluster = TestClusterBuilder::new()
        .with_protocol_version(ProtocolVersion::new(9))
        .build()
        .await;

    let client = test_cluster.rpc_client();

    let tx_bytes = build_faulty_transaction_byte_sequence();

    let result = client
        .dev_inspect_transaction_block(test_cluster.get_address_0(), tx_bytes, None, None, None)
        .await;

    let dev_inspect_result = result.expect("transaction should have been considered valid");
    assert_eq!(
        *dev_inspect_result.effects.status(),
        IotaExecutionStatus::Failure {
            error: "Move Bytecode Verification Error. Please run the Bytecode Verifier for more information. in command 0".to_string(),
        }
    );
}

#[sim_test]
async fn above_version_9_it_fails() {
    let test_cluster = TestClusterBuilder::new().build().await;

    let client = test_cluster.rpc_client();

    let tx_bytes = build_faulty_transaction_byte_sequence();

    let result = client
        .dev_inspect_transaction_block(test_cluster.get_address_0(), tx_bytes, None, None, None)
        .await;

    if let ClientError::Call(error_object) =
        result.expect_err("transaction should have been considered invalid")
    {
        assert_eq!(error_object.code(), ErrorCode::InvalidParams.code());
        assert_eq!(
            error_object.message(),
            "Error checking transaction input objects: Invalid identifier found in the transaction: _"
                .to_string()
        );
    } else {
        panic!("received unexpected error from json rpc api")
    }
}
