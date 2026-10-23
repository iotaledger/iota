// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests of which services `IotaNode::start` starts for each node config.
//!
//! Each test starts one node from a fresh genesis and looks at the node only
//! from outside: the sockets it listens on, the JSON-RPC methods it serves,
//! the metrics it registers, the public fields of its state, and its database
//! directory.

// The tests start nodes outside a simulated node and probe real sockets.
#![cfg(not(msim))]

use std::{
    collections::{BTreeMap, BTreeSet},
    net::{SocketAddr, TcpListener, TcpStream},
    panic::{AssertUnwindSafe, catch_unwind},
    path::Path,
    sync::Arc,
    time::Duration,
};

use iota_config::{
    NodeConfig,
    local_ip_utils::new_local_tcp_socket_for_testing,
    node::{GrpcApiConfig, RunWithRange, StateSnapshotConfig},
    object_storage_config::{ObjectStoreConfig, ObjectStoreType},
    transaction_deny_config::TransactionDenyConfigBuilder,
};
use iota_grpc_types::v1::transaction_execution_service::{
    ExecuteTransactionsRequest,
    transaction_execution_service_client::TransactionExecutionServiceClient,
};
use iota_metrics::{RegistryService, test_utils::MetricsReader};
use iota_node::{IotaNode, ServerVersion};
use iota_protocol_config::ProtocolVersion;
use iota_swarm_config::{
    network_config::NetworkConfig,
    network_config_builder::ConfigBuilder,
    node_config_builder::{FullnodeConfigBuilder, ValidatorConfigBuilder},
};
use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};
use prometheus_filtered::{IntGauge, Registry};
use rand::{rand_core::UnwrapErr, rngs::SysRng};
use rstest::rstest;
use tonic::{Code, transport::Channel};

// One method of each JSON-RPC module a node can serve.
const READ: &str = "iota_getObject";
const COIN_READ: &str = "iotax_getBalance";
const GOVERNANCE_READ: &str = "iotax_getReferenceGasPrice";
const INDEXER: &str = "iotax_queryEvents";
const MOVE_UTILS: &str = "iota_getNormalizedMoveFunction";
const TRANSACTION_BUILDER: &str = "unsafe_moveCall";
const TRANSACTION_EXECUTION: &str = "iota_executeTransactionBlock";

/// A metric the state snapshot uploader registers when it is built. It shows
/// only that the node built the uploader: `state_snapshot_tests` checks that
/// the node also hands it the snapshot of each epoch it leaves.
const STATE_SNAPSHOT_UPLOADER_METRIC: &str = "first_missing_state_snapshot_epoch";

/// A metric that only the IOTA conservation check sets.
const IOTA_CONSERVATION_METRIC: &str = "iota_conservation_live_object_count";

/// A metric that the monitor of each runtime registers, with the name of the
/// runtime as its `runtime` label.
const RUNTIME_MONITOR_METRIC: &str = "tokio_runtime_workers";

/// What a started node serves.
#[derive(Debug, Default, PartialEq, Eq)]
struct Services {
    /// The JSON-RPC modules the node serves, each named by one of its
    /// methods. `None` when nothing listens on the JSON-RPC address.
    json_rpc: Option<BTreeSet<&'static str>>,
    grpc_api: bool,
    grpc_api_executes_transactions: bool,
    validator_server: bool,
    transaction_orchestrator: bool,
    index_store: bool,
    grpc_indexes: bool,
    validator_tx_finalizer: bool,
    state_snapshot_uploader: bool,
    /// The `process` label of the uptime metric.
    uptime_process: String,
}

/// The JSON-RPC modules every fullnode serves.
fn read_only_json_rpc() -> BTreeSet<&'static str> {
    BTreeSet::from([READ, COIN_READ, GOVERNANCE_READ, INDEXER, MOVE_UTILS])
}

/// A network config with a committee of one validator.
fn network_config() -> NetworkConfig {
    ConfigBuilder::new_with_temp_dir().build()
}

