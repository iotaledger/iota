// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests that the gRPC server runs simulations off the threads that serve
//! requests, and refuses a simulation while the executor runs as many as it
//! allows.

mod common;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc},
    thread::ThreadId,
    time::Duration,
};

use common::{
    MockGrpcStateReader, create_test_transaction, start_test_server_with_traffic_controller,
};
use iota_grpc_server::GrpcServerHandle;
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::{
        bcs::BcsData,
        ledger_service::{GetServiceInfoRequest, ledger_service_client::LedgerServiceClient},
        transaction::Transaction as ProtoTransaction,
        transaction_execution_service::{
            SimulateTransactionItem, SimulateTransactionsRequest, simulate_transaction_result,
            transaction_execution_service_client::TransactionExecutionServiceClient,
        },
    },
};
use iota_sdk_types::{Address, GasPayment, Transaction, TransactionDigest, TransactionEffects};
use iota_traffic_controller::TrafficController;
use iota_types::{
    effects::TestEffectsBuilder,
    error::IotaError,
    messages_checkpoint::CheckpointSequenceNumber,
    quorum_driver_types::{
        ExecuteTransactionRequestV1, ExecuteTransactionResponseV1, QuorumDriverError,
    },
    traffic_control::{ClientIdSource, PolicyConfig, PolicyType, SimulationGasLimitConfig, Weight},
    transaction_executor::{SimulateTransactionResult, TransactionExecutor, VmChecks},
};
use tokio::sync::{Notify, Semaphore};
use tonic::{Code, transport::Channel};

/// A `TransactionExecutor` that records the thread each simulation runs on.
/// A simulation fails, or with `computation_cost` set succeeds at that cost
/// and `gas_price`. It allows `permits` simulations at once.
struct SimulationExecutor {
    permits: Arc<Semaphore>,
    computation_cost: Option<u64>,
    gas_price: u64,
    threads: Mutex<Vec<ThreadId>>,
    /// Notified when a simulation starts.
    started: Notify,
    /// When set, the next simulation waits until the sender sends or is
    /// dropped.
    gate: Mutex<Option<mpsc::Receiver<()>>>,
}

impl SimulationExecutor {
    fn new(permits: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(permits)),
            computation_cost: None,
            gas_price: 1,
            threads: Mutex::default(),
            started: Notify::new(),
            gate: Mutex::default(),
        }
    }

    fn costing(computation_cost: u64, gas_price: u64) -> Self {
        Self {
            computation_cost: Some(computation_cost),
            gas_price,
            ..Self::new(1)
        }
    }
}

#[async_trait::async_trait]
impl TransactionExecutor for SimulationExecutor {
    async fn execute_transaction(
        &self,
        _request: ExecuteTransactionRequestV1,
        _skip_certification: bool,
        _client_addr: Option<std::net::SocketAddr>,
    ) -> Result<ExecuteTransactionResponseV1, QuorumDriverError> {
        unreachable!("test requests only simulate transactions")
    }

    fn simulate_transaction(
        &self,
        _transaction: Transaction,
        _checks: VmChecks,
    ) -> Result<SimulateTransactionResult, IotaError> {
        self.threads
            .lock()
            .unwrap()
            .push(std::thread::current().id());
        self.started.notify_one();
        let gate = self.gate.lock().unwrap().take();
        if let Some(gate) = gate {
            // Either a send or a dropped sender opens the gate.
            let _ = gate.recv();
        }
        let computation_cost = self
            .computation_cost
            .ok_or_else(|| IotaError::Unknown("test simulations fail".to_string()))?;
        let (_digest, transaction, _effects) = create_test_transaction();
        let mut effects = TestEffectsBuilder::new(transaction.data()).build();
        let TransactionEffects::V1(effects_v1) = &mut effects else {
            unreachable!("the test effects are V1")
        };
        effects_v1.gas_cost_summary.computation_cost = computation_cost;
        Ok(SimulateTransactionResult {
            effects,
            events: None,
            input_objects: BTreeMap::new(),
            output_objects: BTreeMap::new(),
            execution_result: Ok(vec![]),
            mock_gas_id: None,
            suggested_gas_price: None,
            gas_data: GasPayment {
                objects: vec![],
                owner: Address::ZERO,
                price: self.gas_price,
                budget: computation_cost,
            },
        })
    }

    fn try_acquire_simulation_permit(&self) -> Option<Box<dyn Send + Sync>> {
        self.permits
            .clone()
            .try_acquire_owned()
            .ok()
            .map(|permit| Box::new(permit) as Box<dyn Send + Sync>)
    }

