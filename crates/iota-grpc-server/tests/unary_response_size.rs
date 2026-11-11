// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The unary transaction-execution handlers check the encoded size of a
//! response before they return it, so that a client that does not read
//! cannot make the server hold an oversized response.
//!
//! Without the check, tonic builds and returns the full response from the
//! handler, and only rejects it once hyper polls the response body to send
//! it (`max_encoding_message_size`, `tonic::codec::encode::finish_encoding`).
//! By then the oversized response has already been held in memory. This
//! test asserts the rejection comes from the handler itself: its error
//! message differs from tonic's own ("Error, encoded message length too
//! large: ..."), so a version of the handler without the check would either
//! return the oversized response (if under tonic's own limit) or fail with
//! tonic's message instead of the handler's.

mod common;

use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use common::{create_large_object, start_test_server_with_executor};
use iota_grpc_server::GrpcServerHandle;
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::{
        bcs::BcsData,
        signatures::{UserSignature, UserSignatures},
        transaction::Transaction as ProtoTransaction,
        transaction_execution_service::{
            ExecuteTransactionItem, ExecuteTransactionsRequest, SimulateTransactionItem,
            SimulateTransactionsRequest,
            transaction_execution_service_client::TransactionExecutionServiceClient,
        },
    },
};
use iota_sdk_types::{Address, GasPayment, Transaction, TransactionDigest, TransactionEffects};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::random_object_ref,
    crypto::{AccountPrivateKey, get_key_pair},
    effects::TransactionEffectsExtForTesting as _,
    error::IotaError,
    messages_checkpoint::CheckpointSequenceNumber,
    quorum_driver_types::{
        EffectsFinalityInfo, ExecuteTransactionRequestV1, ExecuteTransactionResponseV1,
        FinalizedEffects, QuorumDriverError,
    },
    transaction_executor::{SimulateTransactionResult, TransactionExecutor, VmChecks},
};
use tonic::{Code, transport::Channel};

/// The gRPC config floor for `max_message_size_bytes`, see
/// `GrpcApiConfig::GRPC_MIN_CLIENT_MAX_MESSAGE_SIZE_BYTES`. The test
/// configures the server at exactly this floor: it is the smallest `M` an
/// operator can configure, so it is the strictest case the handler-level
/// check must still catch.
const MAX_MESSAGE_SIZE_BYTES: u32 = 4 * 1024 * 1024;

/// Comfortably more than [`MAX_MESSAGE_SIZE_BYTES`] so the single output
/// object alone pushes the response over the limit.
const OVERSIZED_OBJECT_PADDING_BYTES: usize = 5 * 1024 * 1024;

/// A `TransactionExecutor` whose `execute_transaction` and
/// `simulate_transaction` return a single output object bigger than
/// [`MAX_MESSAGE_SIZE_BYTES`], so the response built from it exceeds the
/// configured limit.
struct OversizedExecutor;

#[async_trait::async_trait]
impl TransactionExecutor for OversizedExecutor {
    async fn execute_transaction(
        &self,
        request: ExecuteTransactionRequestV1,
        _skip_certification: bool,
        _client_addr: Option<SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        let digest = *request.transaction.digest();
        let (_, object) = create_large_object(OVERSIZED_OBJECT_PADDING_BYTES);
        Ok(ExecuteTransactionResponseV1 {
            effects: FinalizedEffects {
                effects: TransactionEffects::new_empty_v1_for_testing(digest),
                finality_info: EffectsFinalityInfo::QuorumExecuted(0),
            },
            events: None,
            input_objects: None,
            output_objects: Some(vec![object]),
            auxiliary_data: None,
        })
    }

    fn simulate_transaction(
        &self,
        transaction: Transaction,
        _checks: VmChecks,
    ) -> Result<SimulateTransactionResult, IotaError> {
        let digest = transaction.digest();
        let (object_id, object) = create_large_object(OVERSIZED_OBJECT_PADDING_BYTES);
        Ok(SimulateTransactionResult {
            effects: TransactionEffects::new_empty_v1_for_testing(digest),
            events: None,
            input_objects: BTreeMap::new(),
            output_objects: BTreeMap::from([(object_id, object)]),
            execution_result: Ok(Vec::new()),
            mock_gas_id: None,
            suggested_gas_price: None,
            gas_data: GasPayment {
                objects: Vec::new(),
                owner: Address::random(),
                price: 1,
                budget: 1,
            },
        })
    }