/// A fullnode config whose gRPC API, enabled or not, has an address of its
/// own, so that a test can see whether anything listens there.
fn fullnode_config(network: &NetworkConfig, builder: FullnodeConfigBuilder) -> NodeConfig {
    builder
        .with_grpc_api_config(grpc_api_config())
        .build(&mut UnwrapErr(SysRng), network)
}

fn grpc_api_config() -> GrpcApiConfig {
    GrpcApiConfig {
        address: new_local_tcp_socket_for_testing(),
        ..Default::default()
    }
}

/// A validator config with addresses of its own for the JSON-RPC and gRPC
/// APIs, so that a test can see whether anything listens there.
fn with_own_api_addresses(mut config: NodeConfig) -> NodeConfig {
    config.json_rpc_address = new_local_tcp_socket_for_testing();
    config.grpc_api_config = Some(grpc_api_config());
    config
}

fn state_snapshot_config(store: &Path) -> StateSnapshotConfig {
    StateSnapshotConfig {
        object_store_config: Some(ObjectStoreConfig {
            object_store: Some(ObjectStoreType::File),
            directory: Some(store.to_path_buf()),
            ..Default::default()
        }),
        concurrency: 1,
    }
}

/// A started node, the registry of its metrics, and the addresses of its
/// servers.
struct StartedNode {
    node: Arc<IotaNode>,
    registry: Registry,
    json_rpc_address: SocketAddr,
    grpc_api_address: SocketAddr,
    validator_address: SocketAddr,
}

impl StartedNode {
    /// What the node serves.
    async fn services(&self) -> Services {
        let state = self.node.state();
        let metrics = MetricsReader::new(&self.registry);
        Services {
            json_rpc: json_rpc_modules(self.json_rpc_address).await,
            grpc_api: listens(self.grpc_api_address),
            grpc_api_executes_transactions: grpc_api_executes_transactions(self.grpc_api_address)
                .await,
            validator_server: listens(self.validator_address),
            transaction_orchestrator: self.node.transaction_orchestrator().is_some(),
            index_store: state.indexes.is_some(),
            grpc_indexes: state.grpc_indexes_store.is_some(),
            validator_tx_finalizer: state.validator_tx_finalizer.is_some(),
            state_snapshot_uploader: metrics.has_family(STATE_SNAPSHOT_UPLOADER_METRIC),
            uptime_process: self.uptime_process(),
        }
    }

    fn uptime_process(&self) -> String {
        self.registry
            .gather()
            .into_iter()
            .filter(|family| family.name() == "uptime")
            .flat_map(|family| family.get_metric().to_vec())
            .flat_map(|metric| metric.get_label().to_vec())
            .find(|label| label.name() == "process")
            .map(|label| label.value().to_owned())
            .unwrap_or_default()
    }

    /// The worker count that the monitor of each runtime reports, by the
    /// `runtime` label of the monitor.
    fn runtime_workers(&self) -> BTreeMap<String, f64> {
        self.registry
            .gather()
            .into_iter()
            .filter(|family| family.name() == RUNTIME_MONITOR_METRIC)
            .flat_map(|family| family.get_metric().to_vec())
            .filter_map(|metric| {
                let runtime = metric
                    .get_label()
                    .iter()
                    .find(|label| label.name() == "runtime")?
                    .value()
                    .to_owned();
                Some((runtime, metric.get_gauge().value()))
            })
            .collect()
    }
}

async fn start(config: NodeConfig) -> anyhow::Result<StartedNode> {
    start_with_serving_runtime(config, None).await
}

async fn start_with_serving_runtime(
    config: NodeConfig,
    serving_runtime: Option<tokio::runtime::Handle>,
) -> anyhow::Result<StartedNode> {
    start_with_registry(config, Registry::new(), serving_runtime).await
}