    async fn wait_for_checkpoint_inclusion(
        &self,
        _digests: &[TransactionDigest],
        _timeout: Duration,
    ) -> Result<BTreeMap<TransactionDigest, (CheckpointSequenceNumber, u64)>, IotaError> {
        unreachable!("test requests only simulate transactions")
    }
}

/// Starts a server whose traffic controller blocks a client on its first
/// client error, and connects to it. The server runs while the handle is
/// held.
async fn connect(
    executor: Arc<SimulationExecutor>,
) -> (GrpcServerHandle, TransactionExecutionServiceClient<Channel>) {
    connect_with(executor, None).await
}

/// Like [`connect`], with the given simulation gas limit and a computation cap
/// of [`MAX_COMPUTATION`].
async fn connect_with(
    executor: Arc<SimulationExecutor>,
    simulation_gas_limit: Option<SimulationGasLimitConfig>,
) -> (GrpcServerHandle, TransactionExecutionServiceClient<Channel>) {
    let traffic_controller = Arc::new(TrafficController::init_for_test(PolicyConfig {
        connection_blocklist_ttl_sec: 120,
        error_policy_type: PolicyType::TestNConnIP(1),
        spam_sample_rate: Weight::one(),
        dry_run: false,
        simulation_gas_limit,
        ..Default::default()
    }));
    traffic_controller.set_max_simulation_computation(MAX_COMPUTATION);
    let (handle, _reader) = start_test_server_with_traffic_controller(
        Arc::new(MockGrpcStateReader::default()),
        traffic_controller,
        Some(executor),
        ClientIdSource::SocketAddr,
    )
    .await;
    let channel = Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .expect("failed to connect to test gRPC server");
    (handle, TransactionExecutionServiceClient::new(channel))
}

/// The computation cap of one transaction in these tests, in gas units.
const MAX_COMPUTATION: u64 = 1_000;

/// A request that simulates `n` transactions which pass the checks of the
/// handler.
fn simulate_request(n: usize) -> SimulateTransactionsRequest {
    let (_digest, transaction, _effects) = create_test_transaction();
    let transaction_bcs =
        bcs::to_bytes(&transaction.data().signed_transaction().transaction).unwrap();
    let item = SimulateTransactionItem::default().with_transaction(
        ProtoTransaction::default().with_bcs(BcsData::default().with_data(transaction_bcs)),
    );
    SimulateTransactionsRequest::default()
        .with_transactions(vec![item; n])
        .with_read_mask(prost_types::FieldMask::from_str("execution_result"))
}

/// The status codes of the items of a simulation batch.
async fn simulate(
    client: &mut TransactionExecutionServiceClient<Channel>,
    n: usize,
) -> Result<Vec<Code>, tonic::Status> {
    let response = client.simulate_transactions(simulate_request(n)).await?;
    Ok(response
        .into_inner()
        .transaction_results
        .into_iter()
        .map(|result| match result.result {
            Some(simulate_transaction_result::Result::Error(error)) => Code::from(error.code),
            _ => Code::Ok,
        })
        .collect())
}

/// The error message of a simulation of one transaction that the server
/// refuses.
async fn refusal_message(client: &mut TransactionExecutionServiceClient<Channel>) -> String {
    let response = client
        .simulate_transactions(simulate_request(1))
        .await
        .unwrap()
        .into_inner();
    let Some(simulate_transaction_result::Result::Error(error)) =
        &response.transaction_results[0].result
    else {
        panic!("the simulation is refused");
    };
    error.message.clone()
}

/// On a runtime with a single thread, a simulation that ran on the thread
/// serving the request would stall every other request until it ended.
#[tokio::test(flavor = "current_thread")]
async fn a_simulation_does_not_run_on_the_thread_serving_requests() {
    let executor = Arc::new(SimulationExecutor::new(2));
    let (_handle, mut client) = connect(executor.clone()).await;

    assert_eq!(
        simulate(&mut client, 2).await.unwrap(),
        [Code::Internal, Code::Internal]
    );
    let serving_thread = std::thread::current().id();
    let threads = executor.threads.lock().unwrap().clone();
    assert_eq!(threads.len(), 2);
    assert!(
        threads.iter().all(|thread| *thread != serving_thread),
        "a simulation ran on the thread serving requests"
    );
}