    async fn wait_for_checkpoint_inclusion(
        &self,
        _digests: &[TransactionDigest],
        _timeout: Duration,
    ) -> Result<BTreeMap<TransactionDigest, (CheckpointSequenceNumber, u64)>, IotaError> {
        unreachable!("the requests do not wait for checkpoint inclusion")
    }
}

/// A `SimulateTransactionsRequest` with one syntactically valid transaction,
/// asking only for the output objects (where the oversized data lives).
fn oversized_simulate_request() -> SimulateTransactionsRequest {
    // The executor is mocked, so the transaction never needs a signature.
    let (sender, _): (_, AccountPrivateKey) = get_key_pair();
    let gas = random_object_ref();
    let transaction = TestTransactionBuilder::new(sender, gas, 1000)
        .transfer(random_object_ref(), sender)
        .build();
    let transaction_bytes = bcs::to_bytes(&transaction).expect("serialize transaction");

    SimulateTransactionsRequest::default()
        .with_transactions(vec![
            SimulateTransactionItem::default()
                .with_transaction(ProtoTransaction::default().with_bcs(transaction_bytes)),
        ])
        .with_read_mask(prost_types::FieldMask::from_str(
            "executed_transaction.output_objects.bcs",
        ))
}

/// An `ExecuteTransactionsRequest` with one signed transaction, asking only
/// for the output objects (where the oversized data lives).
fn oversized_execute_request() -> ExecuteTransactionsRequest {
    // The executor is mocked, so the signature is never checked.
    let (sender, key): (_, AccountPrivateKey) = get_key_pair();
    let transaction = TestTransactionBuilder::new(sender, random_object_ref(), 1000)
        .transfer(random_object_ref(), sender)
        .build_and_sign(&key);
    let signatures = UserSignatures::default().with_signatures(
        transaction
            .signatures()
            .iter()
            .map(|signature| {
                UserSignature::default().with_bcs(
                    BcsData::default().with_data(bcs::to_bytes(signature).expect("serialize")),
                )
            })
            .collect(),
    );

    ExecuteTransactionsRequest::default()
        .with_transactions(vec![
            ExecuteTransactionItem::default()
                .with_transaction(
                    ProtoTransaction::default().with_bcs(
                        BcsData::default().with_data(
                            bcs::to_bytes(transaction.transaction()).expect("serialize"),
                        ),
                    ),
                )
                .with_signatures(signatures),
        ])
        .with_read_mask(prost_types::FieldMask::from_str("output_objects.bcs"))
}

/// Starts a server with [`OversizedExecutor`] and returns a client of its
/// transaction execution service.
async fn start_oversized_server() -> (GrpcServerHandle, TransactionExecutionServiceClient<Channel>)
{
    let (handle, _reader) = start_test_server_with_executor(
        Arc::new(common::MockGrpcStateReader::default()),
        Arc::new(OversizedExecutor),
        |config| config.max_message_size_bytes = MAX_MESSAGE_SIZE_BYTES,
    )
    .await;
    let channel = Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .expect("connect");
    (handle, TransactionExecutionServiceClient::new(channel))
}

/// Checks that the handler itself rejected the oversized response.
fn assert_rejected_by_the_handler(status: tonic::Status) {
    assert_eq!(
        status.code(),
        Code::OutOfRange,
        "unexpected error: {status:?}"
    );
    assert!(
        status.message().contains("exceeds max message size"),
        "expected the handler's own size-limit message, got: {status:?}"
    );
}

/// A response whose only requested field is bigger than the configured
/// `max_message_size_bytes` must be rejected by the handler itself, not left
/// for tonic to build in full and reject only once the transport tries to
/// send it.
#[tokio::test]
async fn oversized_simulate_response_is_rejected_by_the_handler() {
    let (_handle, mut client) = start_oversized_server().await;
    let status = client
        .simulate_transactions(oversized_simulate_request())
        .await
        .expect_err("oversized response should be rejected");
    assert_rejected_by_the_handler(status);
}

/// The same as [`oversized_simulate_response_is_rejected_by_the_handler`],
/// for `ExecuteTransactions`.
#[tokio::test]
async fn oversized_execute_response_is_rejected_by_the_handler() {
    let (_handle, mut client) = start_oversized_server().await;
    let status = client
        .execute_transactions(oversized_execute_request())
        .await
        .expect_err("oversized response should be rejected");
    assert_rejected_by_the_handler(status);
}