/// Starts a node from `config`, which must have a gRPC API config, enabled
/// or not, with its metrics in `registry`. The node runs its client and
/// validator servers on `serving_runtime`, or on the current runtime when it
/// is `None`.
async fn start_with_registry(
    config: NodeConfig,
    registry: Registry,
    serving_runtime: Option<tokio::runtime::Handle>,
) -> anyhow::Result<StartedNode> {
    let json_rpc_address = config.json_rpc_address;
    let grpc_api_address = config.grpc_api_config.as_ref().unwrap().address;
    let validator_address = config.network_address.to_socket_addr().unwrap();
    let registry_service = RegistryService::new(registry.clone());
    let node = match serving_runtime {
        None => IotaNode::start(config, registry_service).await?,
        Some(serving_runtime) => {
            IotaNode::start_async(
                config,
                registry_service,
                ServerVersion::new("iota-node", "test"),
                serving_runtime,
            )
            .await?
        }
    };
    Ok(StartedNode {
        node,
        registry,
        json_rpc_address,
        grpc_api_address,
        validator_address,
    })
}

/// Starts a node from `config` and returns what it serves.
async fn start_and_look(config: NodeConfig) -> Services {
    start(config).await.unwrap().services().await
}

/// A multi-threaded runtime, for a test that starts and stops runtimes.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
}

/// Asserts that the node started when `started` is set, and otherwise that
/// the start failed with an error whose text contains `expected_error`.
fn assert_start<T>(result: anyhow::Result<T>, started: bool, expected_error: &str) {
    match result {
        Ok(_) => assert!(started, "the node started, expected: {expected_error}"),
        Err(err) => {
            let err = format!("{err:#}");
            assert!(!started && err.contains(expected_error), "{err}");
        }
    }
}

/// The error a bind to `address` gets while another socket listens there.
fn address_in_use_error(address: SocketAddr) -> String {
    TcpListener::bind(address).unwrap_err().to_string()
}

fn listens(address: SocketAddr) -> bool {
    TcpStream::connect(address).is_ok()
}

async fn json_rpc_modules(address: SocketAddr) -> Option<BTreeSet<&'static str>> {
    if !listens(address) {
        return None;
    }
    let client = HttpClientBuilder::default()
        .build(format!("http://{address}"))
        .unwrap();
    let discovery: serde_json::Value = client.request("rpc.discover", rpc_params![]).await.unwrap();
    let methods: BTreeSet<&str> = discovery["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|method| method["name"].as_str().unwrap())
        .collect();
    Some(
        [
            READ,
            COIN_READ,
            GOVERNANCE_READ,
            INDEXER,
            MOVE_UTILS,
            TRANSACTION_BUILDER,
            TRANSACTION_EXECUTION,
        ]
        .into_iter()
        .filter(|module| methods.contains(module))
        .collect(),
    )
}