#[tokio::test]
async fn a_simulation_without_a_permit_is_refused_without_an_error_weight() {
    let executor = Arc::new(SimulationExecutor::new(0));
    let (_handle, mut client) = connect(executor.clone()).await;

    assert_eq!(
        simulate(&mut client, 2).await.unwrap(),
        [Code::ResourceExhausted, Code::ResourceExhausted]
    );
    assert!(executor.threads.lock().unwrap().is_empty());
    // The error policy blocks the client on its first client error, so the
    // refusals above must not have counted as one.
    assert_eq!(
        simulate(&mut client, 1).await.unwrap(),
        [Code::ResourceExhausted]
    );
}

#[tokio::test]
async fn a_simulation_is_refused_while_another_runs_and_served_after_it_ends() {
    let executor = Arc::new(SimulationExecutor::new(1));
    let (open_gate, gate) = mpsc::channel();
    *executor.gate.lock().unwrap() = Some(gate);
    let (_handle, mut client) = connect(executor.clone()).await;

    let first = tokio::spawn({
        let mut client = client.clone();
        async move { simulate(&mut client, 1).await }
    });
    executor.started.notified().await;
    assert_eq!(
        simulate(&mut client, 1).await.unwrap(),
        [Code::ResourceExhausted]
    );

    open_gate.send(()).unwrap();
    assert_eq!(first.await.unwrap().unwrap(), [Code::Internal]);
    assert_eq!(simulate(&mut client, 1).await.unwrap(), [Code::Internal]);
}

/// A batch whose first items use up the client's gas balance has the later
/// items refused, without an error weight, and the client's other requests
/// are still served.
#[tokio::test]
async fn simulations_over_the_gas_limit_are_refused() {
    // Each simulation reserves 1_000 and is charged 4_000 / 10 = 400 of it,
    // so after three of them 800 of the 2_000 are left.
    let executor = Arc::new(SimulationExecutor::costing(4_000, 10));
    let (handle, mut client) = connect_with(
        executor,
        Some(SimulationGasLimitConfig {
            gas_per_sec: 1,
            burst_gas: 2_000,
        }),
    )
    .await;

    assert_eq!(
        simulate(&mut client, 5).await.unwrap(),
        [
            Code::Ok,
            Code::Ok,
            Code::Ok,
            Code::ResourceExhausted,
            Code::ResourceExhausted
        ]
    );
    let message = refusal_message(&mut client).await;
    assert!(
        message.starts_with("simulation gas limit reached, retry in "),
        "{message}"
    );

    // The error policy blocks the client on its first client error, so the
    // refusals must not have counted as one.
    let mut ledger = LedgerServiceClient::new(
        Channel::from_shared(format!("http://{}", handle.address()))
            .unwrap()
            .connect()
            .await
            .unwrap(),
    );
    // The mock store has no checkpoints to report, so the handler answers
    // with an internal error rather than the refusal of a blocked client.
    let status = ledger
        .get_service_info(GetServiceInfoRequest::default())
        .await
        .expect_err("the mock store has no checkpoints");
    assert_eq!(status.code(), Code::Internal, "{status:?}");
}

#[tokio::test]
async fn a_failed_simulation_gives_its_gas_back() {
    let executor = Arc::new(SimulationExecutor::new(1));
    let (_handle, mut client) = connect_with(
        executor,
        Some(SimulationGasLimitConfig {
            gas_per_sec: 1,
            burst_gas: MAX_COMPUTATION,
        }),
    )
    .await;
    assert_eq!(
        simulate(&mut client, 3).await.unwrap(),
        [Code::Internal, Code::Internal, Code::Internal]
    );
}

/// A simulation that the executor refuses for want of a permit gives back the
/// gas it reserved.
#[tokio::test]
async fn a_simulation_refused_for_a_permit_gives_its_gas_back() {
    let executor = Arc::new(SimulationExecutor::new(1));
    let (open_gate, gate) = mpsc::channel();
    *executor.gate.lock().unwrap() = Some(gate);
    let (_handle, mut client) = connect_with(
        executor.clone(),
        Some(SimulationGasLimitConfig {
            gas_per_sec: 1,
            burst_gas: 2 * MAX_COMPUTATION,
        }),
    )
    .await;

    let first = tokio::spawn({
        let mut client = client.clone();
        async move { simulate(&mut client, 1).await }
    });
    executor.started.notified().await;
    // The running simulation holds the permit and half of the balance. Each
    // simulation below reserves the other half before it is refused, so
    // without a refund the second one would be refused for gas.
    for _ in 0..2 {
        let message = refusal_message(&mut client).await;
        assert!(
            message.contains("running as many simulations as it allows"),
            "{message}"
        );
    }

    open_gate.send(()).unwrap();
    assert_eq!(first.await.unwrap().unwrap(), [Code::Internal]);
}