/// Whether the gRPC API at `address` serves the transaction execution
/// service. An empty request executes nothing.
async fn grpc_api_executes_transactions(address: SocketAddr) -> bool {
    if !listens(address) {
        return false;
    }
    let channel = Channel::from_shared(format!("http://{address}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let result = TransactionExecutionServiceClient::new(channel)
        .execute_transactions(ExecuteTransactionsRequest::default())
        .await;
    !matches!(result, Err(status) if status.code() == Code::Unimplemented)
}

#[rstest]
#[tokio::test]
async fn a_fullnode_starts_what_its_config_asks_for(
    #[values(false, true)] grpc_api: bool,
    #[values(false, true)] index_processing: bool,
    #[values(false, true)] run_with_range: bool,
    #[values(false, true)] snapshot_upload: bool,
) {
    let network = network_config();
    let snapshot_store = tempfile::tempdir().unwrap();
    let mut builder = FullnodeConfigBuilder::new().with_enable_grpc_api(grpc_api);
    if run_with_range {
        builder = builder.with_run_with_range(Some(RunWithRange::Epoch(5)));
    }
    if snapshot_upload {
        builder = builder.with_state_snapshot_config(state_snapshot_config(snapshot_store.path()));
    }
    let mut config = fullnode_config(&network, builder);
    config.enable_index_processing = index_processing;

    let mut json_rpc = read_only_json_rpc();
    if !run_with_range {
        json_rpc.extend([TRANSACTION_BUILDER, TRANSACTION_EXECUTION]);
    }
    assert_eq!(
        start_and_look(config).await,
        Services {
            json_rpc: Some(json_rpc),
            grpc_api,
            grpc_api_executes_transactions: grpc_api && !run_with_range,
            transaction_orchestrator: !run_with_range,
            index_store: index_processing,
            grpc_indexes: grpc_api,
            state_snapshot_uploader: snapshot_upload,
            uptime_process: "fullnode".to_owned(),
            ..Default::default()
        }
    );
}

#[tokio::test]
async fn a_fullnode_runs_the_validator_tx_finalizer_when_its_config_asks_for_it() {
    let network = network_config();
    let mut config = fullnode_config(&network, FullnodeConfigBuilder::new());
    config.enable_validator_tx_finalizer = true;

    assert!(start_and_look(config).await.validator_tx_finalizer);
}

/// A validator starts neither the client APIs nor the fullnode stores, even
/// with index processing on and addresses for the client APIs.
#[rstest]
#[tokio::test]
async fn a_committee_validator_starts_only_the_validator_services(
    #[values(false, true)] tx_finalizer: bool,
) {
    let mut network = network_config();
    let mut config = with_own_api_addresses(network.validator_configs.remove(0));
    config.enable_index_processing = true;
    config.enable_validator_tx_finalizer = tx_finalizer;

    assert_eq!(
        start_and_look(config).await,
        Services {
            validator_server: true,
            validator_tx_finalizer: tx_finalizer,
            uptime_process: "validator".to_owned(),
            ..Default::default()
        }
    );
}

/// A node with a validator config outside the committee starts no validator
/// server, and no client API either.
#[tokio::test]
async fn a_validator_outside_the_committee_starts_no_server() {
    let network = network_config();
    let mut config = with_own_api_addresses(
        ValidatorConfigBuilder::new().build_new_validator(&mut UnwrapErr(SysRng), &network),
    );
    config.enable_index_processing = true;

    assert_eq!(
        start_and_look(config).await,
        Services {
            validator_tx_finalizer: true,
            uptime_process: "validator".to_owned(),
            ..Default::default()
        }
    );
}

/// The validator components start for a key in the committee, and they need a
/// consensus config. A config with such a key and no consensus config fails
/// to start.
#[tokio::test]
async fn a_committee_member_without_a_consensus_config_fails_to_start() {
    let mut network = network_config();
    let mut config = with_own_api_addresses(network.validator_configs.remove(0));
    config.consensus_config = None;

    let result = start(config).await;

    assert_start(result, false, "Validator is missing consensus config");
}

/// The JSON-RPC, gRPC API and validator gRPC servers run on the serving
/// runtime: they stop listening when that runtime shuts down.
#[rstest]
fn the_servers_run_on_the_serving_runtime(#[values(false, true)] validator: bool) {
    let mut network = network_config();
    let config = if validator {
        with_own_api_addresses(network.validator_configs.remove(0))
    } else {
        fullnode_config(
            &network,
            FullnodeConfigBuilder::new().with_enable_grpc_api(true),
        )
    };
    let node_runtime = runtime();
    let serving_runtime = runtime();

    let node = node_runtime
        .block_on(start_with_serving_runtime(
            config,
            Some(serving_runtime.handle().clone()),
        ))
        .unwrap();
    let servers = if validator {
        vec![node.validator_address]
    } else {
        vec![node.json_rpc_address, node.grpc_api_address]
    };
    assert!(servers.iter().all(|address| listens(*address)));

    drop(serving_runtime);

    let still_listening: Vec<_> = servers
        .into_iter()
        .filter(|address| listens(*address))
        .collect();
    assert!(still_listening.is_empty(), "{still_listening:?}");
}

/// A fullnode checks IOTA conservation only on its first start, on an empty
/// database. Started again on the same database, it serves the same.
#[test]
fn a_fullnode_starts_again_on_its_database() {
    let network = network_config();
    let config = fullnode_config(
        &network,
        FullnodeConfigBuilder::new().with_enable_grpc_api(true),
    );

    let (first_services, first_live_objects) = start_and_stop(config.clone());
    let (second_services, second_live_objects) = start_and_stop(config);

    assert!(first_live_objects > 0.0);
    assert_eq!(second_live_objects, 0.0);
    assert_eq!(second_services, first_services);
}

/// Starts a node from `config` on a runtime of its own, and stops it by
/// shutting that runtime down. Returns what the node served, and the number
/// of live objects its IOTA conservation check counted, zero if the check did
/// not run.
fn start_and_stop(config: NodeConfig) -> (Services, f64) {
    runtime().block_on(async {
        let node = start(config).await.unwrap();
        let live_objects = MetricsReader::new(&node.registry).value(IOTA_CONSERVATION_METRIC, &[]);
        (node.services().await, live_objects)
    })
}

/// The database corruption mark that the node sets when it starts on an empty
/// database stays until the node has written genesis. A first start that fails
/// while the mark is set leaves a database the next start refuses. A first
/// start that fails before the mark, or after the node clears it, leaves a
/// database the next start uses. Each case makes the first start fail at one
/// point: the registry already has a metric that the node registers there.
/// What the failed start left in the database directory shows that it failed
/// at that point.
#[rstest]
#[case::before_the_mark("tx_deny_config_user_transaction_disabled", false, false, true)]
#[case::after_the_mark_before_the_stores("uptime", true, false, false)]
#[case::after_the_stores_before_the_clear("total_tx_certificates_created", true, true, false)]
#[case::after_the_clear("iota_current_protocol_version", true, true, true)]
fn a_failed_first_start_leaves_a_usable_database_unless_it_fails_while_marked(
    #[case] metric_the_start_fails_on: &str,
    #[case] leaves_the_mark_database: bool,
    #[case] leaves_the_stores: bool,
    #[case] next_start_works: bool,
) {
    let network = network_config();
    let config = fullnode_config(&network, FullnodeConfigBuilder::new());
    let registry = Registry::new();
    let taken = IntGauge::new(metric_the_start_fails_on, "taken before the start").unwrap();
    registry.register(Box::new(taken)).unwrap();

    let first_start = start_and_stop_with_registry(config.clone(), registry);
    assert!(first_start.is_err(), "the first start did not fail");
    assert_eq!(
        (
            config.db_path().join("status").exists(),
            config.db_path().join("store").exists()
        ),
        (leaves_the_mark_database, leaves_the_stores)
    );

    match start_and_stop_with_registry(config, Registry::new()) {
        Ok(()) => assert!(next_start_works, "the next start worked"),
        Err(panic) => assert!(
            !next_start_works && panic.contains("Database is corrupted"),
            "{panic}"
        ),
    }
}

/// Starts a node from `config` with its metrics in `registry`, on a runtime
/// of its own, and stops it by shutting that runtime down. Returns the
/// message of the panic if the start panics.
fn start_and_stop_with_registry(config: NodeConfig, registry: Registry) -> Result<(), String> {
    let runtime = runtime();
    let result = catch_unwind(AssertUnwindSafe(|| {
        runtime.block_on(async {
            start_with_registry(config, registry, None).await.unwrap();
        })
    }));
    drop(runtime);
    result.map_err(|panic| {
        panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                panic
                    .downcast_ref::<&str>()
                    .map(|message| message.to_string())
            })
            .unwrap_or_default()
    })
}

#[rstest]
#[tokio::test]
async fn only_a_fullnode_needs_its_json_rpc_address(#[values(false, true)] validator: bool) {
    let mut network = network_config();
    let mut config = if validator {
        with_own_api_addresses(network.validator_configs.remove(0))
    } else {
        fullnode_config(&network, FullnodeConfigBuilder::new())
    };
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    config.json_rpc_address = taken.local_addr().unwrap();

    let result = start(config).await;

    assert_start(
        result,
        validator,
        &address_in_use_error(taken.local_addr().unwrap()),
    );
}

#[rstest]
#[tokio::test]
async fn only_an_enabled_grpc_api_needs_its_address(#[values(false, true)] grpc_api: bool) {
    let network = network_config();
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let config = FullnodeConfigBuilder::new()
        .with_enable_grpc_api(grpc_api)
        .with_grpc_api_config(GrpcApiConfig {
            address: taken.local_addr().unwrap(),
            ..Default::default()
        })
        .build(&mut UnwrapErr(SysRng), &network);

    let result = start(config).await;

    assert_start(
        result,
        !grpc_api,
        &address_in_use_error(taken.local_addr().unwrap()),
    );
}

/// The key-value store behind JSON-RPC is built only on a fullnode, so only a
/// fullnode fails on a URL that does not parse.
#[rstest]
#[tokio::test]
async fn only_a_fullnode_needs_a_valid_kv_store_url(#[values(false, true)] validator: bool) {
    let mut network = network_config();
    let mut config = if validator {
        with_own_api_addresses(network.validator_configs.remove(0))
    } else {
        fullnode_config(&network, FullnodeConfigBuilder::new())
    };
    let url = "not a url";
    config.transaction_kv_store_read_config.base_url = url.to_owned();

    let result = start(config).await;

    assert_start(
        result,
        validator,
        &url.parse::<url::Url>().unwrap_err().to_string(),
    );
}

/// A node records its config and protocol versions in metrics, whatever its
/// role. With the default config, the node supports the same maximum protocol
/// version as the binary, so this does not tell the two metrics apart.
#[rstest]
#[tokio::test]
async fn a_node_records_its_config_and_protocol_versions(#[values(false, true)] validator: bool) {
    let mut network = network_config();
    let mut config = if validator {
        with_own_api_addresses(network.validator_configs.remove(0))
    } else {
        fullnode_config(&network, FullnodeConfigBuilder::new())
    };
    config.transaction_deny_config = TransactionDenyConfigBuilder::new()
        .disable_user_transaction()
        .build();

    let node = start(config).await.unwrap();

    let metrics = MetricsReader::new(&node.registry);
    let max = ProtocolVersion::MAX.as_u64() as f64;
    assert_eq!(
        metrics.value("tx_deny_config_user_transaction_disabled", &[]),
        1.0
    );
    assert_eq!(metrics.value("iota_binary_max_protocol_version", &[]), max);
    assert_eq!(
        metrics.value("iota_configured_max_protocol_version", &[]),
        max
    );
}

/// A node monitors both its runtimes, whatever its role: the monitor of each
/// runtime reports the worker count of that runtime.
#[rstest]
fn a_node_monitors_its_runtimes(#[values(false, true)] validator: bool) {
    let mut network = network_config();
    let config = if validator {
        with_own_api_addresses(network.validator_configs.remove(0))
    } else {
        fullnode_config(&network, FullnodeConfigBuilder::new())
    };
    // One worker on the node runtime, two on the serving runtime.
    let node_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let serving_runtime = runtime();

    let workers = node_runtime.block_on(async {
        let node = start_with_serving_runtime(config, Some(serving_runtime.handle().clone()))
            .await
            .unwrap();
        // Each monitor samples its runtime in a task of its own.
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let workers = node.runtime_workers();
                if workers.len() == 2 && workers.values().all(|count| *count > 0.0) {
                    return workers;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap()
    });

    assert_eq!(
        workers,
        BTreeMap::from([("iota_node".to_owned(), 1.0), ("serving".to_owned(), 2.0)])
    );
}

/// A node removes the database checkpoint directory that earlier releases
/// wrote under its database path.
#[tokio::test]
async fn a_node_removes_the_database_checkpoints_of_an_earlier_release() {
    let network = network_config();
    let config = fullnode_config(&network, FullnodeConfigBuilder::new());
    let legacy_db_checkpoints = config.db_path.join("db_checkpoints");
    std::fs::create_dir_all(legacy_db_checkpoints.join("epoch_0")).unwrap();

    start(config).await.unwrap();

    assert!(!legacy_db_checkpoints.exists());
}
