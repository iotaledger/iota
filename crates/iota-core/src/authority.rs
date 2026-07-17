// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, atomic::Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
    vec,
};

use arc_swap::{ArcSwap, Guard};
use async_trait::async_trait;
use authority_per_epoch_store::TxLockGuard;
pub use authority_store::{AuthorityStore, ResolverWrapper, UpdateType};
use fastcrypto::{
    encoding::{Base58, Encoding},
    hash::MultisetHash,
};
use iota_common::{debug_fatal, fatal};
use iota_config::{
    NodeConfig,
    genesis::Genesis,
    node::{AuthorityOverloadConfig, ExpensiveSafetyCheckConfig, StateDebugDumpConfig},
};
use iota_framework::{BuiltInFramework, SystemPackage as FrameworkSystemPackage};
use iota_json_rpc_types::{
    EventFilter, IotaEvent, IotaObjectDataFilter, IotaTransactionBlockEffects,
    IotaTransactionBlockEvents, TransactionFilter,
};
use iota_macros::{fail_point, fail_point_async, fail_point_if};
use iota_metrics::{
    TX_TYPE_SHARED_OBJ_TX, TX_TYPE_SINGLE_WRITER_TX, monitored_scope, spawn_monitored_task,
};
use iota_sdk_types::{
    Address, CheckpointContentsDigest, CheckpointDigest, Digest, EndOfEpochTransactionKind,
    ExecutionStatus, InputSharedObject, MoveAuthenticator, ObjectDigest, ObjectId, ObjectReference,
    RandomnessRound, SenderSignedTransaction, StructTag, SystemPackage, Transaction,
    TransactionDigest, TransactionEffects, TransactionEffectsDigest, TransactionEvents,
    TransactionKind, TypeTag, Version,
    checkpoint::{CheckpointCommitment, CheckpointContents, CheckpointSummary},
    crypto::{Intent, IntentScope},
    gas::GasCostSummary,
};
use iota_storage::{
    key_value_store::{
        KVStoreTransactionData, TransactionKeyValueStore, TransactionKeyValueStoreTrait,
    },
    key_value_store_metrics::KeyValueStoreMetrics,
};
use iota_traffic_controller::{TrafficController, metrics::TrafficControllerMetrics};
#[cfg(msim)]
use iota_types::committee::CommitteeTrait;
use iota_types::{
    account_abstraction::authenticator_function::{
        AuthenticatorFunctionRef, AuthenticatorFunctionRefForExecution,
        authenticator_function_ref_v1_from_dynamic_field_object,
        derive_authenticator_function_ref_v1_dynamic_field_id, extract_auth_fun_refs,
    },
    auth_context::AuthContextData,
    base_types::{AuthorityName, ConciseableName, ObjectInfo, ObjectType, VersionNumber},
    committee::{Committee, EpochId, ProtocolVersion},
    crypto::{AuthorityPublicKey, AuthoritySignInfo, AuthoritySignature, Signer},
    deny_list_v1::check_coin_deny_list_v1,
    deny_rule_governance::DenyRuleConfig,
    digests::ChainIdentifier,
    dynamic_field::DynamicFieldInfo,
    effects::{
        SignedTransactionEffects, TransactionEffectsAPI, TransactionEffectsExt,
        VerifiedSignedTransactionEffects,
    },
    error::{ExecutionError, IotaError, IotaResult, UserInputError},
    event::{EventID, SystemEpochInfoEvent},
    executable_transaction::VerifiedExecutableTransaction,
    execution_config_utils::to_binary_config,
    fp_ensure,
    full_checkpoint_content::CheckpointData,
    gas::IotaGasStatus,
    gas_coin::mock_simulation_gas_coin,
    inner_temporary_store::{InnerTemporaryStore, PackageStoreWithFallback},
    iota_system_state::{
        IotaSystemState, IotaSystemStateTrait,
        epoch_start_iota_system_state::EpochStartSystemStateTrait, get_iota_system_state,
    },
    layout_resolver::into_struct_layout,
    message_envelope::Message,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContentsExt, CheckpointRequest, CheckpointResponse,
        CheckpointSequenceNumber, CheckpointSummaryResponse, CheckpointTimestamp,
        ECMHLiveObjectSetDigest, VerifiedCheckpoint,
    },
    messages_consensus::AuthorityCapabilitiesV1,
    messages_grpc::{
        HandleTransactionResponse, LayoutGenerationOption, ObjectInfoRequest,
        ObjectInfoRequestKind, ObjectInfoResponse, TransactionInfoRequest, TransactionInfoResponse,
        TransactionStatus,
    },
    metrics::{BytecodeVerifierMetrics, LimitsMetrics},
    move_authenticator::MoveAuthenticatorExt,
    object::{Object, ObjectRead, PastObjectRead},
    storage::{BackingPackageStore, BackingStore, ObjectKey, ObjectOrTombstone, ObjectStore},
    supported_protocol_versions::{
        ProtocolConfig, SupportedProtocolVersions, SupportedProtocolVersionsWithHashes,
    },
    traffic_control::{PolicyConfig, RemoteFirewallConfig, TrafficControlReconfigParams},
    transaction::*,
    transaction_executor::{SimulateTransactionResult, VmChecks},
};
use itertools::Itertools;
use move_binary_format::{CompiledModule, binary_config::BinaryConfig};
use move_core_types::{
    account_address::AccountAddress, annotated_value::MoveStructLayout, language_storage::ModuleId,
};
use parking_lot::Mutex;
use prometheus_filtered::{
    Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec, MetricLevel,
    Registry, register_histogram_vec_with_registry, register_histogram_with_registry,
    register_int_counter_vec_with_registry, register_int_counter_with_registry,
    register_int_gauge_vec_with_registry, register_int_gauge_with_registry,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tap::TapFallible;
use tokio::{
    sync::{RwLock, mpsc, mpsc::unbounded_channel, oneshot},
    task::JoinHandle,
};
use tracing::{debug, error, info, instrument, trace, warn};
use typed_store::TypedStoreError;

use self::{
    authority_store::ExecutionLockWriteGuard, authority_store_pruner::AuthorityStorePruningMetrics,
};
#[cfg(msim)]
pub use crate::checkpoints::checkpoint_executor::utils::{
    CheckpointTimeoutConfig, init_checkpoint_timeout_config,
};
use crate::{
    authority::{
        authority_per_epoch_store::{AuthorityPerEpochStore, TxGuard},
        authority_per_epoch_store_pruner::AuthorityPerEpochStorePruner,
        authority_store::{ExecutionLockReadGuard, ObjectLockStatus},
        authority_store_pruner::{AuthorityStorePruner, EPOCH_DURATION_MS_FOR_TESTING},
        authority_store_tables::AuthorityPrunerTables,
        epoch_start_configuration::{EpochStartConfigTrait, EpochStartConfiguration},
        shared_object_version_manager::{AssignedVersions, Schedulable},
    },
    authority_client::NetworkAuthorityClient,
    checkpoint_progress_tracker::CheckpointProgressTracker,
    checkpoints::{CheckpointBuilderError, CheckpointBuilderResult, CheckpointStore},
    congestion_tracker::CongestionTracker,
    consensus_adapter::ConsensusAdapter,
    epoch::committee_store::CommitteeStore,
    execution_cache::{
        CheckpointCache, ExecutionCacheCommit, ExecutionCacheReconfigAPI,
        ExecutionCacheTraitPointers, ExecutionCacheWrite, ObjectCacheRead, StateSyncAPI,
        TransactionCacheRead,
    },
    execution_driver::execution_process,
    execution_scheduler::{ExecutionSchedulerAPI, ExecutionSchedulerWrapper},
    global_state_hasher::{GlobalStateHashStore, GlobalStateHasher},
    grpc_indexes::GrpcIndexesStore,
    jsonrpc_index::{CoinInfo, IndexStore},
    metrics::{LatencyObserver, RateTracker},
    module_cache_metrics::ResolverMetrics,
    overload_monitor::{
        AuthorityOverloadInfo, compute_graduated_load_shedding_percentage,
        overload_monitor_accept_tx,
    },
    stake_aggregator::StakeAggregator,
    subscription_handler::SubscriptionHandler,
    transaction_input_loader::TransactionInputLoader,
    transaction_outputs::TransactionOutputs,
    validator_tx_finalizer::ValidatorTxFinalizer,
    verify_indexes::verify_indexes,
};

#[cfg(test)]
#[path = "unit_tests/authority_tests.rs"]
pub mod authority_tests;

#[cfg(test)]
#[path = "unit_tests/transaction_tests.rs"]
pub mod transaction_tests;

#[cfg(test)]
#[path = "unit_tests/batch_transaction_tests.rs"]
mod batch_transaction_tests;

#[cfg(test)]
#[path = "unit_tests/move_integration_tests.rs"]
pub mod move_integration_tests;

#[cfg(test)]
#[path = "unit_tests/gas_tests.rs"]
mod gas_tests;

#[cfg(test)]
#[path = "unit_tests/batch_verification_tests.rs"]
mod batch_verification_tests;

#[cfg(test)]
#[path = "unit_tests/coin_deny_list_tests.rs"]
mod coin_deny_list_tests;

#[cfg(test)]
#[path = "unit_tests/auth_unit_test_utils.rs"]
pub mod auth_unit_test_utils;

#[cfg(any(test, feature = "test-utils"))]
pub mod authority_test_utils;

pub mod authority_per_epoch_store;
pub mod authority_per_epoch_store_pruner;

pub mod authority_store_pruner;
pub mod authority_store_tables;
pub mod authority_store_types;
pub mod epoch_start_configuration;
pub mod shared_object_congestion_tracker;
pub mod shared_object_version_manager;
pub mod suggested_gas_price_calculator;
#[cfg(any(test, feature = "test-utils"))]
pub mod test_authority_builder;
pub mod transaction_deferral;

pub(crate) mod authority_store;
pub mod backpressure;
pub(crate) mod dropped_tx_status_cache;

/// Prometheus metrics which can be displayed in Grafana, queried and alerted on
pub struct AuthorityMetrics {
    tx_orders: IntCounter,
    total_certs: IntCounter,
    total_cert_attempts: IntCounter,
    total_effects: IntCounter,
    pub shared_obj_tx: IntCounter,
    sponsored_tx: IntCounter,
    tx_already_processed: IntCounter,
    num_input_objs: Histogram,
    num_shared_objects: Histogram,
    batch_size: Histogram,

    authority_state_handle_transaction_latency: Histogram,

    execute_certificate_latency_single_writer: Histogram,
    execute_certificate_latency_shared_object: Histogram,

    internal_execution_latency: Histogram,
    /// Number of times the validator refused to report effects (signed or
    /// unsigned, labeled by RPC surface) because it had previously signed
    /// different effects for the same transaction.
    signed_effects_equivocation_prevented: IntCounterVec,
    execution_load_input_objects_latency: Histogram,
    prepare_certificate_latency: Histogram,
    commit_certificate_latency: Histogram,
    db_checkpoint_latency: Histogram,

    pub(crate) transaction_manager_num_enqueued_certificates: IntCounterVec,
    pub(crate) transaction_manager_num_missing_objects: IntGauge,
    pub(crate) transaction_manager_num_pending_certificates: IntGauge,
    pub(crate) transaction_manager_num_executing_certificates: IntGauge,
    pub(crate) transaction_manager_num_ready: IntGauge,
    pub(crate) transaction_manager_object_cache_size: IntGauge,
    pub(crate) transaction_manager_object_cache_hits: IntCounter,
    pub(crate) transaction_manager_object_cache_misses: IntCounter,
    pub(crate) transaction_manager_object_cache_evictions: IntCounter,
    pub(crate) transaction_manager_package_cache_size: IntGauge,
    pub(crate) transaction_manager_package_cache_hits: IntCounter,
    pub(crate) transaction_manager_package_cache_misses: IntCounter,
    pub(crate) transaction_manager_package_cache_evictions: IntCounter,
    pub(crate) transaction_manager_transaction_queue_age_s: Histogram,

    pub(crate) execution_driver_executed_transactions: IntCounter,
    pub(crate) execution_driver_dispatch_queue: IntGauge,
    pub(crate) execution_queueing_delay_s: Histogram,
    pub(crate) prepare_cert_gas_latency_ratio: Histogram,
    pub(crate) execution_gas_latency_ratio: Histogram,

    pub(crate) skipped_consensus_txns: IntCounter,
    pub(crate) skipped_consensus_txns_cache_hit: IntCounter,

    pub(crate) authority_overload_status: IntGauge,
    /// Percentage of transactions shed due to consensus queue length.
    pub(crate) consensus_queue_load_shedding_percentage: IntGauge,
    /// This authority's locally computed load shedding percentage, taken as the
    /// max of its latency/rate-based, transaction-manager-queue-based, and
    /// writeback-cache-backpressure signals.
    pub(crate) local_post_consensus_load_shedding_percentage: IntGauge,

    pub(crate) transaction_overload_sources: IntCounterVec,

    // Post processing metrics
    post_processing_total_events_emitted: IntCounter,
    post_processing_total_tx_had_event_processed: IntCounter,
    post_processing_total_failures: IntCounter,

    // Consensus handler metrics
    pub consensus_handler_processed: IntCounterVec,
    pub consensus_handler_transaction_sizes: HistogramVec,
    pub consensus_handler_num_low_scoring_authorities: IntGauge,
    pub consensus_handler_scores: IntGaugeVec,
    pub consensus_handler_deferred_transactions: IntCounter,
    pub consensus_handler_congested_transactions: IntCounter,
    pub consensus_handler_cancelled_transactions: IntCounter,
    /// Number of user transactions dropped during a consensus commit because
    /// post-consensus conflict/lock validation rejected them. Distinct from
    /// `consensus_handler_load_shedding_dropped_transactions`.
    pub consensus_handler_validation_dropped_transactions: IntCounter,
    /// Number of user transactions dropped during a consensus commit by
    /// post-consensus load shedding, i.e. probabilistically rejected at the
    /// quorum `consensus_handler_load_shedding_percentage` rate.
    pub consensus_handler_load_shedding_dropped_transactions: IntCounter,
    /// Stake-weighted quorum (2f+1) load shedding percentage enforced on user
    /// transactions in the most recent consensus commit. This is the cluster
    /// value actually applied post-consensus, as opposed to this authority's
    /// own `authority_load_shedding_percentage`. 0 when the P-COOL flow is
    /// disabled.
    pub consensus_handler_load_shedding_percentage: IntGauge,
    pub consensus_handler_max_object_costs: IntGaugeVec,
    pub consensus_committed_subdags: IntCounterVec,
    pub consensus_committed_messages: IntGaugeVec,
    pub consensus_committed_user_transactions: IntGaugeVec,
    pub consensus_handler_leader_round: IntGauge,
    pub consensus_calculated_throughput: IntGauge,
    pub consensus_calculated_throughput_profile: IntGauge,

    pub validator_scoreboard_scores: IntGaugeVec,
    pub invalid_misbehavior_reports_by_authority: IntGaugeVec,

    pub limits_metrics: Arc<LimitsMetrics>,

    /// bytecode verifier metrics for tracking timeouts
    pub bytecode_verifier_metrics: Arc<BytecodeVerifierMetrics>,

    /// Count of multisig signatures
    pub multisig_sig_count: IntCounter,

    // Tracks recent average txn queueing delay between when it is ready for execution
    // until it starts executing.
    pub execution_queueing_latency: LatencyObserver,

    // Tracks the rate at which transactions become ready for execution in the
    // scheduler. The need for the Mutex is that the tracker is updated in the
    // scheduler and read in the overload_monitor. There should be low mutex
    // contention because the update side is effectively single threaded and the
    // read rate in overload_monitor is low. If the update side becomes
    // multi-threaded, we can create one rate tracker per thread.
    pub txn_ready_rate_tracker: Arc<Mutex<RateTracker>>,

    // Tracks the rate of transactions starts execution in execution driver.
    // Similar reason for using a Mutex here as to `txn_ready_rate_tracker`.
    pub execution_rate_tracker: Arc<Mutex<RateTracker>>,
}

// Override default Prom buckets for positive numbers in 0-10M range
const POSITIVE_INT_BUCKETS: &[f64] = &[
    1., 2., 5., 7., 10., 20., 50., 70., 100., 200., 500., 700., 1000., 2000., 5000., 7000., 10000.,
    20000., 50000., 70000., 100000., 200000., 500000., 700000., 1000000., 2000000., 5000000.,
    7000000., 10000000.,
];

const LATENCY_SEC_BUCKETS: &[f64] = &[
    0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1., 2., 3., 4., 5., 6., 7., 8., 9.,
    10., 20., 30., 60., 90.,
];

// Buckets for low latency samples. Starts from 10us.
const LOW_LATENCY_SEC_BUCKETS: &[f64] = &[
    0.00001, 0.00002, 0.00005, 0.0001, 0.0002, 0.0005, 0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1,
    0.2, 0.5, 1., 2., 5., 10., 20., 50., 100.,
];

const GAS_LATENCY_RATIO_BUCKETS: &[f64] = &[
    10.0, 50.0, 100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0, 2000.0,
    3000.0, 4000.0, 5000.0, 6000.0, 7000.0, 8000.0, 9000.0, 10000.0, 50000.0, 100000.0, 1000000.0,
];

impl AuthorityMetrics {
    pub fn new(registry: &prometheus_filtered::Registry) -> AuthorityMetrics {
        let execute_certificate_latency = register_histogram_vec_with_registry!(
            "authority_state_execute_certificate_latency",
            "Latency of executing certificates, including waiting for inputs",
            &["tx_type"],
            LATENCY_SEC_BUCKETS.to_vec(),
            registry;
            MetricLevel::Info,
        )
        .unwrap();

        let execute_certificate_latency_single_writer =
            execute_certificate_latency.with_label_values(&[TX_TYPE_SINGLE_WRITER_TX]);
        let execute_certificate_latency_shared_object =
            execute_certificate_latency.with_label_values(&[TX_TYPE_SHARED_OBJ_TX]);

        Self {
            tx_orders: register_int_counter_with_registry!(
                "total_transaction_orders",
                "Total number of transaction orders",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            total_certs: register_int_counter_with_registry!(
                "total_transaction_certificates",
                "Total number of transaction certificates handled",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            total_cert_attempts: register_int_counter_with_registry!(
                "total_handle_certificate_attempts",
                "Number of calls to handle_certificate",
                registry,
            )
                .unwrap(),
            // total_effects == total transactions finished
            total_effects: register_int_counter_with_registry!(
                "total_transaction_effects",
                "Total number of transaction effects produced",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),

            shared_obj_tx: register_int_counter_with_registry!(
                "num_shared_obj_tx",
                "Number of transactions involving shared objects",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),

            sponsored_tx: register_int_counter_with_registry!(
                "num_sponsored_tx",
                "Number of sponsored transactions",
                registry,
            )
                .unwrap(),

            tx_already_processed: register_int_counter_with_registry!(
                "num_tx_already_processed",
                "Number of transaction orders already processed previously",
                registry,
            )
                .unwrap(),
            num_input_objs: register_histogram_with_registry!(
                "num_input_objects",
                "Distribution of number of input TX objects per TX",
                POSITIVE_INT_BUCKETS.to_vec(),
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            num_shared_objects: register_histogram_with_registry!(
                "num_shared_objects",
                "Number of shared input objects per TX",
                POSITIVE_INT_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            batch_size: register_histogram_with_registry!(
                "batch_size",
                "Distribution of size of transaction batch",
                POSITIVE_INT_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            authority_state_handle_transaction_latency: register_histogram_with_registry!(
                "authority_state_handle_transaction_latency",
                "Latency of handling transactions",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            execute_certificate_latency_single_writer,
            execute_certificate_latency_shared_object,
            internal_execution_latency: register_histogram_with_registry!(
                "authority_state_internal_execution_latency",
                "Latency of actual certificate executions",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            signed_effects_equivocation_prevented: register_int_counter_vec_with_registry!(
                "authority_state_signed_effects_equivocation_prevented",
                "Number of times the validator refused to report effects that differ from previously signed effects for the same transaction, by RPC surface",
                &["surface"],
                registry,
            )
            .unwrap(),
            execution_load_input_objects_latency: register_histogram_with_registry!(
                "authority_state_execution_load_input_objects_latency",
                "Latency of loading input objects for execution",
                LOW_LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            prepare_certificate_latency: register_histogram_with_registry!(
                "authority_state_prepare_certificate_latency",
                "Latency of executing certificates, before committing the results",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            commit_certificate_latency: register_histogram_with_registry!(
                "authority_state_commit_certificate_latency",
                "Latency of committing certificate execution results",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            )
                .unwrap(),
            db_checkpoint_latency: register_histogram_with_registry!(
                "db_checkpoint_latency",
                "Latency of checkpointing the perpetual store at epoch end",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry,
            ).unwrap(),
            transaction_manager_num_enqueued_certificates: register_int_counter_vec_with_registry!(
                "transaction_manager_num_enqueued_certificates",
                "Current number of certificates enqueued to TransactionManager",
                &["result"],
                registry,
            )
                .unwrap(),
            transaction_manager_num_missing_objects: register_int_gauge_with_registry!(
                "transaction_manager_num_missing_objects",
                "Current number of missing objects in TransactionManager",
                registry,
            )
                .unwrap(),
            transaction_manager_num_pending_certificates: register_int_gauge_with_registry!(
                "transaction_manager_num_pending_certificates",
                "Number of certificates pending in TransactionManager, with at least 1 missing input object",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            transaction_manager_num_executing_certificates: register_int_gauge_with_registry!(
                "transaction_manager_num_executing_certificates",
                "Number of executing certificates, including queued and actually running certificates",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            transaction_manager_num_ready: register_int_gauge_with_registry!(
                "transaction_manager_num_ready",
                "Number of ready transactions in TransactionManager",
                registry,
            )
                .unwrap(),
            transaction_manager_object_cache_size: register_int_gauge_with_registry!(
                "transaction_manager_object_cache_size",
                "Current size of object-availability cache in TransactionManager",
                registry,
            )
                .unwrap(),
            transaction_manager_object_cache_hits: register_int_counter_with_registry!(
                "transaction_manager_object_cache_hits",
                "Number of object-availability cache hits in TransactionManager",
                registry,
            )
                .unwrap(),
            authority_overload_status: register_int_gauge_with_registry!(
                "authority_overload_status",
                "Whether authority is current experiencing overload and enters load shedding mode.",
                registry;
                MetricLevel::Warn,)
                .unwrap(),
            local_post_consensus_load_shedding_percentage: register_int_gauge_with_registry!(
                "authority_load_shedding_percentage",
                "This authority's locally computed load shedding percentage. In the P-COOL flow this is the value broadcast to peers, not necessarily the rate enforced (see consensus_handler_load_shedding_percentage).",
                registry;
                MetricLevel::Info,)
                .unwrap(),
            consensus_queue_load_shedding_percentage: register_int_gauge_with_registry!(
                "consensus_queue_load_shedding_percentage",
                "Percentage of transactions shed due to consensus queue length. Separate admission-control signal, not an input to authority_load_shedding_percentage.",
                registry)
                .unwrap(),
            transaction_manager_object_cache_misses: register_int_counter_with_registry!(
                "transaction_manager_object_cache_misses",
                "Number of object-availability cache misses in TransactionManager",
                registry,
            )
                .unwrap(),
            transaction_manager_object_cache_evictions: register_int_counter_with_registry!(
                "transaction_manager_object_cache_evictions",
                "Number of object-availability cache evictions in TransactionManager",
                registry,
            )
                .unwrap(),
            transaction_manager_package_cache_size: register_int_gauge_with_registry!(
                "transaction_manager_package_cache_size",
                "Current size of package-availability cache in TransactionManager",
                registry,
            )
                .unwrap(),
            transaction_manager_package_cache_hits: register_int_counter_with_registry!(
                "transaction_manager_package_cache_hits",
                "Number of package-availability cache hits in TransactionManager",
                registry,
            )
                .unwrap(),
            transaction_manager_package_cache_misses: register_int_counter_with_registry!(
                "transaction_manager_package_cache_misses",
                "Number of package-availability cache misses in TransactionManager",
                registry,
            )
                .unwrap(),
            transaction_manager_package_cache_evictions: register_int_counter_with_registry!(
                "transaction_manager_package_cache_evictions",
                "Number of package-availability cache evictions in TransactionManager",
                registry,
            )
                .unwrap(),
            transaction_manager_transaction_queue_age_s: register_histogram_with_registry!(
                "transaction_manager_transaction_queue_age_s",
                "Time spent in waiting for transaction in the queue",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            transaction_overload_sources: register_int_counter_vec_with_registry!(
                "transaction_overload_sources",
                "Number of times each source indicates transaction overload.",
                &["source"],
                registry)
                .unwrap(),
            execution_driver_executed_transactions: register_int_counter_with_registry!(
                "execution_driver_executed_transactions",
                "Cumulative number of transaction executed by execution driver",
                registry;
                MetricLevel::Warn,
            )
                .unwrap(),
            execution_driver_dispatch_queue: register_int_gauge_with_registry!(
                "execution_driver_dispatch_queue",
                "Number of transaction pending in execution driver dispatch queue",
                registry,
            )
                .unwrap(),
            execution_queueing_delay_s: register_histogram_with_registry!(
                "execution_queueing_delay_s",
                "Queueing delay between a transaction is ready for execution until it starts executing.",
                LATENCY_SEC_BUCKETS.to_vec(),
                registry
            )
                .unwrap(),
            prepare_cert_gas_latency_ratio: register_histogram_with_registry!(
                "prepare_cert_gas_latency_ratio",
                "The ratio of computation gas divided by VM execution latency.",
                GAS_LATENCY_RATIO_BUCKETS.to_vec(),
                registry
            )
                .unwrap(),
            execution_gas_latency_ratio: register_histogram_with_registry!(
                "execution_gas_latency_ratio",
                "The ratio of computation gas divided by certificate execution latency, include committing certificate.",
                GAS_LATENCY_RATIO_BUCKETS.to_vec(),
                registry
            )
                .unwrap(),
            skipped_consensus_txns: register_int_counter_with_registry!(
                "skipped_consensus_txns",
                "Total number of consensus transactions skipped",
                registry,
            )
                .unwrap(),
            skipped_consensus_txns_cache_hit: register_int_counter_with_registry!(
                "skipped_consensus_txns_cache_hit",
                "Total number of consensus transactions skipped because of local cache hit",
                registry,
            )
                .unwrap(),
            post_processing_total_events_emitted: register_int_counter_with_registry!(
                "post_processing_total_events_emitted",
                "Total number of events emitted in post processing",
                registry,
            )
                .unwrap(),
            post_processing_total_tx_had_event_processed: register_int_counter_with_registry!(
                "post_processing_total_tx_had_event_processed",
                "Total number of txes finished event processing in post processing",
                registry,
            )
                .unwrap(),
            post_processing_total_failures: register_int_counter_with_registry!(
                "post_processing_total_failures",
                "Total number of failure in post processing",
                registry,
            )
                .unwrap(),
            consensus_handler_processed: register_int_counter_vec_with_registry!(
                "consensus_handler_processed",
                "Number of transactions processed by consensus handler",
                &["class"],
                registry
            ).unwrap(),
            consensus_handler_transaction_sizes: register_histogram_vec_with_registry!(
                "consensus_handler_transaction_sizes",
                "Sizes of each type of transactions processed by consensus handler",
                &["class"],
                POSITIVE_INT_BUCKETS.to_vec(),
                registry;
                MetricLevel::Warn,
            ).unwrap(),
            consensus_handler_num_low_scoring_authorities: register_int_gauge_with_registry!(
                "consensus_handler_num_low_scoring_authorities",
                "Number of low scoring authorities based on reputation scores from consensus",
                registry
            ).unwrap(),
            consensus_handler_scores: register_int_gauge_vec_with_registry!(
                "consensus_handler_scores",
                "scores from consensus for each authority",
                &["authority"],
                registry,
            ).unwrap(),
            validator_scoreboard_scores: register_int_gauge_vec_with_registry!(
                "validator_scoreboard_scores",
                "Per-authority validator scores published by the local Scoreboard after each consensus commit. Range [0, MAX_SCORE].",
                &["authority"],
                registry;
                MetricLevel::Warn,
            ).unwrap(),
            invalid_misbehavior_reports_by_authority: register_int_gauge_vec_with_registry!(
                "invalid_misbehavior_reports_by_authority",
                "Cumulative count of invalid misbehavior reports received from each reporting authority in the current epoch. Bumped when a `MisbehaviorReport` consensus transaction fails sender/authority match or payload validation. Snapshot republished after each consensus commit.",
                &["authority"],
                registry;
                MetricLevel::Warn,
            ).unwrap(),
            consensus_handler_deferred_transactions: register_int_counter_with_registry!(
                "consensus_handler_deferred_transactions",
                "Number of transactions deferred by consensus handler",
                registry,
            ).unwrap(),
            consensus_handler_congested_transactions: register_int_counter_with_registry!(
                "consensus_handler_congested_transactions",
                "Number of transactions deferred by consensus handler due to congestion",
                registry,
            ).unwrap(),
            consensus_handler_cancelled_transactions: register_int_counter_with_registry!(
                "consensus_handler_cancelled_transactions",
                "Number of transactions cancelled by consensus handler",
                registry,
            ).unwrap(),
            consensus_handler_validation_dropped_transactions: register_int_counter_with_registry!(
                "consensus_handler_validation_dropped_transactions",
                "Number of UserTransactionV1 transactions dropped by post-consensus validation",
                registry,
            ).unwrap(),
            consensus_handler_load_shedding_dropped_transactions: register_int_counter_with_registry!(
                "consensus_handler_load_shedding_dropped_transactions",
                "Number of user transactions dropped by post-consensus load shedding, based on the quorum load shedding percentage",
                registry,
            ).unwrap(),
            consensus_handler_load_shedding_percentage: register_int_gauge_with_registry!(
                "consensus_handler_load_shedding_percentage",
                "Stake-weighted quorum (2f+1) load shedding percentage enforced on user transactions in the most recent consensus commit. 0 when the P-COOL flow is disabled.",
                registry,
            ).unwrap(),
            consensus_handler_max_object_costs: register_int_gauge_vec_with_registry!(
                "consensus_handler_max_congestion_control_object_costs",
                "Max object costs for congestion control in the current consensus commit",
                &["commit_type"],
                registry,
            ).unwrap(),
            consensus_committed_subdags: register_int_counter_vec_with_registry!(
                "consensus_committed_subdags",
                "Number of committed subdags, sliced by leader",
                &["authority"],
                registry,
            ).unwrap(),
            consensus_committed_messages: register_int_gauge_vec_with_registry!(
                "consensus_committed_messages",
                "Total number of committed consensus messages, sliced by author",
                &["authority"],
                registry;
                MetricLevel::Warn,
            ).unwrap(),
            consensus_committed_user_transactions: register_int_gauge_vec_with_registry!(
                "consensus_committed_user_transactions",
                "Number of committed user transactions, sliced by submitter",
                &["authority"],
                registry,
            ).unwrap(),
            consensus_handler_leader_round: register_int_gauge_with_registry!(
                "consensus_handler_leader_round",
                "The leader round of the current consensus output being processed in the consensus handler",
                registry;
                MetricLevel::Warn,
            ).unwrap(),
            limits_metrics: Arc::new(LimitsMetrics::new(registry)),
            bytecode_verifier_metrics: Arc::new(BytecodeVerifierMetrics::new(registry)),
            multisig_sig_count: register_int_counter_with_registry!(
                "multisig_sig_count",
                "Count of multisig signatures",
                registry,
            )
                .unwrap(),
            consensus_calculated_throughput: register_int_gauge_with_registry!(
                "consensus_calculated_throughput",
                "The calculated throughput from consensus output. Result is calculated based on unique transactions.",
                registry,
            ).unwrap(),
            consensus_calculated_throughput_profile: register_int_gauge_with_registry!(
                "consensus_calculated_throughput_profile",
                "The current active calculated throughput profile",
                registry
            ).unwrap(),
            execution_queueing_latency: LatencyObserver::new(),
            txn_ready_rate_tracker: Arc::new(Mutex::new(RateTracker::new(Duration::from_secs(10)))),
            execution_rate_tracker: Arc::new(Mutex::new(RateTracker::new(Duration::from_secs(10)))),
        }
    }

    /// Reset metrics that contain `hostname` as one of the labels. This is
    /// needed to avoid retaining metrics for long-gone committee members and
    /// only exposing metrics for the committee in the current epoch.
    pub fn reset_on_reconfigure(&self) {
        self.consensus_committed_messages.reset();
        self.consensus_handler_scores.reset();
        self.validator_scoreboard_scores.reset();
        self.invalid_misbehavior_reports_by_authority.reset();
        self.consensus_committed_user_transactions.reset();
    }
}

/// a Trait object for `Signer` that is:
/// - Pin, i.e. confined to one place in memory (we don't want to copy private
///   keys).
/// - Sync, i.e. can be safely shared between threads.
///
/// Typically instantiated with Box::pin(keypair) where keypair is a `KeyPair`
pub type StableSyncAuthoritySigner = Pin<Arc<dyn Signer<AuthoritySignature> + Send + Sync>>;

/// Execution env contains the "environment" for the transaction to be executed
/// in, that is, all the information necessary for execution that is not
/// specified by the transaction itself.
#[derive(Debug, Clone, Default)]
pub struct ExecutionEnv {
    /// The assigned version of each shared object for the transaction.
    pub assigned_versions: AssignedVersions,
    /// The expected digest of the effects of the transaction, if executing from
    /// checkpoint or other sources where the effects are known in advance.
    pub expected_effects_digest: Option<TransactionEffectsDigest>,
}

impl ExecutionEnv {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_expected_effects_digest(
        mut self,
        expected_effects_digest: TransactionEffectsDigest,
    ) -> Self {
        self.expected_effects_digest = Some(expected_effects_digest);
        self
    }

    pub fn with_assigned_versions(mut self, assigned_versions: AssignedVersions) -> Self {
        if !assigned_versions.is_empty() {
            self.assigned_versions = assigned_versions;
        }
        self
    }
}

pub struct AuthorityState {
    // Fixed size, static, identity of the authority
    /// The name of this authority.
    pub name: AuthorityName,
    /// The signature key of the authority.
    pub secret: StableSyncAuthoritySigner,

    /// The database
    input_loader: TransactionInputLoader,
    execution_cache_trait_pointers: ExecutionCacheTraitPointers,

    epoch_store: ArcSwap<AuthorityPerEpochStore>,

    /// This lock denotes current 'execution epoch'.
    /// Execution acquires read lock, checks transaction epoch and holds it
    /// until all writes are complete. Reconfiguration acquires write lock,
    /// changes the epoch and revert all transactions from previous epoch
    /// that are executed but did not make into checkpoint.
    execution_lock: RwLock<EpochId>,

    pub indexes: Option<Arc<IndexStore>>,
    pub grpc_indexes_store: Option<Arc<GrpcIndexesStore>>,

    pub subscription_handler: Arc<SubscriptionHandler>,
    pub checkpoint_store: Arc<CheckpointStore>,

    committee_store: Arc<CommitteeStore>,

    /// Schedules transaction execution.
    execution_scheduler: Arc<ExecutionSchedulerWrapper>,

    /// Shuts down the execution task. Used only in testing.
    #[cfg_attr(not(test), expect(unused))]
    tx_execution_shutdown: Mutex<Option<oneshot::Sender<()>>>,

    pub metrics: Arc<AuthorityMetrics>,
    /// The store pruner. The checkpoint executor uses it to nudge the pruner
    /// after each checkpoint.
    pruner: AuthorityStorePruner,
    authority_per_epoch_pruner: AuthorityPerEpochStorePruner,
    checkpoint_progress_tracker: Option<Arc<CheckpointProgressTracker>>,

    pub config: NodeConfig,

    /// Current overload status in this authority. Updated periodically.
    pub overload_info: AuthorityOverloadInfo,

    pub validator_tx_finalizer: Option<Arc<ValidatorTxFinalizer<NetworkAuthorityClient>>>,

    /// The chain identifier is derived from the digest of the genesis
    /// checkpoint.
    chain_identifier: ChainIdentifier,

    pub(crate) congestion_tracker: Arc<CongestionTracker>,

    /// Traffic controller for IOTA core servers (json-rpc, validator service)
    pub traffic_controller: Option<Arc<TrafficController>>,
}

/// The authority state encapsulates all state, drives execution, and ensures
/// safety.
///
/// Note the authority operations can be accessed through a read ref (&) and do
/// not require &mut. Internally a database is synchronized through a mutex
/// lock.
///
/// Repeating valid commands should produce no changes and return no error.
impl AuthorityState {
    pub fn is_committee_validator(&self, epoch_store: &AuthorityPerEpochStore) -> bool {
        epoch_store.committee().authority_exists(&self.name)
    }

    pub fn is_active_validator(&self, epoch_store: &AuthorityPerEpochStore) -> bool {
        epoch_store
            .active_validators()
            .iter()
            .any(|a| AuthorityName::from(a) == self.name)
    }

    pub fn is_fullnode(&self, epoch_store: &AuthorityPerEpochStore) -> bool {
        !self.is_committee_validator(epoch_store)
    }

    pub fn committee_store(&self) -> &Arc<CommitteeStore> {
        &self.committee_store
    }

    pub fn clone_committee_store(&self) -> Arc<CommitteeStore> {
        self.committee_store.clone()
    }

    pub fn overload_config(&self) -> &AuthorityOverloadConfig {
        &self.config.authority_overload_config
    }

    pub fn get_epoch_state_commitments(
        &self,
        epoch: EpochId,
    ) -> IotaResult<Option<Vec<CheckpointCommitment>>> {
        self.checkpoint_store.get_epoch_state_commitments(epoch)
    }

    /// Runs deny list, input object validation, gas checks, coin deny list, and
    /// MoveAuthenticator checks. Returns the owned object refs for optional
    /// version validation. Does NOT acquire locks or sign the transaction.
    ///
    /// `deny_config` is the deny rule source to enforce, chosen per caller:
    /// the local config alone, the local config combined with the governance
    /// rules (admission), or the governance-derived active set alone
    /// (post-consensus) — the latter two when `deny_rule_governance` is
    /// enabled.
    ///
    /// `epoch_gated_coin_deny_list` selects how the coin deny list is read:
    /// `false` reads the latest value, so denials apply immediately - for
    /// validator-local admission (signing); `true` reads the value settled
    /// before the current epoch, which is deterministic across validators
    /// regardless of each validator's execution progress - required
    /// post-consensus, where the verdict decides whether the transaction
    /// stays in the committed set. The two read modes intentionally disagree
    /// about deny-list changes made in the current epoch, in both directions:
    /// - An entry added this epoch is enforced at admission right away, while
    ///   the epoch-gated layers enforce it only from the next epoch. Since
    ///   execution and post-consensus must read epoch-gated to stay
    ///   deterministic, admission is the only layer that can react to a new
    ///   denial or global pause before the epoch boundary.
    /// - An entry removed this epoch is admitted right away but still denied by
    ///   the epoch-gated post-consensus read, so such transactions are
    ///   sequenced by consensus and then deterministically dropped (no
    ///   execution, no gas charged) until the removal settles at the next epoch
    ///   boundary. The wasted consensus slot is accepted: post-consensus must
    ///   handle deterministic drops regardless (owned-object double-spend
    ///   losers, for example), and validators that skip admission can put such
    ///   transactions into their blocks anyway, so no admission policy can
    ///   limit how many deterministically-dropped transactions reach consensus.
    #[instrument(level = "trace", skip_all, fields(tx_digest = ?transaction.digest()))]
    pub(crate) async fn handle_transaction_validation_checks(
        &self,
        transaction: &VerifiedTransaction,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        deny_config: &dyn DenyRuleConfig,
        epoch_gated_coin_deny_list: bool,
    ) -> IotaResult<Vec<ObjectReference>> {
        let protocol_config = epoch_store.protocol_config();
        let reference_gas_price = epoch_store.reference_gas_price();

        let epoch = epoch_store.epoch();

        let tx = transaction.data().transaction();

        // Note: the deny checks may do redundant package loads but:
        // - they only load packages when there is an active package deny map
        // - the loads are cached anyway
        iota_transaction_checks::deny::check_transaction_for_validation(
            tx,
            transaction.signatures(),
            &transaction.input_objects()?,
            &tx.receiving_objects(),
            deny_config,
            self.get_backing_package_store().as_ref(),
        )?;

        // Load all transaction-related input objects including ones for every
        // `MoveAuthenticator`. Loading all objects eagerly means that any invalid
        // reference — missing object, wrong version, inaccessible object — causes a
        // pre-consensus rejection.
        let (tx_input_objects, tx_receiving_objects, per_authenticator_inputs) =
            self.read_objects_for_validation(transaction, epoch)?;

        let move_authenticators = transaction.move_authenticators();

        // Check the inputs for signing.
        // If there are `MoveAuthenticator` signatures, their input objects and the
        // account objects are also checked and must be provided.
        // It is also checked if there is enough gas to execute the transaction and its
        // authenticators.
        let (gas_status, tx_checked_input_objects, per_authenticator_checked_inputs) = self
            .check_transaction_inputs_for_validation(
                protocol_config,
                reference_gas_price,
                tx,
                tx_input_objects,
                &tx_receiving_objects,
                &move_authenticators,
                per_authenticator_inputs,
            )?;

        // Get the input objects for the authenticators, if there are
        // `MoveAuthenticator`s.
        let per_authenticator_checked_input_objects: Vec<_> = per_authenticator_checked_inputs
            .iter()
            .map(|i| &i.0)
            .collect();

        // Move authenticators cannot use owned objects, so their inputs never
        // acquire owned-object locks.
        debug_assert!(
            per_authenticator_checked_input_objects
                .iter()
                .all(|objects| objects.inner().filter_owned_objects().is_empty()),
            "Move authenticator input objects must not contain owned objects"
        );

        // Check if any of the sender, the transaction input objects, the receiving
        // objects and the authenticator input objects are in the coin deny
        // list, which would prevent the transaction from being signed.
        check_coin_deny_list_v1(
            tx.sender(),
            &tx_checked_input_objects,
            &tx_receiving_objects,
            &per_authenticator_checked_input_objects,
            &self.get_object_store(),
            epoch_gated_coin_deny_list.then_some(epoch),
        )?;

        let (kind, signer, gas_data) = tx.execution_parts();

        let (sender_authenticator_function_ref, sponsor_authenticator_function_ref) =
            extract_auth_fun_refs(signer, gas_data.owner, |address| {
                move_authenticators
                    .iter()
                    .zip(per_authenticator_checked_inputs.iter())
                    .find(|(move_authenticator, _)| move_authenticator.address() == address)
                    .map(|(_, (_, auth_fun_ref))| auth_fun_ref.clone())
            });

        // Filter the authenticators and their checked inputs down to those that must
        // be executed pre-consensus. This is done *after* the deny-list check so
        // that all MoveAuthenticator input objects are covered by that check regardless
        // of deferral.
        let pre_consensus_move_authenticators =
            pre_consensus_move_authenticators(transaction, protocol_config);
        // Asserted before the zip below pairs them positionally; the two lists
        // come from independent computations.
        debug_assert_eq!(
            move_authenticators.len(),
            per_authenticator_checked_inputs.len(),
            "Move authenticators amount must match the number of checked authenticator inputs"
        );
        let (move_authenticators, per_authenticator_checked_inputs): (Vec<_>, Vec<_>) =
            move_authenticators
                .into_iter()
                .zip(per_authenticator_checked_inputs)
                .filter(|(a, _)| pre_consensus_move_authenticators.contains(a))
                .unzip();
        let per_authenticator_checked_input_objects: Vec<_> = per_authenticator_checked_inputs
            .iter()
            .map(|i| &i.0)
            .collect();

        // If there are `MoveAuthenticator` signatures, execute them and check if they
        // all succeed.
        if !move_authenticators.is_empty() {
            let aggregated_authenticator_input_objects =
                iota_transaction_checks::aggregate_authenticator_input_objects(
                    &per_authenticator_checked_input_objects,
                )?;

            let move_authenticators = move_authenticators
                .into_iter()
                .zip(per_authenticator_checked_inputs)
                .map(
                    |(
                        move_authenticator,
                        (authenticator_checked_input_objects, authenticator_function_ref),
                    )| {
                        (
                            move_authenticator.to_owned(),
                            authenticator_function_ref,
                            authenticator_checked_input_objects,
                        )
                    },
                )
                .collect();

            // It is supposed that `MoveAuthenticator` availability is checked in
            // `SenderSignedTransaction::validity_check`.

            // Serialize the Transaction for the auth context before decomposing.
            let tx_bytes = bcs::to_bytes(tx).expect("Transaction serialization cannot fail");

            let (sender_auth_digest, sponsor_auth_digest) =
                transaction.data().compute_auth_digests()?;

            let auth_context_data = AuthContextData {
                transaction_data_bytes: tx_bytes,
                sender_auth_digest,
                sponsor_auth_digest,
                sender_authenticator_function_ref,
                sponsor_authenticator_function_ref,
            };

            // Execute the Move authenticators.
            let validation_result = epoch_store.executor().authenticate_transaction(
                self.get_backing_store().as_ref(),
                protocol_config,
                self.metrics.limits_metrics.clone(),
                &epoch_store.epoch_start_config().epoch_data().epoch_id(),
                epoch_store
                    .epoch_start_config()
                    .epoch_data()
                    .epoch_start_timestamp(),
                gas_data,
                gas_status,
                move_authenticators,
                aggregated_authenticator_input_objects,
                kind,
                signer,
                transaction.digest().to_owned(),
                auth_context_data,
                &mut None,
            );

            if let Err(validation_error) = validation_result {
                return Err(IotaError::MoveAuthenticatorExecutionFailure {
                    error: validation_error.to_string(),
                });
            }
        }

        Ok(tx_checked_input_objects.inner().filter_owned_objects())
    }

    /// This is a private method and should be kept that way. It doesn't check
    /// whether the provided transaction is a system transaction, and hence
    /// can only be called internally.
    async fn handle_transaction_impl(
        &self,
        transaction: VerifiedTransaction,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<VerifiedSignedTransaction> {
        // Ensure that validator cannot reconfigure while we are signing the tx
        let _execution_lock = self.execution_lock_for_signing()?;

        let owned_objects = self
            .handle_transaction_validation_checks(
                &transaction,
                epoch_store,
                &self.config.transaction_deny_config,
                // Latest-value coin deny-list read: admission is validator-local,
                // and denials should take effect immediately. Unlike the P-COOL
                // submission path, no post-consensus re-check follows - this is
                // the only sender-side coin deny check in the certificate flow.
                false,
            )
            .await?;

        let epoch = epoch_store.epoch();
        let signed_transaction =
            VerifiedSignedTransaction::new(epoch, transaction, self.name, &*self.secret);

        // Check and write locks, to signed transaction, into the database
        // The call to self.set_transaction_lock checks the lock is not conflicting,
        // and returns ConflictingTransaction error in case there is a lock on a
        // different existing transaction.
        self.get_cache_writer().try_acquire_transaction_locks(
            epoch_store,
            &owned_objects,
            signed_transaction.clone(),
        )?;

        Ok(signed_transaction)
    }

    /// Initiate a new transaction.
    #[instrument(name = "handle_transaction", level = "trace", skip_all, fields(tx_digest = ?transaction.digest(), sender = transaction.data().transaction().gas_owner().to_string()
    ))]
    pub async fn handle_transaction(
        &self,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        transaction: VerifiedTransaction,
    ) -> IotaResult<HandleTransactionResponse> {
        let tx_digest = *transaction.digest();
        debug!("handle_transaction");

        // Ensure an idempotent answer.
        if let Some((_, status)) = self.get_transaction_status(&tx_digest, epoch_store)? {
            return Ok(HandleTransactionResponse { status });
        }

        let _metrics_guard = self
            .metrics
            .authority_state_handle_transaction_latency
            .start_timer();
        self.metrics.tx_orders.inc();

        let signed = self.handle_transaction_impl(transaction, epoch_store).await;
        match signed {
            Ok(s) => {
                if self.is_committee_validator(epoch_store) {
                    if let Some(validator_tx_finalizer) = &self.validator_tx_finalizer {
                        let tx = s.clone();
                        let validator_tx_finalizer = validator_tx_finalizer.clone();
                        let cache_reader = self.get_transaction_cache_reader().clone();
                        let epoch_store = epoch_store.clone();
                        spawn_monitored_task!(epoch_store.within_alive_epoch(
                            validator_tx_finalizer.track_signed_tx(cache_reader, &epoch_store, tx)
                        ));
                    }
                }
                Ok(HandleTransactionResponse {
                    status: TransactionStatus::Signed(s.into_inner().into_sig()),
                })
            }
            // It happens frequently that while we are checking the validity of the transaction, it
            // has just been executed.
            // In that case, we could still return Ok to avoid showing confusing errors.
            Err(err) => Ok(HandleTransactionResponse {
                status: self
                    .get_transaction_status(&tx_digest, epoch_store)?
                    .ok_or(err)?
                    .1,
            }),
        }
    }

    pub fn check_system_overload_at_signing(&self) -> bool {
        self.config
            .authority_overload_config
            .check_system_overload_at_signing
    }

    pub fn check_system_overload_at_execution(&self) -> bool {
        self.config
            .authority_overload_config
            .check_system_overload_at_execution
    }

    /// Checks system overload conditions before accepting a transaction.
    ///
    /// In certificate-less (P-COOL) mode: only checks consensus
    /// queue overload, since execution-based overload will be handled
    /// post-consensus.
    ///
    /// In certificate mode: runs all checks — authority overload
    /// (execution latency), the execution scheduler (execution queue),
    /// consensus adapter (queue limit), and writeback cache backpressure.
    pub(crate) fn check_system_overload(
        &self,
        consensus_adapter: &Arc<ConsensusAdapter>,
        tx: &SenderSignedTransaction,
        do_authority_overload_check: bool,
        pcool_flow_enabled: bool,
    ) -> IotaResult {
        if pcool_flow_enabled {
            // Graduated shedding: 0% to 100% as consensus queue fills from soft
            // to hard limit.
            self.check_consensus_queue_graduated_limits(consensus_adapter, tx)
                .tap_err(|_| {
                    self.update_overload_metrics("consensus");
                })?;

            // NOTE: graduated shedding at 100% already rejects everything at or above
            // `max_pending_transactions`, so the queue-length part of the check below
            // is redundant but harmless. But `check_consensus_overload()` should be
            // kept here because it also verifies that `submit_semaphore` has permits
            // (see `check_consensus_hard_limits` in consensus_adapter.rs), which is a
            // separate concurrency limit not covered by the graduated shedding.
            consensus_adapter.check_consensus_overload().tap_err(|_| {
                self.update_overload_metrics("consensus");
            })?;
        } else {
            if do_authority_overload_check {
                self.check_authority_overload(tx).tap_err(|_| {
                    self.update_overload_metrics("execution_queue");
                })?;
            }
            self.execution_scheduler
                .check_execution_overload(self.overload_config(), tx)
                .tap_err(|_| {
                    self.update_overload_metrics("execution_pending");
                })?;
            consensus_adapter.check_consensus_overload().tap_err(|_| {
                self.update_overload_metrics("consensus");
            })?;

            let pending_tx_count = self
                .get_cache_commit()
                .approximate_pending_transaction_count();
            if pending_tx_count
                > self
                    .config
                    .execution_cache_config
                    .writeback_cache
                    .backpressure_threshold_for_rpc()
            {
                return Err(IotaError::ValidatorOverloadedRetryAfter {
                    retry_after_secs: 10,
                });
            }
        }

        Ok(())
    }

    /// Rejects `tx_data` via graduated shedding based on consensus queue
    /// length. Scales from 0% at the soft limit to 100% at
    /// `max_pending_transactions`. Returns `ValidatorOverloadedRetryAfter`
    /// for probabilistic rejection (shedding percentage < 100%, via
    /// `overload_monitor_accept_tx`) or `TooManyTransactionsPendingConsensus`
    /// for unconditional rejection (shedding percentage >= 100%). Updates
    /// `consensus_queue_load_shedding_percentage` metric.
    fn check_consensus_queue_graduated_limits(
        &self,
        consensus_adapter: &Arc<ConsensusAdapter>,
        tx: &SenderSignedTransaction,
    ) -> IotaResult {
        let num_inflight_txs = consensus_adapter.num_inflight_transactions() as usize;

        let shedding_pct = compute_graduated_load_shedding_percentage(
            num_inflight_txs,
            consensus_adapter.max_pending_transactions(),
            consensus_adapter.graduated_load_shedding_soft_limit_pct(),
        );

        self.metrics
            .consensus_queue_load_shedding_percentage
            .set(shedding_pct as i64);

        if shedding_pct == 0 {
            return Ok(());
        }

        // At/above the hard limit, rejection is unconditional (not
        // probabilistic), so the seed-rotation retry hint of
        // `ValidatorOverloadedRetryAfter` doesn't apply - return the
        // capacity-bound error instead.
        if shedding_pct >= 100 {
            return Err(IotaError::TooManyTransactionsPendingConsensus);
        }

        overload_monitor_accept_tx(shedding_pct, tx.digest())
    }

    fn check_authority_overload(&self, tx: &SenderSignedTransaction) -> IotaResult {
        if !self.overload_info.is_overload.load(Ordering::Relaxed) {
            return Ok(());
        }

        let load_shedding_percentage = self
            .overload_info
            .local_load_shedding_percentage
            .load(Ordering::Relaxed);
        overload_monitor_accept_tx(load_shedding_percentage, tx.digest())
    }

    fn update_overload_metrics(&self, source: &str) {
        self.metrics
            .transaction_overload_sources
            .with_label_values(&[source])
            .inc();
    }

    /// Wait for a certificate to be executed.
    /// For consensus transactions, it needs to be sequenced by the consensus.
    /// For owned object transactions, this function will enqueue the
    /// transaction for execution.
    #[instrument(level = "trace", skip_all)]
    pub async fn wait_for_certificate_execution(
        &self,
        certificate: &VerifiedCertificate,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<TransactionEffects> {
        let _metrics_guard = if certificate.contains_shared_object() {
            self.metrics
                .execute_certificate_latency_shared_object
                .start_timer()
        } else {
            self.metrics
                .execute_certificate_latency_single_writer
                .start_timer()
        };
        trace!("wait_for_certificate_execution");

        self.metrics.total_cert_attempts.inc();

        if !certificate.contains_shared_object() {
            // Shared object transactions need to be sequenced by the consensus before
            // enqueueing for execution, done in
            // AuthorityPerEpochStore::handle_consensus_transaction(). For owned
            // object transactions, they can be enqueued for execution immediately.
            self.execution_scheduler.enqueue(
                vec![(
                    Schedulable::Transaction(VerifiedExecutableTransaction::new_from_certificate(
                        certificate.clone(),
                    )),
                    ExecutionEnv::new(),
                )],
                epoch_store,
            );
        }

        // tx could be reverted when epoch ends, so we must be careful not to return a
        // result here after the epoch ends.
        epoch_store
            .within_alive_epoch(self.notify_read_effects(
                "AuthorityState::wait_for_certificate_execution",
                certificate,
            ))
            .await
            .and_then(|r| r)
    }

    /// Internal logic to execute a transaction.
    ///
    /// Guarantees that
    /// - If input objects are available, return no permanent failure.
    /// - Execution and output commit are atomic. i.e. outputs are only written
    ///   to storage,
    /// on successful execution; crashed execution has no observable effect and
    /// can be retried.
    ///
    /// It is caller's responsibility to ensure input objects are available and
    /// locks are set. If this cannot be satisfied by the caller,
    /// `wait_for_certificate_execution()` should be called instead.
    ///
    /// Should only be called within iota-core.
    #[instrument(level = "trace", skip_all, fields(tx_digest = ?transaction.digest()))]
    pub fn try_execute_immediately(
        &self,
        transaction: &VerifiedExecutableTransaction,
        execution_env: ExecutionEnv,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<(TransactionEffects, Option<ExecutionError>)> {
        let _scope = monitored_scope("Execution::try_execute_immediately");
        let _metrics_guard = self.metrics.internal_execution_latency.start_timer();

        let tx_digest = transaction.digest();

        // Acquire a lock to prevent concurrent executions of the same transaction.
        let tx_guard = epoch_store.acquire_tx_guard(transaction)?;

        // The transaction could have been processed by a concurrent attempt of the
        // same transaction, so check if the effects have already been written.
        if let Some(effects) = self
            .get_transaction_cache_reader()
            .try_get_executed_effects(tx_digest)?
        {
            if let Some(expected_effects_digest_inner) = execution_env.expected_effects_digest {
                assert_eq!(
                    effects.digest(),
                    expected_effects_digest_inner,
                    "Unexpected effects digest for transaction {tx_digest}"
                );
            }
            tx_guard.release();
            return Ok((effects, None));
        }

        let (tx_input_objects, per_authenticator_inputs) = self.read_objects_for_execution(
            tx_guard.as_lock_guard(),
            transaction,
            execution_env.assigned_versions,
            epoch_store,
        )?;

        self.process_transaction(
            tx_guard,
            transaction,
            tx_input_objects,
            per_authenticator_inputs,
            execution_env.expected_effects_digest,
            epoch_store,
        )
        .tap_err(|e| info!(?tx_digest, "process_transaction failed: {e}"))
        .tap_ok(
            |(fx, _)| debug!(?tx_digest, fx_digest=?fx.digest(), "process_transaction succeeded"),
        )
    }

    pub fn read_objects_for_execution(
        &self,
        tx_lock: &TxLockGuard,
        transaction: &VerifiedExecutableTransaction,
        assigned_shared_object_versions: AssignedVersions,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<(InputObjects, Vec<(InputObjects, ObjectReadResult)>)> {
        let _scope = monitored_scope("Execution::load_input_objects");
        let _metrics_guard = self
            .metrics
            .execution_load_input_objects_latency
            .start_timer();

        let input_objects = transaction.collect_all_input_object_kind_for_reading()?;

        let input_objects = self.input_loader.read_objects_for_execution(
            &transaction.key(),
            tx_lock,
            &input_objects,
            &assigned_shared_object_versions,
            epoch_store.epoch(),
        )?;

        transaction.split_input_objects_into_groups_for_reading(input_objects)
    }

    /// Test only wrapper for `try_execute_immediately()` above, useful for
    /// checking errors if the pre-conditions are not satisfied, and
    /// executing change epoch transactions.
    pub fn try_execute_for_test(
        &self,
        certificate: &VerifiedCertificate,
        execution_env: ExecutionEnv,
    ) -> IotaResult<(VerifiedSignedTransactionEffects, Option<ExecutionError>)> {
        let epoch_store = self.epoch_store_for_testing();
        let (effects, execution_error_opt) = self.try_execute_immediately(
            &VerifiedExecutableTransaction::new_from_certificate(certificate.clone()),
            execution_env,
            &epoch_store,
        )?;
        let signed_effects = self.sign_effects(effects, &epoch_store)?;
        Ok((signed_effects, execution_error_opt))
    }

    /// Non-fallible version of `try_execute_for_test()`.
    pub fn execute_for_test(
        &self,
        certificate: &VerifiedCertificate,
        execution_env: ExecutionEnv,
    ) -> (VerifiedSignedTransactionEffects, Option<ExecutionError>) {
        self.try_execute_for_test(certificate, execution_env)
            .expect("try_execute_for_test should not fail")
    }

    pub async fn notify_read_effects(
        &self,
        task_name: &'static str,
        certificate: &VerifiedCertificate,
    ) -> IotaResult<TransactionEffects> {
        self.get_transaction_cache_reader()
            .try_notify_read_executed_effects(task_name, &[*certificate.digest()])
            .await
            .map(|mut r| r.pop().expect("must return correct number of effects"))
    }

    fn check_owned_locks(&self, owned_object_refs: &[ObjectReference]) -> IotaResult {
        self.get_object_cache_reader()
            .try_check_owned_objects_are_live(owned_object_refs)
    }

    /// This function captures the required state to debug a forked transaction.
    /// The dump is written to a file in dir `path`, with name prefixed by the
    /// transaction digest. NOTE: Since this info escapes the validator
    /// context, make sure not to leak any private info here
    pub(crate) fn debug_dump_transaction_state(
        &self,
        tx_digest: &TransactionDigest,
        effects: &TransactionEffects,
        expected_effects_digest: TransactionEffectsDigest,
        inner_temporary_store: &InnerTemporaryStore,
        transaction: &VerifiedExecutableTransaction,
        debug_dump_config: &StateDebugDumpConfig,
    ) -> IotaResult<PathBuf> {
        // Fall back to the OS temp directory if no dump directory is configured.
        // This is safe: dump files are named by transaction digest, so no collisions.
        let dump_dir = debug_dump_config
            .dump_file_directory
            .as_ref()
            .cloned()
            .unwrap_or(std::env::temp_dir());
        let epoch_store = self.load_epoch_store_one_call_per_task();

        NodeStateDump::new(
            tx_digest,
            effects,
            expected_effects_digest,
            self.get_object_store().as_ref(),
            &epoch_store,
            inner_temporary_store,
            transaction,
        )?
        .write_to_file(&dump_dir)
        .map_err(|e| IotaError::FileIO(e.to_string()))
    }

    #[instrument(name = "process_certificate", level = "trace", skip_all, fields(tx_digest = ?transaction.digest(), sender = ?transaction.data().transaction().gas_owner().to_string()))]
    pub(crate) fn process_transaction(
        &self,
        tx_guard: TxGuard,
        transaction: &VerifiedExecutableTransaction,
        tx_input_objects: InputObjects,
        per_authenticator_inputs: Vec<(InputObjects, ObjectReadResult)>,
        expected_effects_digest: Option<TransactionEffectsDigest>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<(TransactionEffects, Option<ExecutionError>)> {
        let process_transaction_start_time = tokio::time::Instant::now();
        let digest = *transaction.digest();

        let _scope = monitored_scope("Execution::process_certificate");

        fail_point_if!("correlated-crash-process-transaction", || {
            if iota_simulator::random::deterministic_probability_once(digest, 0.01) {
                iota_simulator::task::kill_current_node(None);
            }
        });

        let execution_guard = self.execution_lock_for_executable_transaction(transaction);
        // Any caller that verifies the signatures on the transaction will have already
        // checked the epoch. But paths that don't verify sigs (e.g. execution
        // from checkpoint, reading from db) present the possibility of an epoch
        // mismatch. If this transaction is not finalized in previous epoch, then it's
        // invalid.
        let execution_guard = match execution_guard {
            Ok(execution_guard) => execution_guard,
            Err(err) => {
                tx_guard.release();
                return Err(err);
            }
        };
        // Since we obtain a reference to the epoch store before taking the execution
        // lock, it's possible that reconfiguration has happened and they no
        // longer match.
        if *execution_guard != epoch_store.epoch() {
            tx_guard.release();
            info!("The epoch of the execution_guard doesn't match the epoch store");
            return Err(IotaError::WrongEpoch {
                expected_epoch: epoch_store.epoch(),
                actual_epoch: *execution_guard,
            });
        }

        // Errors originating from `execute_transaction` may be transient (failure to
        // read locks) or non-transient (transaction input is invalid, move vm
        // errors). However, all errors from this function occur before we have
        // written anything to the db, so we commit the tx guard and rely on the
        // client to retry the tx (if it was transient).
        let (inner_temporary_store, effects, execution_error_opt) = match self.execute_transaction(
            &execution_guard,
            transaction,
            tx_input_objects,
            per_authenticator_inputs,
            epoch_store,
        ) {
            Err(e) => {
                info!(name = ?self.name, ?digest, "Error preparing transaction: {e}");
                tx_guard.release();
                return Err(e);
            }
            Ok(res) => res,
        };

        if let Some(expected_effects_digest) = expected_effects_digest {
            if effects.digest() != expected_effects_digest {
                // We dont want to mask the original error, so we log it and continue.
                match self.debug_dump_transaction_state(
                    &digest,
                    &effects,
                    expected_effects_digest,
                    &inner_temporary_store,
                    transaction,
                    &self.config.state_debug_dump_config,
                ) {
                    Ok(out_path) => {
                        info!(
                            "Dumped node state for transaction {} to {}",
                            digest,
                            out_path.as_path().display().to_string()
                        );
                    }
                    Err(e) => {
                        error!("Error dumping state for transaction {}: {e}", digest);
                    }
                }
                error!(
                    tx_digest = ?digest,
                    ?expected_effects_digest,
                    actual_effects = ?effects,
                    "fork detected!"
                );
                panic!(
                    "Transaction {} is expected to have effects digest {}, but got {}!",
                    digest,
                    expected_effects_digest,
                    effects.digest(),
                );
            }
        }

        fail_point!("crash");

        self.commit_transaction(
            transaction,
            inner_temporary_store,
            &effects,
            tx_guard,
            execution_guard,
            expected_effects_digest,
            epoch_store,
        )?;

        let elapsed = process_transaction_start_time.elapsed().as_micros() as f64;
        if elapsed > 0.0 {
            self.metrics
                .execution_gas_latency_ratio
                .observe(effects.gas_cost_summary().computation_cost as f64 / elapsed);
        };
        Ok((effects, execution_error_opt))
    }

    pub async fn reconfigure_traffic_control(
        &self,
        params: TrafficControlReconfigParams,
    ) -> Result<TrafficControlReconfigParams, IotaError> {
        if let Some(traffic_controller) = self.traffic_controller.as_ref() {
            traffic_controller.admin_reconfigure(params)
        } else {
            Err(IotaError::InvalidAdminRequest(
                "Traffic controller is not configured on this node".to_string(),
            ))
        }
    }

    #[instrument(level = "trace", skip_all)]
    fn commit_transaction(
        &self,
        transaction: &VerifiedExecutableTransaction,
        inner_temporary_store: InnerTemporaryStore,
        effects: &TransactionEffects,
        tx_guard: TxGuard,
        _execution_guard: ExecutionLockReadGuard<'_>,
        expected_effects_digest: Option<TransactionEffectsDigest>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult {
        let _scope: Option<iota_metrics::MonitoredScopeGuard> =
            monitored_scope("Execution::commit_certificate");
        let _metrics_guard = self.metrics.commit_certificate_latency.start_timer();

        let tx_digest = transaction.digest();
        let input_object_count = inner_temporary_store.input_objects.len();
        let shared_object_count = effects.input_shared_objects().len();

        let output_keys = inner_temporary_store.get_output_keys(effects);

        // emit subscription notifications
        let _ = self
            .post_process_one_tx(transaction, effects, &inner_temporary_store, epoch_store)
            .tap_err(|e| {
                self.metrics.post_processing_total_failures.inc();
                error!(?tx_digest, "tx post processing failed: {e}");
            });

        // The insertion to epoch_store is not atomic with the insertion to the
        // perpetual store. This is OK because we insert to the epoch store
        // first. And during lookups we always look up in the perpetual store first.
        epoch_store.insert_executed_in_epoch(tx_digest);

        let key = transaction.key();
        if !matches!(key, TransactionKey::Digest(_)) {
            epoch_store.insert_tx_key(key, *tx_digest)?;
        }

        // Allow testing what happens if we crash here.
        fail_point!("crash");

        let transaction_outputs = TransactionOutputs::build_transaction_outputs(
            transaction.clone().into_unsigned(),
            effects.clone(),
            inner_temporary_store,
        );
        self.get_cache_writer()
            .try_write_transaction_outputs(epoch_store.epoch(), transaction_outputs.into())?;

        self.report_failed_deny_rule_update_execution(
            transaction,
            effects,
            expected_effects_digest,
            epoch_store,
        );

        if transaction.transaction().is_end_of_epoch_tx() {
            // At the end of epoch, since system packages may have been upgraded, force
            // reload them in the cache.
            self.get_object_cache_reader()
                .force_reload_system_packages(&BuiltInFramework::all_package_ids());
        }

        // `commit_transaction()` finished, the tx is fully committed to the store.
        tx_guard.commit_tx();

        match self.execution_scheduler.as_ref() {
            ExecutionSchedulerWrapper::ExecutionScheduler(_) => {}
            ExecutionSchedulerWrapper::TransactionManager(tm) => {
                // Notifies transaction manager about transaction and output objects committed.
                // This provides necessary information to transaction manager to start executing
                // additional ready transactions.
                tm.notify_commit(tx_digest, output_keys, epoch_store);
                // A transaction with a non-digest key can execute from a synced
                // checkpoint, in which case local randomness generation — the only
                // other caller of `notify_transaction_key` — never runs for that
                // round and would leave the env parked under its key forever. The
                // enqueue this triggers is filtered out as already executed.
                if let Some(key) = transaction.non_digest_key() {
                    tm.notify_transaction_key(epoch_store, key, *tx_digest);
                }
            }
        }

        self.update_metrics(transaction, input_object_count, shared_object_count);

        Ok(())
    }

    fn update_metrics(
        &self,
        transaction: &VerifiedExecutableTransaction,
        input_object_count: usize,
        shared_object_count: usize,
    ) {
        // count signature by scheme, for multisig
        if transaction.has_multisig() {
            self.metrics.multisig_sig_count.inc();
        }

        self.metrics.total_effects.inc();
        self.metrics.total_certs.inc();

        if shared_object_count > 0 {
            self.metrics.shared_obj_tx.inc();
        }

        if transaction.is_sponsored_tx() {
            self.metrics.sponsored_tx.inc();
        }

        self.metrics
            .num_input_objs
            .observe(input_object_count as f64);
        self.metrics
            .num_shared_objects
            .observe(shared_object_count as f64);
        self.metrics
            .batch_size
            .observe(transaction.data().transaction().kind().num_commands() as f64);
    }

    /// `execute_transaction()` validates the transaction input, and executes
    /// the transaction, returning effects, output objects, events, etc.
    ///
    /// It reads state from the db (both owned and shared locks), but it has no
    /// side effects.
    ///
    /// It can be generally understood that a failure of `execute_transaction`
    /// indicates a non-transient error, e.g. the transaction input is
    /// somehow invalid, the correct locks are not held, etc. However, this
    /// is not entirely true, as a transient db read error may also cause
    /// this function to fail.
    #[instrument(level = "trace", skip_all)]
    fn execute_transaction(
        &self,
        _execution_guard: &ExecutionLockReadGuard<'_>,
        transaction: &VerifiedExecutableTransaction,
        tx_input_objects: InputObjects,
        per_authenticator_inputs: Vec<(InputObjects, ObjectReadResult)>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<(
        InnerTemporaryStore,
        TransactionEffects,
        Option<ExecutionError>,
    )> {
        let _scope = monitored_scope("Execution::execute_certificate");
        let _metrics_guard = self.metrics.prepare_certificate_latency.start_timer();
        let prepare_transaction_start_time = tokio::time::Instant::now();

        let protocol_config = epoch_store.protocol_config();

        let reference_gas_price = epoch_store.reference_gas_price();

        let epoch_id = epoch_store.epoch_start_config().epoch_data().epoch_id();
        let epoch_start_timestamp = epoch_store
            .epoch_start_config()
            .epoch_data()
            .epoch_start_timestamp();

        let backing_store = self.get_backing_store().as_ref();

        let tx_digest = *transaction.digest();

        // TODO: We need to move this to a more appropriate place to avoid redundant
        // checks.
        let tx = transaction.data().transaction();
        tx.validity_check(protocol_config)?;

        let (kind, signer, gas_data) = tx.execution_parts();

        let move_authenticators = transaction.move_authenticators();

        #[cfg_attr(not(any(msim, fail_points)), expect(unused_mut))]
        let (inner_temp_store, _, mut effects, execution_error_opt) = if move_authenticators
            .is_empty()
        {
            // No Move authentication required, proceed to execute the transaction directly.

            // The cost of partially re-auditing a transaction before execution is
            // tolerated.
            let (tx_gas_status, tx_checked_input_objects) =
                iota_transaction_checks::check_certificate_input(
                    transaction,
                    tx_input_objects,
                    protocol_config,
                    reference_gas_price,
                )?;

            let owned_object_refs = tx_checked_input_objects.inner().filter_owned_objects();
            self.check_owned_locks(&owned_object_refs)?;
            epoch_store.executor().execute_transaction_to_effects(
                backing_store,
                protocol_config,
                self.metrics.limits_metrics.clone(),
                // TODO: would be nice to pass the whole NodeConfig here, but it creates a
                // cyclic dependency w/ iota-adapter
                self.config
                    .expensive_safety_check_config
                    .enable_deep_per_tx_iota_conservation_check(),
                self.config.certificate_deny_config.certificate_deny_set(),
                &epoch_id,
                epoch_start_timestamp,
                tx_checked_input_objects,
                gas_data,
                tx_gas_status,
                kind,
                signer,
                tx_digest,
                &mut None,
            )
        } else {
            // One or more `MoveAuthenticator` signatures present — authenticate each and
            // then execute the transaction.
            // It is supposed that `MoveAuthenticator` availability is checked in
            // `SenderSignedTransaction::validity_check`.

            debug_assert_eq!(
                move_authenticators.len(),
                per_authenticator_inputs.len(),
                "Move authenticators amount must match the number of authenticator inputs"
            );

            let per_authenticator_inputs = move_authenticators
                .iter()
                .zip(per_authenticator_inputs)
                .map(
                    |(move_authenticator, (authenticator_input_objects, account_object))| {
                        // Check basic `object_to_authenticate` preconditions and get its
                        // components.
                        let (
                            auth_account_object_id,
                            auth_account_object_seq_number,
                            auth_account_object_digest,
                        ) = move_authenticator
                            .object_to_authenticate_components()
                            .expect("the object to authenticate is validated before consensus and cannot be invalid during execution");

                        let signer = move_authenticator.address();

                        let authenticator_function_ref_for_execution = self
                            .check_move_account_for_execution(
                                auth_account_object_id,
                                auth_account_object_seq_number,
                                auth_account_object_digest,
                                account_object,
                                &signer,
                            );

                        (
                            authenticator_input_objects,
                            authenticator_function_ref_for_execution,
                        )
                    },
                )
                .collect::<Vec<_>>();

            let per_authenticator_input_objects = per_authenticator_inputs
                .iter()
                .map(|(authenticator_input_objects, _)| authenticator_input_objects.clone())
                .collect::<Vec<_>>();

            // Serialize the Transaction for the auth context.
            let tx_bytes = bcs::to_bytes(tx).expect("Transaction serialization cannot fail");

            let (sender_auth_digest, sponsor_auth_digest) =
                transaction.data().compute_auth_digests()?;

            // Check the `MoveAuthenticator` input objects.
            // The `MoveAuthenticator` receiving objects are checked on the signing step.
            // `max_auth_gas` is used here as a Move authenticator gas budget until it is
            // not a part of the transaction data.
            let authenticator_gas_budget = protocol_config.max_auth_gas();
            let (
                gas_status,
                per_authenticator_checked_input_objects,
                authenticator_and_tx_checked_input_objects,
            ) = iota_transaction_checks::check_certificate_and_move_authenticator_input(
                transaction,
                tx_input_objects,
                per_authenticator_input_objects,
                authenticator_gas_budget,
                protocol_config,
                reference_gas_price,
            )?;

            debug_assert_eq!(
                move_authenticators.len(),
                per_authenticator_checked_input_objects.len(),
                "Move authenticators amount must match the number of checked authenticator inputs"
            );

            let move_authenticators = move_authenticators
                .into_iter()
                .zip(per_authenticator_inputs)
                .zip(per_authenticator_checked_input_objects)
                .map(
                    |(
                        (move_authenticator, (_, authenticator_function_ref_for_execution)),
                        authenticator_checked_input_objects,
                    )| {
                        (
                            move_authenticator.to_owned(),
                            authenticator_function_ref_for_execution,
                            authenticator_checked_input_objects,
                        )
                    },
                )
                .collect::<Vec<_>>();

            let owned_object_refs = authenticator_and_tx_checked_input_objects
                .inner()
                .filter_owned_objects();
            self.check_owned_locks(&owned_object_refs)?;

            let (sender_authenticator_function_ref, sponsor_authenticator_function_ref) =
                extract_auth_fun_refs(signer, gas_data.owner, |address| {
                    move_authenticators
                        .iter()
                        .find(|t| t.0.address() == address)
                        .map(|t| t.1.authenticator_function_ref.clone())
                });

            let auth_context_data = AuthContextData {
                transaction_data_bytes: tx_bytes,
                sender_auth_digest,
                sponsor_auth_digest,
                sender_authenticator_function_ref,
                sponsor_authenticator_function_ref,
            };

            epoch_store
                .executor()
                .authenticate_then_execute_transaction_to_effects(
                    backing_store,
                    protocol_config,
                    self.metrics.limits_metrics.clone(),
                    self.config
                        .expensive_safety_check_config
                        .enable_deep_per_tx_iota_conservation_check(),
                    self.config.certificate_deny_config.certificate_deny_set(),
                    &epoch_id,
                    epoch_start_timestamp,
                    gas_data,
                    gas_status,
                    move_authenticators,
                    authenticator_and_tx_checked_input_objects,
                    kind,
                    signer,
                    tx_digest,
                    auth_context_data,
                    &mut None,
                )
        };

        fail_point_if!("cp_execution_nondeterminism", || {
            #[cfg(msim)]
            self.create_fail_state(transaction, epoch_store, &mut effects);
        });

        let elapsed = prepare_transaction_start_time.elapsed().as_micros() as f64;
        if elapsed > 0.0 {
            self.metrics
                .prepare_cert_gas_latency_ratio
                .observe(effects.gas_cost_summary().computation_cost as f64 / elapsed);
        }

        Ok((inner_temp_store, effects, execution_error_opt.err()))
    }

    pub fn prepare_transaction_for_benchmark(
        &self,
        transaction: &VerifiedExecutableTransaction,
        input_objects: InputObjects,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<(
        InnerTemporaryStore,
        TransactionEffects,
        Option<ExecutionError>,
    )> {
        let lock = RwLock::new(epoch_store.epoch());
        let execution_guard = lock.try_read().unwrap();

        self.execute_transaction(
            &execution_guard,
            transaction,
            input_objects,
            vec![],
            epoch_store,
        )
    }

    /// Simulate a transaction without committing it.
    ///
    /// `checks` selects the Move VM semantics: `VmChecks::Enabled` runs the
    /// transaction as it would run on chain (a dry run), while
    /// `VmChecks::Disabled` relaxes the checks around entry functions and
    /// argument values (a dev inspect). Both report the per-command return
    /// values in [`SimulateTransactionResult::execution_result`].
    ///
    /// Under either `checks`, the simulation fills in whatever gas the
    /// transaction leaves unset, so that a caller with no gas to declare can
    /// leave all of it out: no gas payment mints a mock gas coin, whose ID is
    /// reported back in [`SimulateTransactionResult::mock_gas_id`]; a zero gas
    /// price becomes the epoch's reference gas price; and a zero gas budget
    /// becomes as much as the gas coins can back, up to
    /// [`max_tx_gas`](iota_protocol_config::ProtocolConfig::max_tx_gas).
    /// Anything the transaction does declare is metered as given, so a dry run
    /// still rejects the gas a validator would.
    ///
    /// Whatever the budget resolves to, the gas coins have to cover it, since
    /// execution reserves the whole budget from them before running any command
    /// and refunds it afterwards. A caller leaving the budget at zero to have
    /// the cost estimated therefore gets an estimate whatever its coins hold,
    /// but the reserved budget is off limits for the duration of the
    /// programmable transaction: a transaction that also pays out of its gas
    /// coin has to declare a budget leaving room for that, exactly as it would
    /// on chain. A balance too small to declare the minimum budget at all is
    /// rejected with [`UserInputError::GasBalanceTooLow`].
    pub fn simulate_transaction(
        &self,
        transaction: Transaction,
        checks: VmChecks,
    ) -> IotaResult<SimulateTransactionResult> {
        let epoch_store = self.load_epoch_store_one_call_per_task();
        self.simulate_transaction_in_epoch(&epoch_store, transaction, checks)
    }

    /// Same as [`AuthorityState::simulate_transaction`], for callers that
    /// already hold an epoch store.
    ///
    /// Callers that derive gas parameters from an epoch, or resolve types
    /// against its executor once the simulation returns, should pass that same
    /// epoch store here so the whole operation observes one epoch.
    ///
    /// Nothing here checks that `epoch_store` is the current one — pinning a
    /// superseded epoch is the point, and is what
    /// [`AuthorityState::simulate_transaction`] does for the span of its own
    /// call. Keeping one across an unbounded period is the caller's problem:
    /// the simulation would run against that epoch's protocol config,
    /// executor, and reference gas price.
    #[instrument("simulate_tx", level = "trace", skip_all)]
    pub fn simulate_transaction_in_epoch(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        transaction: Transaction,
        checks: VmChecks,
    ) -> IotaResult<SimulateTransactionResult> {
        if !self.is_fullnode(epoch_store) {
            return Err(IotaError::UnsupportedFeature {
                error: "simulate is only supported on fullnodes".to_string(),
            });
        }

        self.simulate_transaction_inner(epoch_store, transaction, checks)
    }

    /// Same as [`AuthorityState::simulate_transaction`], but runs on a
    /// validator too. Only the single-node benchmark, which has no fullnode
    /// to run against, needs this.
    pub fn simulate_transaction_for_benchmark(
        &self,
        transaction: Transaction,
        checks: VmChecks,
    ) -> IotaResult<SimulateTransactionResult> {
        let epoch_store = self.load_epoch_store_one_call_per_task();
        self.simulate_transaction_inner(&epoch_store, transaction, checks)
    }

    #[instrument(level = "trace", skip_all)]
    fn simulate_transaction_inner(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        mut transaction: Transaction,
        checks: VmChecks,
    ) -> IotaResult<SimulateTransactionResult> {
        if transaction.kind().is_system() {
            return Err(IotaError::UnsupportedFeature {
                error: "simulate does not support system transactions".to_string(),
            });
        }

        // Cheap validity checks for a transaction, including input size limits.
        // This does not check if gas objects are missing since we may create a
        // mock gas object. It checks for other transaction input validity.
        transaction.validity_check_no_gas_check(epoch_store.protocol_config())?;

        // The full validity check caps the gas payment size alongside requiring a
        // gas payment at all, which a simulation relaxes so it can mock one. The cap
        // still applies, and is cheapest before any object is loaded.
        transaction.check_gas_payment_size(epoch_store.protocol_config())?;

        let input_object_kinds = transaction.input_objects()?;
        let receiving_object_refs = transaction.receiving_objects();

        // Since we need to simulate a validator signing the transaction, the first step
        // is to check if some transaction elements are denied.
        iota_transaction_checks::deny::check_transaction_for_validation(
            &transaction,
            &[],
            &input_object_kinds,
            &receiving_object_refs,
            &self.config.transaction_deny_config,
            self.get_backing_package_store().as_ref(),
        )?;

        // Load input and receiving objects
        let (mut input_objects, receiving_objects) = self.input_loader.read_objects_for_signing(
            // We don't want to cache this transaction since it's a simulation.
            None,
            &input_object_kinds,
            &receiving_object_refs,
            epoch_store.epoch(),
        )?;

        // Create a mock gas object if one was not provided
        let mock_gas_id = if transaction.gas().is_empty() {
            let mock_gas_object = mock_simulation_gas_coin(transaction.gas_data().owner);
            let mock_gas_object_ref = mock_gas_object.object_ref();
            transaction.gas_data_mut().objects = vec![mock_gas_object_ref];
            input_objects.push(ObjectReadResult::new_from_gas_object(&mock_gas_object));
            Some(mock_gas_object.id())
        } else {
            None
        };

        let protocol_config = epoch_store.protocol_config();

        iota_types::gas::fill_in_unset_simulation_gas(
            &mut transaction,
            &input_objects,
            epoch_store.reference_gas_price(),
            protocol_config,
        );

        // `MoveAuthenticator`s are not supported in simulation, so we set the
        // `authenticator_gas_budget` to 0.
        let authenticator_gas_budget = 0;

        // Checks enabled -> DRY-RUN, it means we are simulating a real TX
        // Checks disabled -> DEV-INSPECT, more relaxed Move VM checks
        let (gas_status, checked_input_objects) = if checks.enabled() {
            iota_transaction_checks::check_transaction_input(
                protocol_config,
                epoch_store.reference_gas_price(),
                &transaction,
                input_objects,
                &receiving_objects,
                &self.metrics.bytecode_verifier_metrics,
                &self.config.verifier_signing_config,
                authenticator_gas_budget,
            )?
        } else {
            // Execution smashes the gas coins and reserves the whole budget from them
            // before running any command, treating the input checks as having verified
            // that they are gas coins at all — so with those checks skipped here, this
            // has to stand in for them. With the checks enabled,
            // `check_transaction_input` covers it.
            iota_types::gas::check_gas_coins_cover_budget_in_simulation(
                &input_objects,
                transaction.gas(),
                transaction.gas_budget(),
            )?;

            let checked_input_objects = iota_transaction_checks::check_simulation_input(
                protocol_config,
                transaction.kind(),
                input_objects,
                receiving_objects,
            )?;
            let gas_status = IotaGasStatus::new(
                transaction.gas_budget(),
                transaction.gas_price(),
                epoch_store.reference_gas_price(),
                protocol_config,
            )?;

            (gas_status, checked_input_objects)
        };

        // Create a new executor for the simulation
        let executor = iota_execution::executor(
            protocol_config,
            true, // silent
            None,
        )
        .expect("Creating an executor should not fail here");

        // Execute the simulation
        let (kind, signer, gas_data) = transaction.execution_parts();
        let (inner_temp_store, _, effects, execution_result) = executor.dev_inspect_transaction(
            self.get_backing_store().as_ref(),
            protocol_config,
            self.metrics.limits_metrics.clone(),
            false, // expensive_checks
            self.config.certificate_deny_config.certificate_deny_set(),
            &epoch_store.epoch_start_config().epoch_data().epoch_id(),
            epoch_store
                .epoch_start_config()
                .epoch_data()
                .epoch_start_timestamp(),
            checked_input_objects,
            gas_data,
            gas_status,
            kind,
            signer,
            transaction.digest(),
            checks.disabled(),
        );

        let mut input_objects = inner_temp_store.input_objects;
        iota_types::storage::extend_input_objects_with_loaded_runtime_objects(
            &mut input_objects,
            &effects,
            &inner_temp_store.loaded_runtime_objects,
            self.get_backing_store().as_object_store(),
        );

        Ok(SimulateTransactionResult {
            input_objects,
            output_objects: inner_temp_store.written,
            events: effects.events_digest().map(|_| inner_temp_store.events),
            effects,
            execution_result,
            suggested_gas_price: self
                .congestion_tracker
                .get_prediction_suggested_gas_price(&transaction),
            mock_gas_id,
            gas_data: transaction.gas_data().clone(),
        })
    }

    // Only used for testing because of how epoch store is loaded.
    pub fn reference_gas_price_for_testing(&self) -> Result<u64, anyhow::Error> {
        let epoch_store = self.epoch_store_for_testing();
        Ok(epoch_store.reference_gas_price())
    }

    #[instrument(level = "trace", skip_all)]
    pub fn try_is_tx_already_executed(&self, digest: &TransactionDigest) -> IotaResult<bool> {
        self.get_transaction_cache_reader()
            .try_is_tx_already_executed(digest)
    }

    /// Non-fallible version of `try_is_tx_already_executed`.
    pub fn is_tx_already_executed(&self, digest: &TransactionDigest) -> bool {
        self.try_is_tx_already_executed(digest)
            .expect("storage access failed")
    }

    /// Builds and stages the JSON-RPC index update for an executed checkpoint.
    ///
    /// The staged batch is committed later, in checkpoint order, via
    /// [`IndexStore::commit_update_for_checkpoint`]. No-op when the JSON-RPC
    /// index is not configured.
    #[instrument(level = "debug", skip_all, fields(checkpoint = checkpoint.checkpoint_summary.sequence_number))]
    pub fn index_checkpoint_for_jsonrpc(
        &self,
        checkpoint: &CheckpointData,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult {
        let Some(indexes) = &self.indexes else {
            return Ok(());
        };

        let mut layout_resolver =
            epoch_store
                .executor()
                .type_layout_resolver(Box::new(PackageStoreWithFallback::new(
                    checkpoint,
                    self.get_backing_package_store(),
                )));

        indexes.index_checkpoint(
            checkpoint,
            self.get_object_store().as_ref(),
            layout_resolver.as_mut(),
            // Coin balances are only served by fullnodes, so validators skip
            // the coin index.
            !self.is_committee_validator(epoch_store),
        )
    }

    #[cfg(msim)]
    fn create_fail_state(
        &self,
        transaction: &VerifiedExecutableTransaction,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        effects: &mut TransactionEffects,
    ) {
        use std::cell::RefCell;

        use iota_types::effects::TransactionEffectsAPIForTesting;
        thread_local! {
            static FAIL_STATE: RefCell<(u64, HashSet<AuthorityName>)> = RefCell::new((0, HashSet::new()));
        }
        if !transaction.data().transaction().is_system_tx() {
            let committee = epoch_store.committee();
            let cur_stake = (**committee).weight(&self.name);
            if cur_stake > 0 {
                FAIL_STATE.with_borrow_mut(|fail_state| {
                    // let (&mut failing_stake, &mut failing_validators) = fail_state;
                    if fail_state.0 < committee.validity_threshold() {
                        fail_state.0 += cur_stake;
                        fail_state.1.insert(self.name);
                    }

                    if fail_state.1.contains(&self.name) {
                        info!("cp_exec failing tx");
                        effects.gas_cost_summary_mut_for_testing().computation_cost += 1;
                    }
                });
            }
        }
    }

    /// Emits transaction and event subscription notifications for an executed
    /// transaction. Index updates happen per checkpoint instead, via
    /// [`Self::index_checkpoint_for_jsonrpc`].
    #[instrument(level = "trace", skip_all, err)]
    fn post_process_one_tx(
        &self,
        transaction: &VerifiedExecutableTransaction,
        effects: &TransactionEffects,
        inner_temporary_store: &InnerTemporaryStore,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult {
        if self.indexes.is_none() {
            return Ok(());
        }

        let _scope = monitored_scope("Execution::post_process_one_tx");

        let tx_digest = transaction.digest();
        let timestamp_ms = Self::unixtime_now_ms();

        let effects: IotaTransactionBlockEffects = effects.clone().try_into()?;
        let events = self.make_transaction_block_events(
            inner_temporary_store.events.clone(),
            *tx_digest,
            timestamp_ms,
            epoch_store,
            inner_temporary_store,
        )?;
        // Emit events
        self.subscription_handler
            .process_tx(transaction.data().transaction(), &effects, &events)
            .tap_ok(|_| {
                self.metrics
                    .post_processing_total_tx_had_event_processed
                    .inc()
            })
            .tap_err(|e| {
                warn!(
                    ?tx_digest,
                    "Post processing - Couldn't process events for tx: {}", e
                )
            })?;

        self.metrics
            .post_processing_total_events_emitted
            .inc_by(events.data.len() as u64);

        Ok(())
    }

    fn make_transaction_block_events(
        &self,
        transaction_events: TransactionEvents,
        digest: TransactionDigest,
        timestamp_ms: u64,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        inner_temporary_store: &InnerTemporaryStore,
    ) -> IotaResult<IotaTransactionBlockEvents> {
        let mut layout_resolver =
            epoch_store
                .executor()
                .type_layout_resolver(Box::new(PackageStoreWithFallback::new(
                    inner_temporary_store,
                    self.get_backing_package_store(),
                )));
        IotaTransactionBlockEvents::try_from(
            transaction_events,
            digest,
            Some(timestamp_ms),
            layout_resolver.as_mut(),
        )
    }

    pub fn unixtime_now_ms() -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_millis();
        u64::try_from(now).expect("Travelling in time machine")
    }

    #[instrument(level = "trace", skip_all)]
    pub async fn handle_transaction_info_request(
        &self,
        request: TransactionInfoRequest,
    ) -> IotaResult<TransactionInfoResponse> {
        let epoch_store = self.load_epoch_store_one_call_per_task();
        let (transaction, status) = self
            .get_transaction_status(&request.transaction_digest, &epoch_store)?
            .ok_or(IotaError::TransactionNotFound {
                digest: request.transaction_digest,
            })?;
        Ok(TransactionInfoResponse {
            transaction,
            status,
        })
    }

    #[instrument(level = "trace", skip_all)]
    pub async fn handle_object_info_request(
        &self,
        request: ObjectInfoRequest,
    ) -> IotaResult<ObjectInfoResponse> {
        let epoch_store = self.load_epoch_store_one_call_per_task();

        let requested_object_seq = match request.request_kind {
            ObjectInfoRequestKind::LatestObjectInfo => {
                self.try_get_object_or_tombstone(request.object_id)?
                    .ok_or_else(|| {
                        IotaError::from(UserInputError::ObjectNotFound {
                            object_id: request.object_id,
                            version: None,
                        })
                    })?
                    .version
            }
            ObjectInfoRequestKind::PastObjectInfoDebug(seq) => seq,
        };

        let object = self
            .get_object_store()
            .try_get_object_by_key(&request.object_id, requested_object_seq)?
            .ok_or_else(|| {
                IotaError::from(UserInputError::ObjectNotFound {
                    object_id: request.object_id,
                    version: Some(requested_object_seq),
                })
            })?;

        let layout = if let (LayoutGenerationOption::Generate, Some(move_obj)) =
            (request.generate_layout, object.data.as_opt_struct())
        {
            Some(into_struct_layout(
                epoch_store
                    .executor()
                    .type_layout_resolver(Box::new(self.get_backing_package_store().as_ref()))
                    .get_annotated_layout(move_obj.struct_tag())?,
            )?)
        } else {
            None
        };

        let lock = if !object.is_address_owned() {
            // Only address owned objects have locks.
            None
        } else {
            self.get_transaction_lock(&object.object_ref(), &epoch_store)?
                .map(|s| s.into_inner())
        };

        Ok(ObjectInfoResponse {
            object,
            layout,
            lock_for_debugging: lock,
        })
    }

    #[instrument(level = "trace", skip_all)]
    pub fn handle_checkpoint_request(
        &self,
        request: &CheckpointRequest,
    ) -> IotaResult<CheckpointResponse> {
        let summary = if request.certified {
            let summary = match request.sequence_number {
                Some(seq) => self
                    .checkpoint_store
                    .get_checkpoint_by_sequence_number(seq)?,
                None => self.checkpoint_store.get_latest_certified_checkpoint()?,
            }
            .map(|v| v.into_inner());
            summary.map(CheckpointSummaryResponse::Certified)
        } else {
            let summary = match request.sequence_number {
                Some(seq) => self.checkpoint_store.get_locally_computed_checkpoint(seq)?,
                None => self
                    .checkpoint_store
                    .get_latest_locally_computed_checkpoint()?,
            };
            summary.map(CheckpointSummaryResponse::Pending)
        };
        let contents = match &summary {
            Some(s) => self
                .checkpoint_store
                .get_checkpoint_contents(&s.contents_digest())?,
            None => None,
        };
        Ok(CheckpointResponse {
            checkpoint: summary,
            contents,
        })
    }

    fn check_protocol_version(
        supported_protocol_versions: SupportedProtocolVersions,
        current_version: ProtocolVersion,
    ) {
        info!("current protocol version is now {:?}", current_version);
        info!("supported versions are: {:?}", supported_protocol_versions);
        if !supported_protocol_versions.is_version_supported(current_version) {
            let msg = format!(
                "Unsupported protocol version. The network is at {current_version:?}, but this IotaNode only supports: {supported_protocol_versions:?}. Shutting down.",
            );

            error!("{}", msg);
            eprintln!("{msg}");

            #[cfg(not(msim))]
            std::process::exit(1);

            #[cfg(msim)]
            iota_simulator::task::shutdown_current_node();
        }
    }

    #[expect(clippy::disallowed_methods)] // allow unbounded_channel()
    pub async fn new(
        name: AuthorityName,
        secret: StableSyncAuthoritySigner,
        supported_protocol_versions: SupportedProtocolVersions,
        store: Arc<AuthorityStore>,
        execution_cache_trait_pointers: ExecutionCacheTraitPointers,
        epoch_store: Arc<AuthorityPerEpochStore>,
        committee_store: Arc<CommitteeStore>,
        indexes: Option<Arc<IndexStore>>,
        grpc_indexes_store: Option<Arc<GrpcIndexesStore>>,
        checkpoint_store: Arc<CheckpointStore>,
        prometheus_registry: &Registry,
        config: NodeConfig,
        validator_tx_finalizer: Option<Arc<ValidatorTxFinalizer<NetworkAuthorityClient>>>,
        chain_identifier: ChainIdentifier,
        pruner_db: Option<Arc<AuthorityPrunerTables>>,
        checkpoint_progress_tracker: Option<Arc<CheckpointProgressTracker>>,
        policy_config: Option<PolicyConfig>,
        firewall_config: Option<RemoteFirewallConfig>,
    ) -> Arc<Self> {
        Self::check_protocol_version(supported_protocol_versions, epoch_store.protocol_version());

        let metrics = Arc::new(AuthorityMetrics::new(prometheus_registry));
        let (tx_ready_transactions, rx_ready_transactions) = unbounded_channel();
        let execution_scheduler = Arc::new(ExecutionSchedulerWrapper::new(
            execution_cache_trait_pointers.object_cache_reader.clone(),
            execution_cache_trait_pointers
                .transaction_cache_reader
                .clone(),
            tx_ready_transactions,
            &epoch_store,
            metrics.clone(),
        ));
        let (tx_execution_shutdown, rx_execution_shutdown) = oneshot::channel();

        let authority_per_epoch_pruner = AuthorityPerEpochStorePruner::new(
            epoch_store.get_parent_path(),
            config
                .authority_store_pruning_config
                .num_latest_epoch_dbs_to_retain,
        )
        .await;
        let pruner = AuthorityStorePruner::new(
            store.perpetual_tables.clone(),
            checkpoint_store.clone(),
            grpc_indexes_store.clone(),
            indexes.clone(),
            config.authority_store_pruning_config.clone(),
            epoch_store.committee().authority_exists(&name),
            epoch_store.epoch_start_state().epoch_duration_ms(),
            prometheus_registry,
            pruner_db,
            checkpoint_progress_tracker.clone(),
        );
        let input_loader =
            TransactionInputLoader::new(execution_cache_trait_pointers.object_cache_reader.clone());
        let epoch = epoch_store.epoch();
        let rgp = epoch_store.reference_gas_price();
        let traffic_controller_metrics =
            Arc::new(TrafficControllerMetrics::new(prometheus_registry));
        let traffic_controller = policy_config.map(|policy_config| {
            Arc::new(TrafficController::init(
                policy_config,
                traffic_controller_metrics,
                firewall_config.clone(),
            ))
        });
        let state = Arc::new(AuthorityState {
            name,
            secret,
            execution_lock: RwLock::new(epoch),
            epoch_store: ArcSwap::new(epoch_store.clone()),
            input_loader,
            execution_cache_trait_pointers,
            indexes,
            grpc_indexes_store,
            subscription_handler: Arc::new(SubscriptionHandler::new(prometheus_registry)),
            checkpoint_store,
            committee_store,
            execution_scheduler,
            tx_execution_shutdown: Mutex::new(Some(tx_execution_shutdown)),
            metrics,
            pruner,
            authority_per_epoch_pruner,
            checkpoint_progress_tracker,
            config,
            overload_info: AuthorityOverloadInfo::default(),
            validator_tx_finalizer,
            chain_identifier,
            congestion_tracker: Arc::new(CongestionTracker::new(rgp)),
            traffic_controller,
        });

        // Start a task to execute ready transactions.
        let authority_state = Arc::downgrade(&state);
        spawn_monitored_task!(execution_process(
            authority_state,
            rx_ready_transactions,
            rx_execution_shutdown,
        ));

        state
    }

    pub fn epoch_db_pruner(&self) -> &AuthorityPerEpochStorePruner {
        &self.authority_per_epoch_pruner
    }

    // TODO: Consolidate our traits to reduce the number of methods here.
    pub fn get_object_cache_reader(&self) -> &Arc<dyn ObjectCacheRead> {
        &self.execution_cache_trait_pointers.object_cache_reader
    }

    pub fn get_transaction_cache_reader(&self) -> &Arc<dyn TransactionCacheRead> {
        &self.execution_cache_trait_pointers.transaction_cache_reader
    }

    pub fn get_cache_writer(&self) -> &Arc<dyn ExecutionCacheWrite> {
        &self.execution_cache_trait_pointers.cache_writer
    }

    pub fn get_backing_store(&self) -> &Arc<dyn BackingStore + Send + Sync> {
        &self.execution_cache_trait_pointers.backing_store
    }

    pub fn get_backing_package_store(&self) -> &Arc<dyn BackingPackageStore + Send + Sync> {
        &self.execution_cache_trait_pointers.backing_package_store
    }

    pub fn get_object_store(&self) -> &Arc<dyn ObjectStore + Send + Sync> {
        &self.execution_cache_trait_pointers.object_store
    }

    pub fn get_reconfig_api(&self) -> &Arc<dyn ExecutionCacheReconfigAPI> {
        &self.execution_cache_trait_pointers.reconfig_api
    }

    pub fn get_global_state_hash_store(&self) -> &Arc<dyn GlobalStateHashStore> {
        &self.execution_cache_trait_pointers.global_state_hash_store
    }

    pub fn get_checkpoint_cache(&self) -> &Arc<dyn CheckpointCache> {
        &self.execution_cache_trait_pointers.checkpoint_cache
    }

    pub fn get_state_sync_store(&self) -> &Arc<dyn StateSyncAPI> {
        &self.execution_cache_trait_pointers.state_sync_store
    }

    pub fn get_cache_commit(&self) -> &Arc<dyn ExecutionCacheCommit> {
        &self.execution_cache_trait_pointers.cache_commit
    }

    pub fn database_for_testing(&self) -> Arc<AuthorityStore> {
        self.execution_cache_trait_pointers
            .testing_api
            .database_for_testing()
    }

    pub async fn prune_checkpoints_for_eligible_epochs_for_testing(
        &self,
        config: NodeConfig,
        metrics: Arc<AuthorityStorePruningMetrics>,
    ) -> anyhow::Result<()> {
        AuthorityStorePruner::prune_checkpoints_for_eligible_epochs(
            &self.database_for_testing().perpetual_tables,
            &self.checkpoint_store,
            self.grpc_indexes_store.as_deref(),
            None,
            config.authority_store_pruning_config,
            metrics,
            EPOCH_DURATION_MS_FOR_TESTING,
            self.checkpoint_progress_tracker.as_ref(),
        )
        .await
    }

    pub fn execution_scheduler(&self) -> &Arc<ExecutionSchedulerWrapper> {
        &self.execution_scheduler
    }

    /// Whether this authority runs the `ExecutionScheduler` rather than the
    /// `TransactionManager`.
    pub fn uses_execution_scheduler(&self) -> bool {
        self.execution_scheduler.uses_execution_scheduler()
    }

    /// Attempts to acquire execution lock for an executable transaction.
    /// Returns the lock if the transaction is matching current executed epoch
    /// Returns None otherwise
    pub fn execution_lock_for_executable_transaction(
        &self,
        transaction: &VerifiedExecutableTransaction,
    ) -> IotaResult<ExecutionLockReadGuard<'_>> {
        let lock = self
            .execution_lock
            .try_read()
            .map_err(|_| IotaError::ValidatorHaltedAtEpochEnd)?;
        if *lock == transaction.auth_sig().epoch() {
            Ok(lock)
        } else {
            Err(IotaError::WrongEpoch {
                expected_epoch: *lock,
                actual_epoch: transaction.auth_sig().epoch(),
            })
        }
    }

    /// Acquires the execution lock for the duration of a transaction signing
    /// request. This prevents reconfiguration from starting until we are
    /// finished handling the signing request. Otherwise, in-memory lock
    /// state could be cleared (by `ObjectLocks::clear_cached_locks`)
    /// while we are attempting to acquire locks for the transaction.
    pub fn execution_lock_for_signing(&self) -> IotaResult<ExecutionLockReadGuard<'_>> {
        self.execution_lock
            .try_read()
            .map_err(|_| IotaError::ValidatorHaltedAtEpochEnd)
    }

    pub async fn execution_lock_for_reconfiguration(&self) -> ExecutionLockWriteGuard<'_> {
        self.execution_lock.write().await
    }

    /// Reports a mirror that diverged from the object at the epoch boundary,
    /// where the two must agree. Reporting is the remedy: reconfiguration
    /// re-seeds the mirror from the object, so failing here would only pin
    /// the node to the diverged state. A missing object is fatal instead.
    /// Objects cannot be deleted, so the local store lost it and there is
    /// nothing to re-seed from. Nodes outside the closing committee are
    /// exempt. So is an epoch this node's consensus did not close. A
    /// checkpoint catch-up leaves the mirror legitimately behind until the
    /// re-seed.
    pub(crate) fn check_transaction_deny_rules_consistency(
        &self,
        cur_epoch_store: &AuthorityPerEpochStore,
        epoch_start_configuration: &EpochStartConfiguration,
    ) {
        if self.is_fullnode(cur_epoch_store) {
            return;
        }
        let Some(walked_deny_rules) = epoch_start_configuration.transaction_deny_rules_state()
        else {
            if cur_epoch_store
                .epoch_start_config()
                .transaction_deny_rules_obj_initial_shared_version()
                .is_some()
            {
                fatal!(
                    "TransactionDenyRules object existed in epoch {} but is missing from the \
                     state walked for the next epoch — the local store is corrupted; restore or \
                     state-sync before rejoining",
                    cur_epoch_store.epoch(),
                );
            }
            return;
        };
        // RejectAllTx proves this node's consensus processed every commit of
        // the epoch, so the mirror is complete. Otherwise the tail came from
        // synced checkpoints and the mirror's lag carries no signal.
        if cur_epoch_store
            .get_reconfig_state_read_lock_guard()
            .should_accept_tx()
        {
            info!(
                "skipping the deny-rule mirror comparison: consensus did not close epoch {} on \
                 this node",
                cur_epoch_store.epoch(),
            );
            return;
        }
        let mirrored_deny_rules = cur_epoch_store.get_mirrored_transaction_deny_rules();
        if *walked_deny_rules != *mirrored_deny_rules {
            debug_fatal!(
                "TransactionDenyRules object diverged from the mirrored state at the end of \
                 epoch {}; continuing from the object (walked: {walked_deny_rules:?}, mirrored: \
                 {mirrored_deny_rules:?})",
                cur_epoch_store.epoch(),
            );
            cur_epoch_store.metrics.deny_rule_mirror_divergence.set(1);
        }
    }

    /// Reports a `TransactionDenyRulesUpdate` whose execution failed — an
    /// invariant violation, the update is built to exclude every expected
    /// failure. The object misses the delta until the epoch boundary re-seeds
    /// the mirror. Identification is by kind, so the report needs no tracking
    /// state and holds across restarts and replays.
    ///
    /// `expected_effects_digest` is `Some` when these effects were handed to
    /// this node with the transaction, which is the case while executing a
    /// certified checkpoint: the failure is then part of agreed history, so it
    /// is reported without asserting. Effects the node derived itself assert,
    /// because only then is the broken invariant its own.
    pub(crate) fn report_failed_deny_rule_update_execution(
        &self,
        transaction: &VerifiedExecutableTransaction,
        effects: &TransactionEffects,
        expected_effects_digest: Option<TransactionEffectsDigest>,
        epoch_store: &AuthorityPerEpochStore,
    ) {
        if !matches!(
            transaction.transaction().kind(),
            TransactionKind::TransactionDenyRulesUpdate(_)
        ) || effects.status().is_success()
        {
            return;
        }
        epoch_store
            .metrics
            .deny_rule_update_execution_failures
            .inc();
        if expected_effects_digest.is_some() {
            error!(
                digest = ?transaction.digest(),
                status = ?effects.status(),
                "TransactionDenyRulesUpdate failed execution; the object misses its delta until \
                 the epoch boundary re-seeds the mirror"
            );
            return;
        }
        debug_fatal!(
            "TransactionDenyRulesUpdate failed execution; the object misses its delta until the \
             epoch boundary re-seeds the mirror (digest: {:?}, status: {:?})",
            transaction.digest(),
            effects.status(),
        );
    }

    #[instrument(level = "error", skip_all)]
    pub async fn reconfigure(
        &self,
        cur_epoch_store: &AuthorityPerEpochStore,
        supported_protocol_versions: SupportedProtocolVersions,
        new_committee: Committee,
        epoch_start_configuration: EpochStartConfiguration,
        state_hasher: Arc<GlobalStateHasher>,
        expensive_safety_check_config: &ExpensiveSafetyCheckConfig,
        epoch_supply_change: i64,
        epoch_last_checkpoint: CheckpointSequenceNumber,
    ) -> IotaResult<Arc<AuthorityPerEpochStore>> {
        Self::check_protocol_version(
            supported_protocol_versions,
            epoch_start_configuration
                .epoch_start_state()
                .protocol_version(),
        );
        self.metrics.reset_on_reconfigure();
        self.committee_store.insert_new_committee(&new_committee)?;

        // Wait until no transactions are being executed.
        let mut execution_lock = self.execution_lock_for_reconfiguration().await;

        // Terminate all epoch-specific tasks (those started with within_alive_epoch).
        cur_epoch_store.epoch_terminated().await;

        let highest_locally_built_checkpoint_seq = self
            .checkpoint_store
            .get_latest_locally_computed_checkpoint()?
            .map(|c| c.sequence_number())
            .unwrap_or(0);

        assert!(
            epoch_last_checkpoint >= highest_locally_built_checkpoint_seq,
            "expected {epoch_last_checkpoint} >= {highest_locally_built_checkpoint_seq}"
        );

        // Safe to reconfigure now. No transactions are being executed,
        // and no epoch-specific tasks are running.

        // TODO: revert_uncommitted_epoch_transactions will soon be unnecessary -
        // clear_state_end_of_epoch() can simply drop all uncommitted transactions
        self.revert_uncommitted_epoch_transactions(cur_epoch_store)
            .await?;
        self.get_reconfig_api()
            .clear_state_end_of_epoch(&execution_lock);
        self.check_system_consistency(
            cur_epoch_store,
            state_hasher,
            expensive_safety_check_config,
            epoch_supply_change,
        )?;
        self.check_transaction_deny_rules_consistency(cur_epoch_store, &epoch_start_configuration);

        self.get_reconfig_api()
            .try_set_epoch_start_configuration(&epoch_start_configuration)?;
        // When state snapshots are published, a RocksDB checkpoint of the
        // perpetual store taken at epoch end serves as the snapshot creation
        // input.
        if self
            .config
            .state_snapshot_write_config
            .object_store_config
            .is_some()
        {
            let current_epoch = cur_epoch_store.epoch();
            let epoch_checkpoint_path = self
                .config
                .db_checkpoint_path()
                .join(format!("epoch_{current_epoch}"));
            self.checkpoint_perpetual_db(&epoch_checkpoint_path, cur_epoch_store)?;
        }

        let new_epoch = new_committee.epoch;
        let new_epoch_store = self
            .reopen_epoch_db(
                cur_epoch_store,
                new_committee,
                epoch_start_configuration,
                expensive_safety_check_config,
                epoch_last_checkpoint,
            )
            .await?;
        assert_eq!(new_epoch_store.epoch(), new_epoch);
        match self.execution_scheduler.as_ref() {
            ExecutionSchedulerWrapper::ExecutionScheduler(_) => {}
            ExecutionSchedulerWrapper::TransactionManager(tm) => {
                tm.reconfigure(new_epoch);
            }
        }
        *execution_lock = new_epoch;
        // drop execution_lock after epoch store was updated
        // see also assert in AuthorityState::process_transaction
        // on the epoch store and execution lock epoch match
        Ok(new_epoch_store)
    }

    /// Advance the epoch store to the next epoch for testing only.
    /// This only manually sets all the places where we have the epoch number.
    /// It doesn't properly reconfigure the node, hence should be only used for
    /// testing.
    pub async fn reconfigure_for_testing(&self) {
        self.reconfigure_for_testing_impl(None).await;
    }

    /// Like [`Self::reconfigure_for_testing`], but the next epoch uses the
    /// given protocol config.
    pub async fn reconfigure_for_testing_with_protocol_config(
        &self,
        protocol_config: ProtocolConfig,
    ) {
        self.reconfigure_for_testing_impl(Some(protocol_config))
            .await;
    }

    async fn reconfigure_for_testing_impl(&self, protocol_config: Option<ProtocolConfig>) {
        let mut execution_lock = self.execution_lock_for_reconfiguration().await;
        let epoch_store = self.epoch_store_for_testing().clone();
        // Default to the epoch store's config, whose override guard may have
        // been dropped. Read it under the lock so config and epoch store are
        // one snapshot.
        let protocol_config =
            protocol_config.unwrap_or_else(|| epoch_store.protocol_config().clone());
        let _guard =
            ProtocolConfig::apply_overrides_for_testing(move |_, _| protocol_config.clone());
        let new_epoch_store = epoch_store.new_at_next_epoch_for_testing(
            self.get_backing_package_store().clone(),
            &self.config.expensive_safety_check_config,
            self.checkpoint_store
                .get_epoch_last_checkpoint(epoch_store.epoch())
                .unwrap()
                .map(|c| c.sequence_number())
                .unwrap_or_default(),
        );
        let new_epoch = new_epoch_store.epoch();
        match self.execution_scheduler.as_ref() {
            ExecutionSchedulerWrapper::ExecutionScheduler(_) => {}
            ExecutionSchedulerWrapper::TransactionManager(tm) => {
                tm.reconfigure(new_epoch);
            }
        }
        self.epoch_store.store(new_epoch_store);
        epoch_store.epoch_terminated().await;
        *execution_lock = new_epoch;
    }

    #[instrument(level = "error", skip_all)]
    fn check_system_consistency(
        &self,
        cur_epoch_store: &AuthorityPerEpochStore,
        state_hasher: Arc<GlobalStateHasher>,
        expensive_safety_check_config: &ExpensiveSafetyCheckConfig,
        epoch_supply_change: i64,
    ) -> IotaResult<()> {
        info!(
            "Performing iota conservation consistency check for epoch {}",
            cur_epoch_store.epoch()
        );

        if cfg!(debug_assertions) {
            cur_epoch_store.check_all_executed_transactions_in_checkpoint();
        }

        self.get_reconfig_api()
            .try_expensive_check_iota_conservation(cur_epoch_store, Some(epoch_supply_change))?;

        // check for root state hash consistency with live object set
        if expensive_safety_check_config.enable_state_consistency_check() {
            info!(
                "Performing state consistency check for epoch {}",
                cur_epoch_store.epoch()
            );
            self.expensive_check_is_consistent_state(state_hasher, cur_epoch_store);
        }

        if expensive_safety_check_config.enable_secondary_index_checks() {
            if let Some(indexes) = self.indexes.clone() {
                verify_indexes(self.get_global_state_hash_store().as_ref(), indexes)
                    .expect("secondary indexes are inconsistent");
            }
        }

        Ok(())
    }

    fn expensive_check_is_consistent_state(
        &self,
        state_hasher: Arc<GlobalStateHasher>,
        cur_epoch_store: &AuthorityPerEpochStore,
    ) {
        let live_object_set_hash = state_hasher.digest_live_object_set();

        let root_state_hash: ECMHLiveObjectSetDigest = self
            .get_global_state_hash_store()
            .get_root_state_hash_for_epoch(cur_epoch_store.epoch())
            .expect("Retrieving root state hash cannot fail")
            .expect("Root state hash for epoch must exist")
            .1
            .digest()
            .into();

        let is_inconsistent = root_state_hash != live_object_set_hash;
        if is_inconsistent {
            debug_fatal!(
                "Inconsistent state detected: root state hash: {:?}, live object set hash: {:?}",
                root_state_hash,
                live_object_set_hash
            );
        } else {
            info!("State consistency check passed");
        }

        state_hasher.set_inconsistent_state(is_inconsistent);
    }

    pub fn current_epoch_for_testing(&self) -> EpochId {
        self.epoch_store_for_testing().epoch()
    }

    /// Takes a RocksDB checkpoint of the perpetual store under
    /// `<checkpoint_path>/store/perpetual`, the layout the state snapshot
    /// uploader reads.
    #[instrument(level = "error", skip_all)]
    fn checkpoint_perpetual_db(
        &self,
        checkpoint_path: &Path,
        cur_epoch_store: &AuthorityPerEpochStore,
    ) -> IotaResult {
        let _metrics_guard = self.metrics.db_checkpoint_latency.start_timer();
        let current_epoch = cur_epoch_store.epoch();

        if checkpoint_path.exists() {
            info!("Skipping db checkpoint as it already exists for epoch: {current_epoch}");
            return Ok(());
        }

        let checkpoint_path_tmp = checkpoint_path.with_extension("tmp");
        let store_checkpoint_path_tmp = checkpoint_path_tmp.join("store");

        if checkpoint_path_tmp.exists() {
            fs::remove_dir_all(&checkpoint_path_tmp)
                .map_err(|e| IotaError::FileIO(e.to_string()))?;
        }

        fs::create_dir_all(&checkpoint_path_tmp).map_err(|e| IotaError::FileIO(e.to_string()))?;
        fs::create_dir(&store_checkpoint_path_tmp).map_err(|e| IotaError::FileIO(e.to_string()))?;

        self.get_reconfig_api()
            .try_checkpoint_db(&store_checkpoint_path_tmp.join("perpetual"))?;

        fs::rename(checkpoint_path_tmp, checkpoint_path)
            .map_err(|e| IotaError::FileIO(e.to_string()))?;
        Ok(())
    }

    /// Load the current epoch store. This can change during reconfiguration. To
    /// ensure that we never end up accessing different epoch stores in a
    /// single task, we need to make sure that this is called once per task.
    /// Each call needs to be carefully audited to ensure it is
    /// the case. This also means we should minimize the number of call-sites.
    /// Only call it when there is no way to obtain it from somewhere else.
    pub fn load_epoch_store_one_call_per_task(&self) -> Guard<Arc<AuthorityPerEpochStore>> {
        self.epoch_store.load()
    }

    // Load the epoch store, should be used in tests only.
    pub fn epoch_store_for_testing(&self) -> Guard<Arc<AuthorityPerEpochStore>> {
        self.load_epoch_store_one_call_per_task()
    }

    pub fn clone_committee_for_testing(&self) -> Committee {
        Committee::clone(self.epoch_store_for_testing().committee())
    }

    #[instrument(level = "trace", skip_all)]
    pub fn try_get_object(&self, object_id: &ObjectId) -> IotaResult<Option<Object>> {
        self.get_object_store()
            .try_get_object(object_id)
            .map_err(Into::into)
    }

    /// Non-fallible version of `try_get_object`.
    pub fn get_object(&self, object_id: &ObjectId) -> Option<Object> {
        self.try_get_object(object_id)
            .expect("storage access failed")
    }

    pub fn get_iota_system_package_object_ref(&self) -> IotaResult<ObjectReference> {
        Ok(self
            .try_get_object(&ObjectId::SYSTEM)?
            .expect("system package should always exist")
            .object_ref())
    }

    // This function is only used for testing.
    pub fn get_iota_system_state_object_for_testing(&self) -> IotaResult<IotaSystemState> {
        self.get_object_cache_reader()
            .try_get_iota_system_state_object_unsafe()
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> IotaResult<Option<VerifiedCheckpoint>> {
        Ok(self
            .checkpoint_store
            .get_checkpoint_by_sequence_number(sequence_number)?)
    }

    /// Wait for the given transactions to be included in a checkpoint.
    ///
    /// Returns a mapping from transaction digest to
    /// `(checkpoint_sequence_number, checkpoint_timestamp_ms)`.
    /// On timeout, returns partial results for any transactions that were
    /// already checkpointed.
    ///
    /// The wait survives epoch boundaries: a transaction in flight at a
    /// boundary may only be checkpointed in the next epoch, and still resolves
    /// here under the original deadline.
    pub async fn wait_for_checkpoint_inclusion(
        &self,
        digests: &[TransactionDigest],
        timeout: Duration,
    ) -> IotaResult<BTreeMap<TransactionDigest, (CheckpointSequenceNumber, u64)>> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut checkpoint_timestamp_cache = HashMap::<CheckpointSequenceNumber, u64>::new();
        let mut results = BTreeMap::new();
        let mut remaining = digests.to_vec();
        let mut epoch_store = self.load_epoch_store_one_call_per_task().clone();

        loop {
            let wait = epoch_store.wait_for_transactions_in_checkpoint_with_timeout(
                &remaining,
                deadline.saturating_duration_since(tokio::time::Instant::now()),
                |seq| self.checkpoint_timestamp_ms_cached(seq, &mut checkpoint_timestamp_cache),
            );
            tokio::select! {
                wait_results = wait => {
                    for (digest, seq_and_ts) in remaining.iter().zip(wait_results?) {
                        if let Some(seq_and_ts) = seq_and_ts {
                            results.insert(*digest, seq_and_ts);
                        }
                    }
                    return Ok(results);
                }
                _ = epoch_store.wait_epoch_terminated() => {}
            }

            // The epoch ended mid-wait, and this epoch store's notifications
            // can no longer fire: whatever is still uncheckpointed here is
            // checkpointed in the next epoch, on the next store. Cancelling
            // the wait may also have dropped notifications it had already
            // received, but the table write precedes each notification, so
            // re-reading the table recovers them.
            let found = match epoch_store.multi_get_transaction_checkpoint(&remaining) {
                Ok(found) => found,
                // The table handles were already released. They are released
                // long after the epoch's checkpoints are executed, so nothing
                // waited on here can still be checkpointed in the old epoch;
                // move on to the next store.
                Err(IotaError::EpochEnded(_)) => vec![None; remaining.len()],
                Err(err) => return Err(err),
            };
            let mut still_uncheckpointed = Vec::new();
            for (digest, found_seq) in remaining.iter().zip(found) {
                match found_seq {
                    Some(seq) => {
                        let ts = self
                            .checkpoint_timestamp_ms_cached(seq, &mut checkpoint_timestamp_cache);
                        results.insert(*digest, (seq, ts));
                    }
                    None => still_uncheckpointed.push(*digest),
                }
            }
            remaining = still_uncheckpointed;
            if remaining.is_empty() {
                return Ok(results);
            }

            match self
                .wait_for_next_epoch_store(epoch_store.epoch(), deadline)
                .await
            {
                Some(next) => epoch_store = next,
                None => return Ok(results),
            }
        }
    }

    /// Wait for the epoch store to be swapped to an epoch later than
    /// `prev_epoch`, returning `None` if `deadline` passes first.
    async fn wait_for_next_epoch_store(
        &self,
        prev_epoch: EpochId,
        deadline: tokio::time::Instant,
    ) -> Option<Arc<AuthorityPerEpochStore>> {
        // There is no notification for the epoch-store swap, and termination
        // and swap can come in either order (`reconfigure` terminates the old
        // epoch first, `reconfigure_for_testing` swaps first), so the swap is
        // polled at this interval.
        const EPOCH_STORE_SWAP_POLL_INTERVAL: Duration = Duration::from_millis(100);

        loop {
            // Deliberately re-loaded on each poll; the one-call-per-task rule
            // guards against *unaware* mixing of epoch stores within a task.
            let current = self.load_epoch_store_one_call_per_task().clone();
            if current.epoch() > prev_epoch {
                return Some(current);
            }
            if tokio::time::Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(EPOCH_STORE_SWAP_POLL_INTERVAL).await;
        }
    }

    /// Resolve a checkpoint's timestamp, memoizing lookups in `cache` so
    /// multiple transactions in the same checkpoint trigger a single
    /// checkpoint summary lookup.
    fn checkpoint_timestamp_ms_cached(
        &self,
        seq: CheckpointSequenceNumber,
        cache: &mut HashMap<CheckpointSequenceNumber, u64>,
    ) -> u64 {
        *cache.entry(seq).or_insert_with(|| {
            self.get_checkpoint_by_sequence_number(seq)
                .ok()
                .flatten()
                .map(|c| c.timestamp_ms)
                .unwrap_or(0)
        })
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_transaction_checkpoint_for_tests(
        &self,
        digest: &TransactionDigest,
        epoch_store: &AuthorityPerEpochStore,
    ) -> IotaResult<Option<VerifiedCheckpoint>> {
        let checkpoint = epoch_store.get_transaction_checkpoint(digest)?;
        let Some(checkpoint) = checkpoint else {
            return Ok(None);
        };
        let checkpoint = self
            .checkpoint_store
            .get_checkpoint_by_sequence_number(checkpoint)?;
        Ok(checkpoint)
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_object_read(&self, object_id: &ObjectId) -> IotaResult<ObjectRead> {
        Ok(
            match self
                .get_object_cache_reader()
                .try_get_latest_object_or_tombstone(*object_id)?
            {
                Some((_, ObjectOrTombstone::Object(object))) => {
                    let layout = self.get_object_layout(&object)?;
                    ObjectRead::Exists(object.object_ref(), object, layout)
                }
                Some((_, ObjectOrTombstone::Tombstone(objref))) => ObjectRead::Deleted(objref),
                None => ObjectRead::NotExists(*object_id),
            },
        )
    }

    /// Chain Identifier is the digest of the genesis checkpoint.
    pub fn get_chain_identifier(&self) -> ChainIdentifier {
        self.chain_identifier
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_move_object<T>(&self, object_id: &ObjectId) -> IotaResult<T>
    where
        T: DeserializeOwned,
    {
        let o = self.get_object_read(object_id)?.into_object()?;
        if let Some(move_object) = o.data.as_opt_struct() {
            Ok(bcs::from_bytes(move_object.contents()).map_err(|e| {
                IotaError::ObjectDeserialization {
                    error: format!("{e}"),
                }
            })?)
        } else {
            Err(IotaError::ObjectDeserialization {
                error: format!("Provided object : [{object_id}] is not a Move object."),
            })
        }
    }

    /// This function aims to serve rpc reads on past objects and
    /// we don't expect it to be called for other purposes.
    /// Depending on the object pruning policies that will be enforced in the
    /// future there is no software-level guarantee/SLA to retrieve an object
    /// with an old version even if it exists/existed.
    #[instrument(level = "trace", skip_all)]
    pub fn get_past_object_read(
        &self,
        object_id: &ObjectId,
        version: Version,
    ) -> IotaResult<PastObjectRead> {
        // Firstly we see if the object ever existed by getting its latest data
        let Some(obj_ref) = self
            .get_object_cache_reader()
            .try_get_latest_object_ref_or_tombstone(*object_id)?
        else {
            return Ok(PastObjectRead::ObjectNotExists(*object_id));
        };

        if version > obj_ref.version {
            return Ok(PastObjectRead::VersionTooHigh {
                object_id: *object_id,
                asked_version: version,
                latest_version: obj_ref.version,
            });
        }

        if version < obj_ref.version {
            // Read past objects
            return Ok(match self.read_object_at_version(object_id, version)? {
                Some((object, layout)) => {
                    let obj_ref = object.object_ref();
                    PastObjectRead::VersionFound(obj_ref, object, layout)
                }

                None => PastObjectRead::VersionNotFound(*object_id, version),
            });
        }

        if !obj_ref.digest.is_alive() {
            return Ok(PastObjectRead::ObjectDeleted(obj_ref));
        }

        match self.read_object_at_version(object_id, obj_ref.version)? {
            Some((object, layout)) => Ok(PastObjectRead::VersionFound(obj_ref, object, layout)),
            None => {
                debug_fatal!(
                    "Object with in parent_entry is missing from object store, datastore is \
                     inconsistent",
                );
                Err(UserInputError::ObjectNotFound {
                    object_id: *object_id,
                    version: Some(obj_ref.version),
                }
                .into())
            }
        }
    }

    #[instrument(level = "trace", skip_all)]
    fn read_object_at_version(
        &self,
        object_id: &ObjectId,
        version: Version,
    ) -> IotaResult<Option<(Object, Option<MoveStructLayout>)>> {
        let Some(object) = self
            .get_object_cache_reader()
            .try_get_object_by_key(object_id, version)?
        else {
            return Ok(None);
        };

        let layout = self.get_object_layout(&object)?;
        Ok(Some((object, layout)))
    }

    fn get_object_layout(&self, object: &Object) -> IotaResult<Option<MoveStructLayout>> {
        let layout = object
            .data
            .as_opt_struct()
            .map(|object| {
                into_struct_layout(
                    self.load_epoch_store_one_call_per_task()
                        .executor()
                        // TODO(cache) - must read through cache
                        .type_layout_resolver(Box::new(self.get_backing_package_store().as_ref()))
                        .get_annotated_layout(object.struct_tag())?,
                )
            })
            .transpose()?;
        Ok(layout)
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_owner_objects(
        &self,
        owner: Address,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<ObjectId>,
        limit: usize,
        filter: Option<IotaObjectDataFilter>,
    ) -> IotaResult<Vec<ObjectInfo>> {
        if let Some(indexes) = &self.indexes {
            indexes.get_owner_objects(owner, cursor, limit, filter)
        } else {
            Err(IotaError::IndexStoreNotAvailable)
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_owned_coins_iterator_with_cursor(
        &self,
        owner: Address,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: (String, ObjectId),
        limit: usize,
        one_coin_type_only: bool,
    ) -> IotaResult<impl Iterator<Item = (String, ObjectId, CoinInfo)> + '_> {
        if let Some(indexes) = &self.indexes {
            indexes.get_owned_coins_iterator_with_cursor(owner, cursor, limit, one_coin_type_only)
        } else {
            Err(IotaError::IndexStoreNotAvailable)
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_owner_objects_iterator(
        &self,
        owner: Address,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<ObjectId>,
        filter: Option<IotaObjectDataFilter>,
    ) -> IotaResult<impl Iterator<Item = ObjectInfo> + '_> {
        let cursor_u = cursor.unwrap_or(ObjectId::ZERO);
        if let Some(indexes) = &self.indexes {
            indexes.get_owner_objects_iterator(owner, cursor_u, filter)
        } else {
            Err(IotaError::IndexStoreNotAvailable)
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_move_objects<T>(&self, owner: Address, tag: StructTag) -> IotaResult<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let object_ids = self
            .get_owner_objects_iterator(owner, None, None)?
            .filter(|o| match &o.object_type {
                ObjectType::Struct(s) => *s == tag,
                ObjectType::Package => false,
            })
            .map(|info| ObjectKey(info.object_id, info.version))
            .collect::<Vec<_>>();
        let mut move_objects = vec![];

        let objects = self
            .get_object_store()
            .try_multi_get_objects_by_key(&object_ids)?;

        for (o, id) in objects.into_iter().zip(object_ids) {
            let object = o.ok_or_else(|| {
                IotaError::from(UserInputError::ObjectNotFound {
                    object_id: id.0,
                    version: Some(id.1),
                })
            })?;
            let move_object = object.data.as_opt_struct().ok_or_else(|| {
                IotaError::from(UserInputError::MovePackageAsObject { object_id: id.0 })
            })?;
            move_objects.push(bcs::from_bytes(move_object.contents()).map_err(|e| {
                IotaError::ObjectDeserialization {
                    error: format!("{e}"),
                }
            })?);
        }
        Ok(move_objects)
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_dynamic_fields(
        &self,
        owner: ObjectId,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<ObjectId>,
        limit: usize,
    ) -> IotaResult<Vec<(ObjectId, DynamicFieldInfo)>> {
        Ok(self
            .get_dynamic_fields_iterator(owner, cursor)?
            .take(limit)
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn get_dynamic_fields_iterator(
        &self,
        owner: ObjectId,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<ObjectId>,
    ) -> IotaResult<impl Iterator<Item = Result<(ObjectId, DynamicFieldInfo), TypedStoreError>> + '_>
    {
        if let Some(indexes) = &self.indexes {
            indexes.get_dynamic_fields_iterator(owner, cursor)
        } else {
            Err(IotaError::IndexStoreNotAvailable)
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_dynamic_field_object_id(
        &self,
        owner: ObjectId,
        name_type: TypeTag,
        name_bcs_bytes: &[u8],
    ) -> IotaResult<Option<ObjectId>> {
        if let Some(indexes) = &self.indexes {
            indexes.get_dynamic_field_object_id(owner, name_type, name_bcs_bytes)
        } else {
            Err(IotaError::IndexStoreNotAvailable)
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_total_transaction_blocks(&self) -> IotaResult<u64> {
        Ok(self.get_indexes()?.next_sequence_number())
    }

    #[instrument(level = "trace", skip_all)]
    pub async fn get_executed_transaction_and_effects(
        &self,
        digest: TransactionDigest,
        kv_store: Arc<TransactionKeyValueStore>,
    ) -> IotaResult<(TransactionEnvelope, TransactionEffects)> {
        let transaction = kv_store.get_tx(digest).await?;
        let effects = kv_store.get_fx_by_tx_digest(digest).await?;
        Ok((transaction, effects))
    }

    #[instrument(level = "trace", skip_all)]
    pub fn multi_get_checkpoint_by_sequence_number(
        &self,
        sequence_numbers: &[CheckpointSequenceNumber],
    ) -> IotaResult<Vec<Option<VerifiedCheckpoint>>> {
        Ok(self
            .checkpoint_store
            .multi_get_checkpoint_by_sequence_number(sequence_numbers)?)
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_transaction_events(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<TransactionEvents> {
        self.get_transaction_cache_reader()
            .try_get_events(digest)?
            .ok_or(IotaError::TransactionEventsNotFound { digest: *digest })
    }

    pub fn get_transaction_input_objects(
        &self,
        effects: &TransactionEffects,
    ) -> anyhow::Result<Vec<Object>> {
        iota_types::storage::get_transaction_input_objects(self.get_object_store(), effects)
            .map_err(Into::into)
    }

    pub fn get_transaction_output_objects(
        &self,
        effects: &TransactionEffects,
    ) -> anyhow::Result<Vec<Object>> {
        iota_types::storage::get_transaction_output_objects(self.get_object_store(), effects)
            .map_err(Into::into)
    }

    fn get_indexes(&self) -> IotaResult<Arc<IndexStore>> {
        match &self.indexes {
            Some(i) => Ok(i.clone()),
            None => Err(IotaError::UnsupportedFeature {
                error: "extended object indexing is not enabled on this server".into(),
            }),
        }
    }

    pub async fn get_transactions_for_tests(
        self: &Arc<Self>,
        filter: Option<TransactionFilter>,
        cursor: Option<TransactionDigest>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        let metrics = KeyValueStoreMetrics::new_for_tests();
        let kv_store = Arc::new(TransactionKeyValueStore::new(
            "rocksdb",
            metrics,
            self.clone(),
        ));
        self.get_transactions(&kv_store, filter, cursor, limit, reverse)
            .await
    }

    #[instrument(level = "trace", skip_all)]
    pub async fn get_transactions(
        &self,
        kv_store: &Arc<TransactionKeyValueStore>,
        filter: Option<TransactionFilter>,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<TransactionDigest>,
        limit: Option<usize>,
        reverse: bool,
    ) -> IotaResult<Vec<TransactionDigest>> {
        if let Some(TransactionFilter::Checkpoint(sequence_number)) = filter {
            let checkpoint_contents = kv_store.get_checkpoint_contents(sequence_number).await?;
            let iter = checkpoint_contents.iter().map(|c| c.transaction);
            if reverse {
                let iter = iter
                    .rev()
                    .skip_while(|d| cursor.is_some() && Some(*d) != cursor)
                    .skip(usize::from(cursor.is_some()));
                return Ok(iter.take(limit.unwrap_or(usize::MAX)).collect());
            } else {
                let iter = iter
                    .skip_while(|d| cursor.is_some() && Some(*d) != cursor)
                    .skip(usize::from(cursor.is_some()));
                return Ok(iter.take(limit.unwrap_or(usize::MAX)).collect());
            }
        }
        self.get_indexes()?
            .get_transactions(filter, cursor, limit, reverse)
    }

    pub fn get_checkpoint_store(&self) -> &Arc<CheckpointStore> {
        &self.checkpoint_store
    }

    /// The store pruner; the checkpoint executor uses it to nudge the pruner
    /// after each checkpoint.
    pub fn pruner(&self) -> &AuthorityStorePruner {
        &self.pruner
    }

    pub fn get_latest_checkpoint_sequence_number(&self) -> IotaResult<CheckpointSequenceNumber> {
        self.get_checkpoint_store()
            .get_highest_executed_checkpoint_seq_number()?
            .ok_or(IotaError::UserInput {
                error: UserInputError::LatestCheckpointSequenceNumberNotFound,
            })
    }

    #[cfg(msim)]
    pub fn get_highest_pruned_checkpoint_for_testing(
        &self,
    ) -> IotaResult<CheckpointSequenceNumber> {
        self.database_for_testing()
            .perpetual_tables
            .get_highest_pruned_checkpoint()
            .map(|c| c.unwrap_or(0))
            .map_err(Into::into)
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_checkpoint_summary_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> IotaResult<CheckpointSummary> {
        let verified_checkpoint = self
            .get_checkpoint_store()
            .get_checkpoint_by_sequence_number(sequence_number)?;
        match verified_checkpoint {
            Some(verified_checkpoint) => Ok(verified_checkpoint.into_inner().into_data()),
            None => Err(IotaError::UserInput {
                error: UserInputError::VerifiedCheckpointNotFound(sequence_number),
            }),
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_checkpoint_summary_by_digest(
        &self,
        digest: CheckpointDigest,
    ) -> IotaResult<CheckpointSummary> {
        let verified_checkpoint = self
            .get_checkpoint_store()
            .get_checkpoint_by_digest(&digest)?;
        match verified_checkpoint {
            Some(verified_checkpoint) => Ok(verified_checkpoint.into_inner().into_data()),
            None => Err(IotaError::UserInput {
                error: UserInputError::VerifiedCheckpointDigestNotFound(Base58::encode(digest)),
            }),
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn find_publish_txn_digest(&self, package_id: ObjectId) -> IotaResult<TransactionDigest> {
        if package_id.is_system_package() {
            return self.find_genesis_txn_digest();
        }
        Ok(self
            .get_object_read(&package_id)?
            .into_object()?
            .previous_transaction)
    }

    #[instrument(level = "trace", skip_all)]
    pub fn find_genesis_txn_digest(&self) -> IotaResult<TransactionDigest> {
        let summary = self
            .get_verified_checkpoint_by_sequence_number(0)?
            .into_message();
        let content = self.get_checkpoint_contents(summary.contents_digest)?;
        let genesis_transaction = content.enumerate_transactions(&summary).next();
        Ok(genesis_transaction
            .ok_or(IotaError::UserInput {
                error: UserInputError::GenesisTransactionNotFound,
            })?
            .1
            .transaction)
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_verified_checkpoint_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> IotaResult<VerifiedCheckpoint> {
        let verified_checkpoint = self
            .get_checkpoint_store()
            .get_checkpoint_by_sequence_number(sequence_number)?;
        match verified_checkpoint {
            Some(verified_checkpoint) => Ok(verified_checkpoint),
            None => Err(IotaError::UserInput {
                error: UserInputError::VerifiedCheckpointNotFound(sequence_number),
            }),
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_verified_checkpoint_summary_by_digest(
        &self,
        digest: CheckpointDigest,
    ) -> IotaResult<VerifiedCheckpoint> {
        let verified_checkpoint = self
            .get_checkpoint_store()
            .get_checkpoint_by_digest(&digest)?;
        match verified_checkpoint {
            Some(verified_checkpoint) => Ok(verified_checkpoint),
            None => Err(IotaError::UserInput {
                error: UserInputError::VerifiedCheckpointDigestNotFound(Base58::encode(digest)),
            }),
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_checkpoint_contents(
        &self,
        digest: CheckpointContentsDigest,
    ) -> IotaResult<CheckpointContents> {
        self.get_checkpoint_store()
            .get_checkpoint_contents(&digest)?
            .ok_or(IotaError::UserInput {
                error: UserInputError::CheckpointContentsNotFound(digest),
            })
    }

    #[instrument(level = "trace", skip_all)]
    pub fn get_checkpoint_contents_by_sequence_number(
        &self,
        sequence_number: CheckpointSequenceNumber,
    ) -> IotaResult<CheckpointContents> {
        let verified_checkpoint = self
            .get_checkpoint_store()
            .get_checkpoint_by_sequence_number(sequence_number)?;
        match verified_checkpoint {
            Some(verified_checkpoint) => {
                let contents_digest = verified_checkpoint.into_inner().contents_digest;
                self.get_checkpoint_contents(contents_digest)
            }
            None => Err(IotaError::UserInput {
                error: UserInputError::VerifiedCheckpointNotFound(sequence_number),
            }),
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub async fn query_events(
        &self,
        kv_store: &Arc<TransactionKeyValueStore>,
        query: EventFilter,
        // If `Some`, the query will start from the next item after the specified cursor
        cursor: Option<EventID>,
        limit: usize,
        descending: bool,
    ) -> IotaResult<Vec<IotaEvent>> {
        let index_store = self.get_indexes()?;

        // Get the tx_num from tx_digest
        let (tx_num, event_num) = if let Some(cursor) = cursor.as_ref() {
            let tx_seq = index_store.get_transaction_seq(&cursor.tx_digest)?.ok_or(
                IotaError::TransactionNotFound {
                    digest: cursor.tx_digest,
                },
            )?;
            (tx_seq, cursor.event_seq as usize)
        } else if descending {
            (u64::MAX, usize::MAX)
        } else {
            (0, 0)
        };

        let limit = limit + 1;
        let mut event_keys = match query {
            EventFilter::All(filters) => {
                if filters.is_empty() {
                    index_store.all_events(tx_num, event_num, limit, descending)?
                } else {
                    return Err(IotaError::UserInput {
                        error: UserInputError::Unsupported(
                            "This query type does not currently support filter combinations"
                                .to_string(),
                        ),
                    });
                }
            }
            EventFilter::Transaction(digest) => {
                index_store.events_by_transaction(&digest, tx_num, event_num, limit, descending)?
            }
            EventFilter::MoveModule { package, module } => {
                let module_id = ModuleId::new(
                    AccountAddress::new(package.into_bytes()),
                    move_core_types::identifier::Identifier::new(module.as_str()).unwrap(),
                );
                index_store.events_by_module_id(&module_id, tx_num, event_num, limit, descending)?
            }
            EventFilter::MoveEventType(struct_name) => index_store
                .events_by_move_event_struct_name(
                    &struct_name,
                    tx_num,
                    event_num,
                    limit,
                    descending,
                )?,
            EventFilter::Sender(sender) => {
                index_store.events_by_sender(&sender, tx_num, event_num, limit, descending)?
            }
            EventFilter::TimeRange {
                start_time,
                end_time,
            } => index_store
                .event_iterator(start_time, end_time, tx_num, event_num, limit, descending)?,
            EventFilter::MoveEventModule { package, module } => index_store
                .events_by_move_event_module(
                    &ModuleId::new(
                        AccountAddress::new(package.into_bytes()),
                        move_core_types::identifier::Identifier::new(module.as_str()).unwrap(),
                    ),
                    tx_num,
                    event_num,
                    limit,
                    descending,
                )?,
            // not using "_ =>" because we want to make sure we remember to add new variants here
            EventFilter::Package(_)
            | EventFilter::MoveEventField { .. }
            | EventFilter::Any(_)
            | EventFilter::And(_, _)
            | EventFilter::Or(_, _) => {
                return Err(IotaError::UserInput {
                    error: UserInputError::Unsupported(
                        "This query type is not supported by the full node.".to_string(),
                    ),
                });
            }
        };

        // skip one event if exclusive cursor is provided,
        // otherwise truncate to the original limit.
        if cursor.is_some() {
            if !event_keys.is_empty() {
                event_keys.remove(0);
            }
        } else {
            event_keys.truncate(limit - 1);
        }

        // get the unique set of digests from the event_keys
        let transaction_digests = event_keys
            .iter()
            .map(|(_, digest, _, _)| *digest)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let events = kv_store
            .multi_get_events_by_tx_digests(&transaction_digests)
            .await?;

        let events_map: HashMap<_, _> =
            transaction_digests.iter().zip(events.into_iter()).collect();

        let stored_events = event_keys
            .into_iter()
            .map(|k| {
                (
                    k,
                    events_map
                        .get(&k.1)
                        .expect("fetched digest is missing")
                        .clone()
                        .and_then(|e| e.get(k.2).cloned()),
                )
            })
            .map(
                |((_event_digest, tx_digest, event_seq, timestamp), event)| {
                    event
                        .map(|e| (e, tx_digest, event_seq, timestamp))
                        .ok_or(IotaError::TransactionEventsNotFound { digest: tx_digest })
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        let epoch_store = self.load_epoch_store_one_call_per_task();
        let backing_store = self.get_backing_package_store().as_ref();
        let mut layout_resolver = epoch_store
            .executor()
            .type_layout_resolver(Box::new(backing_store));
        let mut events = vec![];
        for (e, tx_digest, event_seq, timestamp) in stored_events.into_iter() {
            events.push(IotaEvent::try_from(
                e.clone(),
                tx_digest,
                event_seq as u64,
                Some(timestamp),
                layout_resolver.get_annotated_layout(&e.struct_tag)?,
            )?)
        }
        Ok(events)
    }

    pub fn insert_genesis_object(&self, object: Object) {
        self.get_reconfig_api()
            .try_insert_genesis_object(object)
            .expect("Cannot insert genesis object")
    }

    pub fn insert_genesis_objects(&self, objects: &[Object]) {
        for o in objects {
            self.insert_genesis_object(o.clone());
        }
    }

    /// Make a status response for a transaction
    #[instrument(level = "trace", skip_all)]
    pub fn get_transaction_status(
        &self,
        transaction_digest: &TransactionDigest,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<Option<(SenderSignedTransaction, TransactionStatus)>> {
        // TODO: In the case of read path, we should not have to re-sign the effects.
        if let Some(effects) =
            self.get_signed_effects_and_maybe_resign(transaction_digest, epoch_store)?
        {
            if let Some(transaction) = self
                .get_transaction_cache_reader()
                .try_get_transaction_block(transaction_digest)?
            {
                let cert_sig = epoch_store.get_transaction_cert_sig(transaction_digest)?;
                let events = if effects.events_digest().is_some() {
                    self.get_transaction_events(effects.transaction_digest())?
                } else {
                    TransactionEvents::default()
                };
                return Ok(Some((
                    (*transaction).clone().into_message(),
                    TransactionStatus::Executed(cert_sig, effects.into_inner(), events),
                )));
            } else {
                // The read of effects and read of transaction are not atomic. It's possible
                // that we reverted the transaction (during epoch change) in
                // between the above two reads, and we end up having effects but
                // not transaction. In this case, we just fall through.
                debug!(tx_digest=?transaction_digest, "Signed effects exist but no transaction found");
            }
        }
        if let Some(signed) = epoch_store.get_signed_transaction(transaction_digest)? {
            self.metrics.tx_already_processed.inc();
            let (transaction, sig) = signed.into_inner().into_data_and_sig();
            Ok(Some((transaction, TransactionStatus::Signed(sig))))
        } else {
            Ok(None)
        }
    }

    /// Get the signed effects of the given transaction. If the effects was
    /// signed in a previous epoch, re-sign it so that the caller is able to
    /// form a cert of the effects in the current epoch.
    #[instrument(level = "trace", skip_all)]
    pub fn get_signed_effects_and_maybe_resign(
        &self,
        transaction_digest: &TransactionDigest,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<Option<VerifiedSignedTransactionEffects>> {
        let effects = self
            .get_transaction_cache_reader()
            .try_get_executed_effects(transaction_digest)?;
        match effects {
            Some(effects) => {
                // If the transaction was executed in previous epochs, the validator will
                // re-sign the effects with new current epoch so that a client is always able to
                // obtain an effects certificate at the current epoch.
                //
                // Why is this necessary? Consider the following case:
                // - assume there are 4 validators
                // - Quorum driver gets 2 signed effects before reconfig halt
                // - The tx makes it into final checkpoint.
                // - 2 validators go away and are replaced in the new epoch.
                // - The new epoch begins.
                // - The quorum driver cannot complete the partial effects cert from the
                //   previous epoch, because it may not be able to reach either of the 2 former
                //   validators.
                // - But, if the 2 validators that stayed are willing to re-sign the effects in
                //   the new epoch, the QD can make a new effects cert and return it to the
                //   client.
                //
                // This is a considered a short-term workaround. Eventually, Quorum Driver
                // should be able to return either an effects certificate, -or-
                // a proof of inclusion in a checkpoint. In the case above, the
                // Quorum Driver would return a proof of inclusion in the final
                // checkpoint, and this code would no longer be necessary.
                if effects.epoch() != epoch_store.epoch() {
                    debug!(
                        tx_digest=?transaction_digest,
                        effects_epoch=?effects.epoch(),
                        epoch=?epoch_store.epoch(),
                        "Re-signing the effects with the current epoch"
                    );
                }
                Ok(Some(self.sign_effects(effects, epoch_store)?))
            }
            None => Ok(None),
        }
    }

    /// A client aggregating effects signatures towards a quorum assumes
    /// finality once it collects 2f+1 of them, so within an epoch this
    /// validator must never assert two different effects for the same
    /// transaction on any RPC surface, signed or unsigned. Executed effects
    /// can change across a restart if an uncommitted transaction is
    /// re-executed with divergent results (e.g. by a new binary), so every
    /// effects-reporting path calls this before returning effects, and
    /// refuses to contradict a signature that may already be in a client's
    /// hands.
    pub fn check_effects_against_previously_signed(
        &self,
        epoch_store: &AuthorityPerEpochStore,
        tx_digest: &TransactionDigest,
        effects_digest: &TransactionEffectsDigest,
        surface: &'static str,
    ) -> IotaResult<()> {
        if let Some(previously_signed_digest) = epoch_store.get_signed_effects_digest(tx_digest)? {
            if previously_signed_digest != *effects_digest {
                self.metrics
                    .signed_effects_equivocation_prevented
                    .with_label_values(&[surface])
                    .inc();
                error!(
                    ?tx_digest,
                    ?previously_signed_digest,
                    executed_digest = ?effects_digest,
                    surface,
                    "refusing to report effects that differ from previously signed effects"
                );
                return Err(IotaError::GenericAuthority {
                    error: format!(
                        "Refusing to report effects for transaction {tx_digest}: effects digest \
                         {effects_digest} differs from previously signed effects digest \
                         {previously_signed_digest}"
                    ),
                });
            }
        }
        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    pub(crate) fn sign_effects(
        &self,
        effects: TransactionEffects,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> IotaResult<VerifiedSignedTransactionEffects> {
        let tx_digest = *effects.transaction_digest();

        self.check_effects_against_previously_signed(
            epoch_store,
            &tx_digest,
            &effects.digest(),
            "sign_effects",
        )?;

        let signed_effects = match epoch_store.get_effects_signature(&tx_digest)? {
            Some(sig) => {
                debug_assert!(sig.epoch == epoch_store.epoch());
                SignedTransactionEffects::new_from_data_and_sig(effects, sig)
            }
            _ => {
                let sig = AuthoritySignInfo::new(
                    epoch_store.epoch(),
                    &effects,
                    Intent::iota_app(IntentScope::TransactionEffects),
                    self.name,
                    &*self.secret,
                );

                let effects = SignedTransactionEffects::new_from_data_and_sig(effects, sig.clone());

                epoch_store.insert_effects_digest_and_signature(
                    &tx_digest,
                    effects.digest(),
                    &sig,
                )?;

                effects
            }
        };

        Ok(VerifiedSignedTransactionEffects::new_unchecked(
            signed_effects,
        ))
    }

    /// Get the transaction envelope that currently locks the given object, if
    /// any. Since object locks are only valid for one epoch, we also need
    /// the epoch_id in the query. Returns UserInputError::ObjectNotFound if
    /// no lock records for the given object can be found.
    /// Returns UserInputError::ObjectVersionUnavailableForConsumption if the
    /// object record is at a different version.
    /// Returns Some(VerifiedEnvelope) if the given ObjectReference is locked by
    /// a certain transaction. Returns None if the a lock record is
    /// initialized for the given ObjectReference but not yet locked by any
    /// transaction,     or cannot find the transaction in transaction
    /// table, because of data race etc.
    #[instrument(level = "trace", skip_all)]
    pub fn get_transaction_lock(
        &self,
        object_ref: &ObjectReference,
        epoch_store: &AuthorityPerEpochStore,
    ) -> IotaResult<Option<VerifiedSignedTransaction>> {
        let lock_info = self
            .get_object_cache_reader()
            .try_get_lock(*object_ref, epoch_store)?;
        let lock_info = match lock_info {
            ObjectLockStatus::LockedAtDifferentVersion { locked_ref } => {
                return Err(UserInputError::ObjectVersionUnavailableForConsumption {
                    provided_obj_ref: *object_ref,
                    current_version: locked_ref.version,
                }
                .into());
            }
            ObjectLockStatus::Initialized => {
                return Ok(None);
            }
            ObjectLockStatus::LockedToTx { locked_by_tx } => locked_by_tx,
        };

        epoch_store.get_signed_transaction(&lock_info)
    }

    pub fn try_get_objects(&self, objects: &[ObjectId]) -> IotaResult<Vec<Option<Object>>> {
        self.get_object_cache_reader().try_get_objects(objects)
    }

    /// Non-fallible version of `try_get_objects`.
    pub fn get_objects(&self, objects: &[ObjectId]) -> Vec<Option<Object>> {
        self.try_get_objects(objects)
            .expect("storage access failed")
    }

    pub fn try_get_object_or_tombstone(
        &self,
        object_id: ObjectId,
    ) -> IotaResult<Option<ObjectReference>> {
        self.get_object_cache_reader()
            .try_get_latest_object_ref_or_tombstone(object_id)
    }

    /// Non-fallible version of `try_get_object_or_tombstone`.
    pub fn get_object_or_tombstone(&self, object_id: ObjectId) -> Option<ObjectReference> {
        self.try_get_object_or_tombstone(object_id)
            .expect("storage access failed")
    }

    /// Ordinarily, protocol upgrades occur when 2f + 1 + (f *
    /// ProtocolConfig::buffer_stake_for_protocol_upgrade_bps) vote for the
    /// upgrade.
    ///
    /// This method can be used to dynamic adjust the amount of buffer. If set
    /// to 0, the upgrade will go through with only 2f+1 votes.
    ///
    /// IMPORTANT: If this is used, it must be used on >=2f+1 validators (all
    /// should have the same value), or you risk halting the chain.
    pub fn set_override_protocol_upgrade_buffer_stake(
        &self,
        expected_epoch: EpochId,
        buffer_stake_bps: u64,
    ) -> IotaResult {
        let epoch_store = self.load_epoch_store_one_call_per_task();
        let actual_epoch = epoch_store.epoch();
        if actual_epoch != expected_epoch {
            return Err(IotaError::WrongEpoch {
                expected_epoch,
                actual_epoch,
            });
        }

        epoch_store.set_override_protocol_upgrade_buffer_stake(buffer_stake_bps)
    }

    pub fn clear_override_protocol_upgrade_buffer_stake(
        &self,
        expected_epoch: EpochId,
    ) -> IotaResult {
        let epoch_store = self.load_epoch_store_one_call_per_task();
        let actual_epoch = epoch_store.epoch();
        if actual_epoch != expected_epoch {
            return Err(IotaError::WrongEpoch {
                expected_epoch,
                actual_epoch,
            });
        }

        epoch_store.clear_override_protocol_upgrade_buffer_stake()
    }

    /// Get the set of system packages that are compiled in to this build, if
    /// those packages are compatible with the current versions of those
    /// packages on-chain.
    pub async fn get_available_system_packages(
        &self,
        binary_config: &BinaryConfig,
    ) -> Vec<ObjectReference> {
        let mut results = vec![];

        let system_packages = BuiltInFramework::iter_system_packages();

        // Add extra framework packages during simtest
        #[cfg(msim)]
        let extra_packages = framework_injection::get_extra_packages(self.name);
        #[cfg(msim)]
        let system_packages = {
            let mut packages: Vec<_> = system_packages.collect();
            packages.extend(extra_packages.iter());
            packages
        };

        for system_package in system_packages {
            let modules = system_package.modules().to_vec();
            // In simtests, we could override the current built-in framework packages.
            #[cfg(msim)]
            let modules = framework_injection::get_override_modules(&system_package.id, self.name)
                .unwrap_or(modules);

            let Some(obj_ref) = iota_framework::compare_system_package(
                &self.get_object_store(),
                &system_package.id,
                &modules,
                system_package.dependencies.to_vec(),
                binary_config,
            )
            .await
            else {
                return vec![];
            };
            results.push(obj_ref);
        }

        results
    }

    /// Return the new versions, module bytes, and dependencies for the packages
    /// that have been committed to for a framework upgrade, in
    /// `system_packages`.  Loads the module contents from the binary, and
    /// performs the following checks:
    ///
    /// - Whether its contents matches what is on-chain already, in which case
    ///   no upgrade is required, and its contents are omitted from the output.
    /// - Whether the contents in the binary can form a package whose digest
    ///   matches the input, meaning the framework will be upgraded, and this
    ///   authority can satisfy that upgrade, in which case the contents are
    ///   included in the output.
    ///
    /// If a needed version of the framework can't be loaded, the binary does
    /// not contain the bytes for that framework ID, or the resulting
    /// package fails the digest check, `None` is returned indicating that
    /// this authority cannot run the upgrade that the network voted on.
    ///
    /// All object lookups are pinned to the versions in `system_packages`
    /// instead of using the latest versions, so that the result is
    /// deterministic even if the change epoch transaction that performs the
    /// upgrade has already been executed locally (e.g. via state sync). In
    /// that case the reconstructed change epoch transaction is byte-identical
    /// to the executed one, and the caller detects it as already executed.
    async fn get_system_package_bytes(
        &self,
        system_packages: Vec<ObjectReference>,
        binary_config: &BinaryConfig,
    ) -> Option<Vec<SystemPackage>> {
        let object_store = self.get_object_cache_reader();

        let mut res = Vec::with_capacity(system_packages.len());
        for system_package_ref in system_packages {
            if object_store
                .get_object_by_key(&system_package_ref.object_id, system_package_ref.version)
                .is_some_and(|object| object.object_ref() == system_package_ref)
            {
                // Skip this one because it doesn't need to be upgraded.
                info!(
                    "Framework {} does not need updating",
                    system_package_ref.object_id
                );
                continue;
            }

            // The digest in `system_package_ref` commits to a package built on top of the
            // predecessor version's `previous_transaction` (see `compare_system_package`),
            // so it must be re-derived from that version. A ref at
            // `Version::OBJECT_START` is a freshly created package with no predecessor.
            let prev_transaction = if system_package_ref.version == Version::OBJECT_START {
                TransactionDigest::GENESIS_MARKER
            } else {
                let prev_version = system_package_ref
                    .version
                    .previous()
                    .expect("version is greater than Version::OBJECT_START");
                let Some(prev_object) =
                    object_store.get_object_by_key(&system_package_ref.object_id, prev_version)
                else {
                    error!(
                        "Framework {} not available locally at version {prev_version:?}, cannot \
                         derive upgrade to {system_package_ref:?}",
                        system_package_ref.object_id
                    );
                    return None;
                };
                prev_object.previous_transaction
            };

            #[cfg(msim)]
            let FrameworkSystemPackage {
                id: _,
                bytes,
                dependencies,
            } = framework_injection::get_override_system_package(
                &system_package_ref.object_id,
                self.name,
            )
            .unwrap_or_else(|| {
                BuiltInFramework::get_package_by_id(&system_package_ref.object_id).clone()
            });

            #[cfg(not(msim))]
            let FrameworkSystemPackage {
                id: _,
                bytes,
                dependencies,
            } = BuiltInFramework::get_package_by_id(&system_package_ref.object_id).clone();

            let modules: Vec<_> = bytes
                .iter()
                .map(|m| CompiledModule::deserialize_with_config(m, binary_config).unwrap())
                .collect();

            let new_object = Object::new_system_package(
                &modules,
                system_package_ref.version,
                dependencies.clone(),
                prev_transaction,
            );

            let new_ref = new_object.object_ref();
            if new_ref != system_package_ref {
                debug_fatal!(
                    "Framework mismatch -- binary: {new_ref:?}\n  upgrade: {system_package_ref:?}"
                );
                return None;
            }

            res.push(SystemPackage {
                version: system_package_ref.version,
                modules: bytes,
                dependencies,
            });
        }

        Some(res)
    }

    /// Returns the new protocol version and system packages that the network
    /// has voted to upgrade to. If the proposed protocol version is not
    /// supported, None is returned.
    fn is_protocol_version_supported_v1(
        proposed_protocol_version: ProtocolVersion,
        committee: &Committee,
        capabilities: Vec<AuthorityCapabilitiesV1>,
        mut buffer_stake_bps: u64,
    ) -> Option<(ProtocolVersion, Digest, Vec<ObjectReference>)> {
        if buffer_stake_bps > 10000 {
            warn!("clamping buffer_stake_bps to 10000");
            buffer_stake_bps = 10000;
        }

        // For each validator, gather the protocol version and system packages that it
        // would like to upgrade to in the next epoch.
        let mut desired_upgrades: Vec<_> = capabilities
            .into_iter()
            .filter_map(|mut cap| {
                // A validator that lists no packages is voting against any change at all.
                if cap.available_system_packages.is_empty() {
                    return None;
                }

                cap.available_system_packages.sort();

                info!(
                    "validator {:?} supports {:?} with system packages: {:?}",
                    cap.authority.concise(),
                    cap.supported_protocol_versions,
                    cap.available_system_packages,
                );

                // A validator that only supports the current protocol version is also voting
                // against any change, because framework upgrades always require a protocol
                // version bump.
                cap.supported_protocol_versions
                    .get_version_digest(proposed_protocol_version)
                    .map(|digest| (digest, cap.available_system_packages, cap.authority))
            })
            .collect();

        // There can only be one set of votes that have a majority, find one if it
        // exists.
        desired_upgrades.sort();
        desired_upgrades
            .into_iter()
            .chunk_by(|(digest, packages, _authority)| (*digest, packages.clone()))
            .into_iter()
            .find_map(|((digest, packages), group)| {
                // should have been filtered out earlier.
                assert!(!packages.is_empty());

                let mut stake_aggregator: StakeAggregator<(), true> =
                    StakeAggregator::new(Arc::new(committee.clone()));

                for (_, _, authority) in group {
                    stake_aggregator.insert_generic(authority, ());
                }

                let total_votes = stake_aggregator.total_votes();
                let quorum_threshold = committee.quorum_threshold();
                let effective_threshold = committee.effective_threshold(buffer_stake_bps);

                info!(
                    protocol_config_digest = ?digest,
                    ?total_votes,
                    ?quorum_threshold,
                    ?buffer_stake_bps,
                    ?effective_threshold,
                    ?proposed_protocol_version,
                    ?packages,
                    "support for upgrade"
                );

                let has_support = total_votes >= effective_threshold;
                has_support.then_some((proposed_protocol_version, digest, packages))
            })
    }

    /// Selects the highest supported protocol version and system packages that
    /// the network has voted to upgrade to. If no upgrade is supported,
    /// returns the current protocol version and system packages.
    fn choose_protocol_version_and_system_packages_v1(
        current_protocol_version: ProtocolVersion,
        current_protocol_digest: Digest,
        committee: &Committee,
        capabilities: Vec<AuthorityCapabilitiesV1>,
        buffer_stake_bps: u64,
    ) -> (ProtocolVersion, Digest, Vec<ObjectReference>) {
        let mut next_protocol_version = current_protocol_version;
        let mut system_packages = vec![];
        let mut protocol_version_digest = current_protocol_digest;

        // Finds the highest supported protocol version and system packages by
        // incrementing the proposed protocol version by one until no further
        // upgrades are supported.
        while let Some((version, digest, packages)) = Self::is_protocol_version_supported_v1(
            next_protocol_version + 1,
            committee,
            capabilities.clone(),
            buffer_stake_bps,
        ) {
            next_protocol_version = version;
            protocol_version_digest = digest;
            system_packages = packages;
        }

        (
            next_protocol_version,
            protocol_version_digest,
            system_packages,
        )
    }

    /// Returns the indices of validators that support the given protocol
    /// version and digest. This includes both committee and non-committee
    /// validators based on their capabilities. Uses active validators
    /// instead of committee indices.
    fn get_validators_supporting_protocol_version(
        target_protocol_version: ProtocolVersion,
        target_digest: Digest,
        active_validators: &[AuthorityPublicKey],
        capabilities: &[AuthorityCapabilitiesV1],
    ) -> Vec<u64> {
        let mut eligible_validators = Vec::new();

        for capability in capabilities {
            // Check if this validator supports the target protocol version and digest
            if let Some(digest) = capability
                .supported_protocol_versions
                .get_version_digest(target_protocol_version)
            {
                if digest == target_digest {
                    // Find the validator's index in the active validators list
                    if let Some(index) = active_validators
                        .iter()
                        .position(|name| AuthorityName::from(name) == capability.authority)
                    {
                        eligible_validators.push(index as u64);
                    }
                }
            }
        }

        // Sort indices for deterministic behavior
        eligible_validators.sort();
        eligible_validators
    }

    /// Calculates the sum of weights for eligible validators that are part of
    /// the committee. Takes the indices from
    /// get_validators_supporting_protocol_version and maps them back
    /// to committee members to get their weights.
    fn calculate_eligible_validators_weight(
        eligible_validator_indices: &[u64],
        active_validators: &[AuthorityPublicKey],
        committee: &Committee,
    ) -> u64 {
        let mut total_weight = 0u64;

        for &index in eligible_validator_indices {
            let authority_pubkey = &active_validators[index as usize];
            // Check if this validator is in the committee and get their weight
            if let Some((_, weight)) = committee
                .members()
                .find(|(name, _)| *name == AuthorityName::from(authority_pubkey))
            {
                total_weight += weight;
            }
        }

        total_weight
    }

    /// Creates and execute the advance epoch transaction to effects without
    /// committing it to the database. The effects of the change epoch tx
    /// are only written to the database after a certified checkpoint has been
    /// formed and executed by CheckpointExecutor.
    ///
    /// When a framework upgraded has been decided on, but the validator does
    /// not have the new versions of the packages locally, the validator
    /// cannot form the ChangeEpochTx. In this case it returns Err,
    /// indicating that the checkpoint builder should give up trying to make the
    /// final checkpoint. As long as the network is able to create a certified
    /// checkpoint (which should be ensured by the capabilities vote), it
    /// will arrive via state sync and be executed by CheckpointExecutor.
    #[instrument(level = "error", skip_all)]
    pub async fn create_and_execute_advance_epoch_tx(
        &self,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        gas_cost_summary: &GasCostSummary,
        checkpoint: CheckpointSequenceNumber,
        epoch_start_timestamp_ms: CheckpointTimestamp,
        scores: Vec<u64>,
    ) -> CheckpointBuilderResult<(
        IotaSystemState,
        Option<SystemEpochInfoEvent>,
        TransactionEffects,
    )> {
        let mut txns = Vec::new();

        // Create the TransactionDenyRules object once: the epoch-start
        // configuration is identical on every validator, so the whole
        // committee injects (or skips) the kind together. If this epoch
        // change falls into safe mode the creation is dropped with it, the
        // object stays absent, and the next epoch end injects it again.
        if epoch_store
            .protocol_config()
            .deny_rule_governance_on_chain()
            && epoch_store
                .epoch_start_config()
                .transaction_deny_rules_obj_initial_shared_version()
                .is_none()
        {
            txns.push(EndOfEpochTransactionKind::TransactionDenyRulesCreate);
        }

        let next_epoch = epoch_store.epoch() + 1;

        let buffer_stake_bps = epoch_store.get_effective_buffer_stake_bps();
        let authority_capabilities = epoch_store
            .get_capabilities_v1()
            .expect("read capabilities from db cannot fail");
        let (next_epoch_protocol_version, next_epoch_protocol_digest, next_epoch_system_packages) =
            Self::choose_protocol_version_and_system_packages_v1(
                epoch_store.protocol_version(),
                SupportedProtocolVersionsWithHashes::protocol_config_digest(
                    epoch_store.protocol_config(),
                ),
                epoch_store.committee(),
                authority_capabilities.clone(),
                buffer_stake_bps,
            );

        // since system packages are created during the current epoch, they should abide
        // by the rules of the current epoch, including the current epoch's max
        // Move binary format version
        let config = epoch_store.protocol_config();
        let binary_config = to_binary_config(config);
        let Some(next_epoch_system_package_bytes) = self
            .get_system_package_bytes(next_epoch_system_packages.clone(), &binary_config)
            .await
        else {
            debug_fatal!(
                "upgraded system packages {:?} are not locally available, cannot create \
                ChangeEpochTx. validator binary must be upgraded to the correct version!",
                next_epoch_system_packages
            );
            // the checkpoint builder will keep retrying forever when it hits this error.
            // Eventually, one of two things will happen:
            // - The operator will upgrade this binary to one that has the new packages
            //   locally, and this function will succeed.
            // - The final checkpoint will be certified by other validators, we will receive
            //   it via state sync, and execute it. This will upgrade the framework
            //   packages, reconfigure, and most likely shut down in the new epoch (this
            //   validator likely doesn't support the new protocol version, or else it
            //   should have had the packages.)
            return Err(CheckpointBuilderError::SystemPackagesMissing);
        };

        // Use ChangeEpochV3 or ChangeEpochV4 when the feature flags are enabled and
        // ChangeEpochV2 requirements are met
        if config.select_committee_from_eligible_validators() {
            // Get the list of eligible validators that support the target protocol version
            let active_validators = epoch_store.epoch_start_state().get_active_validators();

            let mut eligible_active_validators = (0..active_validators.len() as u64).collect();

            // Use validators supporting the target protocol version as eligible validators
            // in the next version if select_committee_supporting_next_epoch_version feature
            // flag is set to true.
            if config.select_committee_supporting_next_epoch_version() {
                eligible_active_validators = Self::get_validators_supporting_protocol_version(
                    next_epoch_protocol_version,
                    next_epoch_protocol_digest,
                    &active_validators,
                    &authority_capabilities,
                );

                // Calculate the total weight of eligible validators in the committee
                let eligible_validators_weight = Self::calculate_eligible_validators_weight(
                    &eligible_active_validators,
                    &active_validators,
                    epoch_store.committee(),
                );

                // Safety check: ensure eligible validators have enough stake
                // Use the same effective threshold calculation that was used to decide the
                // protocol version
                let committee = epoch_store.committee();
                let effective_threshold = committee.effective_threshold(buffer_stake_bps);

                if eligible_validators_weight < effective_threshold {
                    error!(
                        "Eligible validators weight {eligible_validators_weight} is less than effective threshold {effective_threshold}. \
                        This could indicate a bug in validator selection logic or inconsistency with protocol version decision.",
                    );
                    // Pass all active validator indices as eligible validators
                    // to perform selection among all of them.
                    eligible_active_validators = (0..active_validators.len() as u64).collect();
                }
            }

            // Use ChangeEpochV4 when the pass_validator_scores_to_advance_epoch feature
            // flag is enabled.
            if config.pass_validator_scores_to_advance_epoch() {
                txns.push(EndOfEpochTransactionKind::new_change_epoch_v4(
                    next_epoch,
                    next_epoch_protocol_version.as_u64(),
                    gas_cost_summary.storage_cost,
                    gas_cost_summary.computation_cost,
                    gas_cost_summary.computation_cost_burned,
                    gas_cost_summary.storage_rebate,
                    gas_cost_summary.non_refundable_storage_fee,
                    epoch_start_timestamp_ms,
                    next_epoch_system_package_bytes,
                    eligible_active_validators,
                    scores,
                    config.adjust_rewards_by_score(),
                ));
            } else {
                txns.push(EndOfEpochTransactionKind::new_change_epoch_v3(
                    next_epoch,
                    next_epoch_protocol_version.as_u64(),
                    gas_cost_summary.storage_cost,
                    gas_cost_summary.computation_cost,
                    gas_cost_summary.computation_cost_burned,
                    gas_cost_summary.storage_rebate,
                    gas_cost_summary.non_refundable_storage_fee,
                    epoch_start_timestamp_ms,
                    next_epoch_system_package_bytes,
                    eligible_active_validators,
                ));
            }
        } else if config.protocol_defined_base_fee()
            && config.max_committee_members_count_as_option().is_some()
        {
            txns.push(EndOfEpochTransactionKind::new_change_epoch_v2(
                next_epoch,
                next_epoch_protocol_version.as_u64(),
                gas_cost_summary.storage_cost,
                gas_cost_summary.computation_cost,
                gas_cost_summary.computation_cost_burned,
                gas_cost_summary.storage_rebate,
                gas_cost_summary.non_refundable_storage_fee,
                epoch_start_timestamp_ms,
                next_epoch_system_package_bytes,
            ));
        } else {
            txns.push(EndOfEpochTransactionKind::new_change_epoch(
                next_epoch,
                next_epoch_protocol_version.as_u64(),
                gas_cost_summary.storage_cost,
                gas_cost_summary.computation_cost,
                gas_cost_summary.storage_rebate,
                gas_cost_summary.non_refundable_storage_fee,
                epoch_start_timestamp_ms,
                next_epoch_system_package_bytes,
            ));
        }

        let tx = VerifiedTransaction::new_end_of_epoch_transaction(txns);

        let executable_tx = VerifiedExecutableTransaction::new_from_checkpoint(
            tx.clone(),
            epoch_store.epoch(),
            checkpoint,
        );

        let tx_digest = executable_tx.digest();

        info!(
            ?next_epoch,
            ?next_epoch_protocol_version,
            ?next_epoch_system_packages,
            computation_cost=?gas_cost_summary.computation_cost,
            computation_cost_burned=?gas_cost_summary.computation_cost_burned,
            storage_cost=?gas_cost_summary.storage_cost,
            storage_rebate=?gas_cost_summary.storage_rebate,
            non_refundable_storage_fee=?gas_cost_summary.non_refundable_storage_fee,
            ?tx_digest,
            "Creating advance epoch transaction"
        );

        fail_point_async!("change_epoch_tx_delay");
        let tx_lock = epoch_store.acquire_tx_lock(tx_digest);

        // The tx could have been executed by state sync already - if so simply return
        // an error. The checkpoint builder will shortly be terminated by
        // reconfiguration anyway.
        if self
            .get_transaction_cache_reader()
            .try_is_tx_already_executed(tx_digest)?
        {
            warn!("change epoch tx has already been executed via state sync");
            return Err(CheckpointBuilderError::ChangeEpochTxAlreadyExecuted);
        }

        let execution_guard = self.execution_lock_for_executable_transaction(&executable_tx)?;

        // We must manually assign the shared object versions to the transaction before
        // executing it. This is because we do not sequence end-of-epoch
        // transactions through consensus.
        let assigned_versions = epoch_store.assign_shared_object_versions_idempotent(
            self.get_object_cache_reader().as_ref(),
            std::iter::once(&Schedulable::Transaction(&executable_tx)),
        )?;

        assert_eq!(assigned_versions.0.len(), 1);
        let assigned_versions = assigned_versions.0.into_iter().next().unwrap().1;

        let (input_objects, _) = self.read_objects_for_execution(
            &tx_lock,
            &executable_tx,
            assigned_versions,
            epoch_store,
        )?;

        let (temporary_store, effects, _execution_error_opt) = self.execute_transaction(
            &execution_guard,
            &executable_tx,
            input_objects,
            vec![],
            epoch_store,
        )?;
        let system_obj = get_iota_system_state(&temporary_store.written)
            .expect("change epoch tx must write to system object");
        // Find the SystemEpochInfoEvent emitted by the advance_epoch transaction.
        let system_epoch_info_event = temporary_store
            .events
            .0
            .into_iter()
            .find(|event| event.is_system_epoch_info_event())
            .map(SystemEpochInfoEvent::from);
        // The system epoch info event can be `None` in case if the `advance_epoch`
        // Move function call failed and was executed in the safe mode.
        assert!(system_epoch_info_event.is_some() || system_obj.safe_mode());

        // We must write tx and effects to the state sync tables so that state sync is
        // able to deliver to the transaction to CheckpointExecutor after it is
        // included in a certified checkpoint.
        self.get_state_sync_store()
            .try_insert_transaction_and_effects(&tx, &effects)?;

        info!(
            "Effects summary of the change epoch transaction: {:?}",
            effects.summary_for_debug()
        );
        epoch_store.record_checkpoint_builder_is_safe_mode_metric(system_obj.safe_mode());
        // The change epoch transaction cannot fail to execute.
        assert!(effects.status().is_success());
        Ok((system_obj, system_epoch_info_event, effects))
    }

    /// This function is called at the very end of the epoch.
    /// This step is required before updating new epoch in the db and calling
    /// reopen_epoch_db.
    #[instrument(level = "error", skip_all)]
    async fn revert_uncommitted_epoch_transactions(
        &self,
        epoch_store: &AuthorityPerEpochStore,
    ) -> IotaResult {
        {
            let state = epoch_store.get_reconfig_state_write_lock_guard();
            if state.should_accept_user_certs() {
                // Need to change this so that consensus adapter do not accept certificates from
                // user. This can happen if our local validator did not initiate
                // epoch change locally, but 2f+1 nodes already concluded the
                // epoch.
                //
                // This lock is essentially a barrier (in the certificate mode only) for
                // `epoch_store.pending_consensus_certificates` table we are reading on the line
                // after this block
                epoch_store.close_user_certs(state);
            }
            // lock is dropped here
        }

        // In the P-COOL flow, the list of pending consensus certificates is
        // always empty, so the reverting below is only for the certificate mode.
        if !epoch_store.protocol_config().enable_pcool_flow() {
            let pending_certificates = epoch_store.pending_consensus_certificates();
            info!(
                "Reverting {} locally executed transactions that was not included in the epoch: \
                    {:?}",
                pending_certificates.len(),
                pending_certificates,
            );
            for digest in pending_certificates {
                if epoch_store.is_transaction_executed_in_checkpoint(&digest)? {
                    info!(
                        "Not reverting pending consensus transaction {:?} - it was included in \
                            checkpoint",
                        digest
                    );
                    continue;
                }
                info!("Reverting {:?} at the end of epoch", digest);
                epoch_store.revert_executed_transaction(&digest)?;
                self.get_reconfig_api().try_revert_state_update(&digest)?;
            }
            info!("All uncommitted local transactions reverted");
        } else {
            info!("P-COOL mode: skipping revert of uncommitted epoch transactions");
        }

        Ok(())
    }

    #[instrument(level = "error", skip_all)]
    async fn reopen_epoch_db(
        &self,
        cur_epoch_store: &AuthorityPerEpochStore,
        new_committee: Committee,
        epoch_start_configuration: EpochStartConfiguration,
        expensive_safety_check_config: &ExpensiveSafetyCheckConfig,
        epoch_last_checkpoint: CheckpointSequenceNumber,
    ) -> IotaResult<Arc<AuthorityPerEpochStore>> {
        let new_epoch = new_committee.epoch;
        info!(new_epoch = ?new_epoch, "re-opening AuthorityEpochTables for new epoch");
        assert_eq!(
            epoch_start_configuration.epoch_start_state().epoch(),
            new_committee.epoch
        );
        fail_point!("before-open-new-epoch-store");
        let new_epoch_store = cur_epoch_store.new_at_next_epoch(
            self.name,
            new_committee,
            epoch_start_configuration,
            self.get_backing_package_store().clone(),
            expensive_safety_check_config,
            epoch_last_checkpoint,
        )?;
        self.epoch_store.store(new_epoch_store.clone());
        Ok(new_epoch_store)
    }

    /// Resolves the account's `AuthenticatorFunctionRef` on the execution path,
    /// where the certificate has already passed validation before consensus.
    ///
    /// A deleted or cancelled account object is not an error here: its version
    /// is returned so execution can proceed and surface the proper effect
    /// (e.g. `InputObjectDeleted` or a shared-object congestion cancellation).
    /// Any other failure is a broken invariant and panics.
    fn check_move_account_for_execution(
        &self,
        auth_account_object_id: ObjectId,
        auth_account_object_seq_number: Option<Version>,
        auth_account_object_digest: Option<ObjectDigest>,
        account_object: ObjectReadResult,
        signer: &Address,
    ) -> AuthenticatorFunctionRefForExecution {
        self.check_move_account(
            auth_account_object_id,
            auth_account_object_seq_number,
            auth_account_object_digest,
            account_object,
            signer,
            true,
        )
        .expect("move account checks cannot fail during execution")
    }

    /// Resolves the account's `AuthenticatorFunctionRef` on the validation
    /// (signing) path, rejecting the transaction when the account object was
    /// deleted or belongs to a cancelled transaction.
    fn check_move_account_for_validation(
        &self,
        auth_account_object_id: ObjectId,
        auth_account_object_seq_number: Option<Version>,
        auth_account_object_digest: Option<ObjectDigest>,
        account_object: ObjectReadResult,
        signer: &Address,
    ) -> IotaResult<AuthenticatorFunctionRefForExecution> {
        self.check_move_account(
            auth_account_object_id,
            auth_account_object_seq_number,
            auth_account_object_digest,
            account_object,
            signer,
            false,
        )
    }

    /// Checks whether `authenticator` unlocks a valid Move account and returns
    /// the account-related `AuthenticatorFunctionRef`. When `is_execution` is
    /// set, a deleted or cancelled account object yields its version instead of
    /// an error, so execution can proceed to the proper effect. Prefer the
    /// `check_move_account_for_execution` / `check_move_account_for_validation`
    /// wrappers over calling this directly.
    fn check_move_account(
        &self,
        auth_account_object_id: ObjectId,
        auth_account_object_seq_number: Option<Version>,
        auth_account_object_digest: Option<ObjectDigest>,
        account_object: ObjectReadResult,
        signer: &Address,
        is_execution: bool,
    ) -> IotaResult<AuthenticatorFunctionRefForExecution> {
        let auth_account_object_seq_number = match (&account_object.object, is_execution) {
            // In any case, if the account object is loaded, we can check its version and digest.
            // Then we return the version of the account object to be used for reading the
            // authenticator function ref dynamic field.
            (ObjectReadResultKind::Object(object), _) => {
                let account_object_addr = Address::from(auth_account_object_id);
                fp_ensure!(
                    signer == &account_object_addr,
                    UserInputError::IncorrectUserSignature {
                        error: format!("Move authenticator is trying to unlock {account_object_addr:?}, but given signer address is {signer:?}")
                    }
                    .into()
                );

                fp_ensure!(
                    object.is_shared() || object.is_immutable(),
                    UserInputError::AccountObjectNotSupported {
                        object_id: auth_account_object_id
                    }
                    .into()
                );

                let auth_account_object_seq_number =
                    if let Some(auth_account_object_seq_number) = auth_account_object_seq_number {
                        let account_object_version = object.version();

                        fp_ensure!(
                            account_object_version == auth_account_object_seq_number,
                            UserInputError::AccountObjectVersionMismatch {
                                object_id: auth_account_object_id,
                                expected_version: auth_account_object_seq_number,
                                actual_version: account_object_version,
                            }
                            .into()
                        );

                        auth_account_object_seq_number
                    } else {
                        object.version()
                    };

                if let Some(auth_account_object_digest) = auth_account_object_digest {
                    let expected_digest = object.digest();
                    fp_ensure!(
                        expected_digest == auth_account_object_digest,
                        UserInputError::InvalidAccountObjectDigest {
                            object_id: auth_account_object_id,
                            expected_digest,
                            actual_digest: auth_account_object_digest,
                        }
                        .into()
                    );
                }

                Ok(auth_account_object_seq_number)
            }
            // If the account object is not loaded because it was deleted, we return the error in
            // the case in which we are not executing the transaction right after.
            (ObjectReadResultKind::DeletedSharedObject(version, digest), false) => {
                Err(UserInputError::AccountObjectDeleted {
                    account_id: account_object.id(),
                    account_version: *version,
                    transaction_digest: *digest,
                })
            }
            // If the account object is not loaded because the transaction was canceled, we return
            // the error in the case in which we are not executing the transaction right
            // after.
            (ObjectReadResultKind::CancelledTransactionObject(version), false) => {
                Err(UserInputError::AccountObjectInCanceledTransaction {
                    account_id: account_object.id(),
                    account_version: *version,
                })
            }
            // If the account object is not loaded because it was deleted, we return the version in
            // the case in which we are executing the transaction right after.
            // This version is used to read the authenticator function ref dynamic field because it
            // is greater than the version of the child dynamic field.
            (ObjectReadResultKind::DeletedSharedObject(version, _), true) => Ok(*version),
            // If the account object is not loaded because the transaction was canceled, we return
            // the version in the case in which we are executing the transaction right
            // after. This version is used to read the authenticator function ref
            // dynamic field because it is greater than the version of the child dynamic
            // field.
            (ObjectReadResultKind::CancelledTransactionObject(version), true) => Ok(*version),
        }?;

        let authenticator_function_ref_field_id =
            derive_authenticator_function_ref_v1_dynamic_field_id(auth_account_object_id)?;

        let authenticator_function_ref_field = self
            .get_object_cache_reader()
            .try_find_object_lt_or_eq_version(
                authenticator_function_ref_field_id,
                auth_account_object_seq_number,
            )?;

        if let Some(authenticator_function_ref_field_obj) = authenticator_function_ref_field {
            Ok(authenticator_function_ref_v1_from_dynamic_field_object(
                auth_account_object_id,
                &authenticator_function_ref_field_obj,
            )?)
        } else {
            Err(UserInputError::MoveAuthenticatorNotFound {
                authenticator_function_ref_id: authenticator_function_ref_field_id,
                account_object_id: auth_account_object_id,
                account_object_version: auth_account_object_seq_number,
            }
            .into())
        }
    }

    #[allow(clippy::type_complexity)]
    fn read_objects_for_validation(
        &self,
        transaction: &VerifiedTransaction,
        epoch: u64,
    ) -> IotaResult<(
        InputObjects,
        ReceivingObjects,
        Vec<(InputObjects, ObjectReadResult)>,
    )> {
        let (input_objects, tx_receiving_objects) = self.input_loader.read_objects_for_signing(
            Some(transaction.digest()),
            &transaction.collect_all_input_object_kind_for_reading()?,
            &transaction.data().transaction().receiving_objects(),
            epoch,
        )?;

        transaction
            .split_input_objects_into_groups_for_reading(input_objects)
            .map(|(tx_input_objects, per_authenticator_inputs)| {
                (
                    tx_input_objects,
                    tx_receiving_objects,
                    per_authenticator_inputs,
                )
            })
    }

    #[allow(clippy::type_complexity)]
    fn check_transaction_inputs_for_validation(
        &self,
        protocol_config: &ProtocolConfig,
        reference_gas_price: u64,
        tx: &Transaction,
        tx_input_objects: InputObjects,
        tx_receiving_objects: &ReceivingObjects,
        move_authenticators: &Vec<&MoveAuthenticator>,
        per_authenticator_inputs: Vec<(InputObjects, ObjectReadResult)>,
    ) -> IotaResult<(
        IotaGasStatus,
        CheckedInputObjects,
        Vec<(CheckedInputObjects, AuthenticatorFunctionRef)>,
    )> {
        let authenticator_gas_budget = if move_authenticators.is_empty() {
            0
        } else {
            // `max_auth_gas` is used here as a Move authenticator gas budget until it is
            // not a part of the transaction data.
            protocol_config.max_auth_gas()
        };

        debug_assert_eq!(
            move_authenticators.len(),
            per_authenticator_inputs.len(),
            "Move authenticators amount must match the number of authenticator inputs"
        );

        let per_authenticator_checked_inputs = move_authenticators
            .iter()
            .zip(per_authenticator_inputs)
            .map(
                |(move_authenticator, (authenticator_input_objects, account_object))| {
                    // Check basic `object_to_authenticate` preconditions and get its components.
                    let (
                        auth_account_object_id,
                        auth_account_object_seq_number,
                        auth_account_object_digest,
                    ) = move_authenticator.object_to_authenticate_components()?;

                    let signer = move_authenticator.address();

                    // Make sure the signer is a Move account.
                    let AuthenticatorFunctionRefForExecution {
                        authenticator_function_ref,
                        ..
                    } = self.check_move_account_for_validation(
                        auth_account_object_id,
                        auth_account_object_seq_number,
                        auth_account_object_digest,
                        account_object,
                        &signer,
                    )?;

                    // Check the MoveAuthenticator input objects.
                    let authenticator_checked_input_objects =
                        iota_transaction_checks::check_move_authenticator_input_for_validation(
                            authenticator_input_objects,
                        )?;

                    Ok((
                        authenticator_checked_input_objects,
                        authenticator_function_ref,
                    ))
                },
            )
            .collect::<IotaResult<Vec<_>>>()?;

        // Check the transaction inputs.
        let (gas_status, tx_checked_input_objects) =
            iota_transaction_checks::check_transaction_input(
                protocol_config,
                reference_gas_price,
                tx,
                tx_input_objects,
                tx_receiving_objects,
                &self.metrics.bytecode_verifier_metrics,
                &self.config.verifier_signing_config,
                authenticator_gas_budget,
            )?;

        Ok((
            gas_status,
            tx_checked_input_objects,
            per_authenticator_checked_inputs,
        ))
    }

    #[cfg(test)]
    pub(crate) fn iter_live_object_set_for_testing(
        &self,
    ) -> impl Iterator<Item = authority_store_tables::LiveObject> + '_ {
        self.get_global_state_hash_store()
            .iter_cached_live_object_set_for_testing()
    }

    #[cfg(test)]
    pub(crate) fn shutdown_execution_for_test(&self) {
        self.tx_execution_shutdown
            .lock()
            .take()
            .unwrap()
            .send(())
            .unwrap();
    }

    /// NOTE: this function is only to be used for fuzzing and testing. Never
    /// use in prod
    pub async fn insert_objects_unsafe_for_testing_only(&self, objects: &[Object]) {
        self.get_reconfig_api().bulk_insert_genesis_objects(objects);
        self.get_object_cache_reader()
            .force_reload_system_packages(&BuiltInFramework::all_package_ids());
        self.get_reconfig_api()
            .clear_state_end_of_epoch(&self.execution_lock_for_reconfiguration().await);
    }
}

pub struct RandomnessRoundReceiver {
    authority_state: Arc<AuthorityState>,
    randomness_rx: mpsc::Receiver<(EpochId, RandomnessRound, Vec<u8>)>,
}

impl RandomnessRoundReceiver {
    pub fn spawn(
        authority_state: Arc<AuthorityState>,
        randomness_rx: mpsc::Receiver<(EpochId, RandomnessRound, Vec<u8>)>,
    ) -> JoinHandle<()> {
        let rrr = RandomnessRoundReceiver {
            authority_state,
            randomness_rx,
        };
        spawn_monitored_task!(rrr.run())
    }

    async fn run(mut self) {
        info!("RandomnessRoundReceiver event loop started");

        loop {
            tokio::select! {
                maybe_recv = self.randomness_rx.recv() => {
                    if let Some((epoch, round, bytes)) = maybe_recv {
                        self.handle_new_randomness(epoch, round, bytes).await;
                    } else {
                        break;
                    }
                },
            }
        }

        info!("RandomnessRoundReceiver event loop ended");
    }

    #[instrument(level = "debug", skip_all, fields(?epoch, ?round))]
    async fn handle_new_randomness(&self, epoch: EpochId, round: RandomnessRound, bytes: Vec<u8>) {
        fail_point_async!("randomness-delay");

        let epoch_store = self.authority_state.load_epoch_store_one_call_per_task();
        if epoch_store.epoch() != epoch {
            warn!(
                "dropping randomness for epoch {epoch}, round {round}, because we are in epoch {}",
                epoch_store.epoch()
            );
            return;
        }
        let key = TransactionKey::RandomnessRound(epoch, round);
        let transaction = VerifiedTransaction::new_randomness_state_update(
            epoch,
            round,
            bytes,
            epoch_store
                .epoch_start_config()
                .randomness_obj_initial_shared_version(),
        );
        debug!(
            "created randomness state update transaction with digest: {:?}",
            transaction.digest()
        );
        let transaction = VerifiedExecutableTransaction::new_system(transaction, epoch);
        let digest = *transaction.digest();

        // Randomness state updates contain the full bls signature for the random round,
        // which cannot necessarily be reconstructed again later. Therefore we must
        // immediately persist this transaction. If we crash before its outputs
        // are committed, this ensures we will be able to re-execute it.
        self.authority_state
            .get_cache_commit()
            .persist_transaction(&transaction);

        // Notify the scheduler that the transaction key now has a known digest
        if epoch_store.insert_tx_key(key, digest).is_err() {
            warn!("epoch ended while handling new randomness");
        }

        // TODO: delete this when transaction manager is deleted
        match self.authority_state.execution_scheduler().as_ref() {
            ExecutionSchedulerWrapper::ExecutionScheduler(_) => {}
            ExecutionSchedulerWrapper::TransactionManager(manager) => {
                // Notifies transaction manager about transaction and output objects
                // committed. This provides necessary information to transaction manager
                // to start executing additional ready transactions.
                manager.notify_transaction_key(&epoch_store, key, digest);
            }
        }

        let authority_state = self.authority_state.clone();
        spawn_monitored_task!(async move {
            // Wait for transaction execution in a separate task, to avoid deadlock in case
            // of out-of-order randomness generation. (Each
            // RandomnessStateUpdate depends on the output of the
            // RandomnessStateUpdate from the previous round.)
            //
            // We set a very long timeout so that in case this gets stuck for some reason,
            // the validator will eventually crash rather than continuing in a
            // zombie mode.
            const RANDOMNESS_STATE_UPDATE_EXECUTION_TIMEOUT: Duration = Duration::from_secs(300);
            let result = tokio::time::timeout(
                RANDOMNESS_STATE_UPDATE_EXECUTION_TIMEOUT,
                authority_state
                    .get_transaction_cache_reader()
                    .try_notify_read_executed_effects(
                        "RandomnessRoundReceiver::notify_read_executed_effects_first",
                        &[digest],
                    ),
            )
            .await;
            let result = match result {
                Ok(result) => result,
                Err(_) => {
                    if cfg!(debug_assertions) {
                        // Crash on randomness update execution timeout in debug builds.
                        panic!(
                            "randomness state update transaction execution timed out at epoch {epoch}, round {round}"
                        );
                    }
                    warn!(
                        "randomness state update transaction execution timed out at epoch {epoch}, round {round}"
                    );
                    // Continue waiting as long as necessary in non-debug builds.
                    authority_state
                        .get_transaction_cache_reader()
                        .try_notify_read_executed_effects(
                            "RandomnessRoundReceiver::notify_read_executed_effects_second",
                            &[digest],
                        )
                        .await
                }
            };

            let mut effects = result.unwrap_or_else(|_| panic!("failed to get effects for randomness state update transaction at epoch {epoch}, round {round}"));
            let effects = effects.pop().expect("should return effects");
            if *effects.status() != ExecutionStatus::Success {
                fatal!(
                    "failed to execute randomness state update transaction at epoch {epoch}, round {round}: {effects:?}"
                );
            }
            debug!(
                "successfully executed randomness state update transaction at epoch {epoch}, round {round}"
            );
        });
    }
}

#[async_trait]
impl TransactionKeyValueStoreTrait for AuthorityState {
    async fn multi_get(
        &self,
        transaction_keys: &[TransactionDigest],
        effects_keys: &[TransactionDigest],
    ) -> IotaResult<KVStoreTransactionData> {
        let txns = if !transaction_keys.is_empty() {
            self.get_transaction_cache_reader()
                .try_multi_get_transaction_blocks(transaction_keys)?
                .into_iter()
                .map(|t| t.map(|t| (*t).clone().into_inner()))
                .collect()
        } else {
            vec![]
        };

        let fx = if !effects_keys.is_empty() {
            self.get_transaction_cache_reader()
                .try_multi_get_executed_effects(effects_keys)?
        } else {
            vec![]
        };

        Ok((txns, fx))
    }

    async fn multi_get_checkpoints(
        &self,
        checkpoint_summaries: &[CheckpointSequenceNumber],
        checkpoint_contents: &[CheckpointSequenceNumber],
        checkpoint_summaries_by_digest: &[CheckpointDigest],
    ) -> IotaResult<(
        Vec<Option<CertifiedCheckpointSummary>>,
        Vec<Option<CheckpointContents>>,
        Vec<Option<CertifiedCheckpointSummary>>,
    )> {
        // TODO: use multi-get methods if it ever becomes important (unlikely)
        let mut summaries = Vec::with_capacity(checkpoint_summaries.len());
        let store = self.get_checkpoint_store();
        for seq in checkpoint_summaries {
            let checkpoint = store
                .get_checkpoint_by_sequence_number(*seq)?
                .map(|c| c.into_inner());

            summaries.push(checkpoint);
        }

        let mut contents = Vec::with_capacity(checkpoint_contents.len());
        for seq in checkpoint_contents {
            let checkpoint = store
                .get_checkpoint_by_sequence_number(*seq)?
                .and_then(|summary| {
                    store
                        .get_checkpoint_contents(&summary.contents_digest)
                        .expect("db read cannot fail")
                });
            contents.push(checkpoint);
        }

        let mut summaries_by_digest = Vec::with_capacity(checkpoint_summaries_by_digest.len());
        for digest in checkpoint_summaries_by_digest {
            let checkpoint = store
                .get_checkpoint_by_digest(digest)?
                .map(|c| c.into_inner());
            summaries_by_digest.push(checkpoint);
        }

        Ok((summaries, contents, summaries_by_digest))
    }

    async fn get_transaction_perpetual_checkpoint(
        &self,
        digest: TransactionDigest,
    ) -> IotaResult<Option<CheckpointSequenceNumber>> {
        self.get_checkpoint_cache()
            .try_get_transaction_perpetual_checkpoint(&digest)
            .map(|res| res.map(|(_epoch, checkpoint)| checkpoint))
    }

    async fn get_object(
        &self,
        object_id: ObjectId,
        version: VersionNumber,
    ) -> IotaResult<Option<Object>> {
        self.get_object_cache_reader()
            .try_get_object_by_key(&object_id, version)
    }

    #[instrument(skip_all)]
    async fn multi_get_objects(
        &self,
        object_keys: &[ObjectKey],
    ) -> IotaResult<Vec<Option<Object>>> {
        Ok(self
            .get_object_cache_reader()
            .multi_get_objects_by_key(object_keys))
    }

    async fn multi_get_transactions_perpetual_checkpoints(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<CheckpointSequenceNumber>>> {
        let res = self
            .get_checkpoint_cache()
            .try_multi_get_transactions_perpetual_checkpoints(digests)?;

        Ok(res
            .into_iter()
            .map(|maybe| maybe.map(|(_epoch, checkpoint)| checkpoint))
            .collect())
    }

    #[instrument(skip(self, digests), fields(digests = digests.iter().map(|d| d.to_string()).collect::<Vec<String>>().join(", ")))]
    async fn multi_get_events_by_tx_digests(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEvents>>> {
        if digests.is_empty() {
            return Ok(vec![]);
        }

        Ok(self
            .get_transaction_cache_reader()
            .multi_get_events(digests))
    }
}

#[cfg(msim)]
pub mod framework_injection {
    use std::{
        cell::RefCell,
        collections::{BTreeMap, BTreeSet},
    };

    use iota_framework::{BuiltInFramework, SystemPackage};
    use iota_sdk_types::ObjectId;
    use iota_types::base_types::AuthorityName;
    use move_binary_format::CompiledModule;

    type FrameworkOverrideConfig = BTreeMap<ObjectId, PackageOverrideConfig>;

    // Thread local cache because all simtests run in a single unique thread.
    thread_local! {
        static OVERRIDE: RefCell<FrameworkOverrideConfig> = RefCell::new(FrameworkOverrideConfig::default());
    }

    type Framework = Vec<CompiledModule>;

    pub type PackageUpgradeCallback =
        Box<dyn Fn(AuthorityName) -> Option<Framework> + Send + Sync + 'static>;

    enum PackageOverrideConfig {
        Global(Framework),
        PerValidator(PackageUpgradeCallback),
    }

    fn compiled_modules_to_bytes(modules: &[CompiledModule]) -> Vec<Vec<u8>> {
        modules
            .iter()
            .map(|m| {
                let mut buf = Vec::new();
                m.serialize_with_version(m.version, &mut buf).unwrap();
                buf
            })
            .collect()
    }

    pub fn set_override(package_id: ObjectId, modules: Vec<CompiledModule>) {
        OVERRIDE.with(|bs| {
            bs.borrow_mut()
                .insert(package_id, PackageOverrideConfig::Global(modules))
        });
    }

    pub fn set_override_cb(package_id: ObjectId, func: PackageUpgradeCallback) {
        OVERRIDE.with(|bs| {
            bs.borrow_mut()
                .insert(package_id, PackageOverrideConfig::PerValidator(func))
        });
    }

    pub fn get_override_bytes(package_id: &ObjectId, name: AuthorityName) -> Option<Vec<Vec<u8>>> {
        OVERRIDE.with(|cfg| {
            cfg.borrow().get(package_id).and_then(|entry| match entry {
                PackageOverrideConfig::Global(framework) => {
                    Some(compiled_modules_to_bytes(framework))
                }
                PackageOverrideConfig::PerValidator(func) => {
                    func(name).map(|fw| compiled_modules_to_bytes(&fw))
                }
            })
        })
    }

    pub fn get_override_modules(
        package_id: &ObjectId,
        name: AuthorityName,
    ) -> Option<Vec<CompiledModule>> {
        OVERRIDE.with(|cfg| {
            cfg.borrow().get(package_id).and_then(|entry| match entry {
                PackageOverrideConfig::Global(framework) => Some(framework.clone()),
                PackageOverrideConfig::PerValidator(func) => func(name),
            })
        })
    }

    pub fn get_override_system_package(
        package_id: &ObjectId,
        name: AuthorityName,
    ) -> Option<SystemPackage> {
        let bytes = get_override_bytes(package_id, name)?;
        let dependencies = if package_id.is_system_package() {
            BuiltInFramework::get_package_by_id(package_id)
                .dependencies
                .to_vec()
        } else {
            // Assume that entirely new injected packages depend on all existing system
            // packages.
            BuiltInFramework::all_package_ids()
        };
        Some(SystemPackage {
            id: *package_id,
            bytes,
            dependencies,
        })
    }

    pub fn get_extra_packages(name: AuthorityName) -> Vec<SystemPackage> {
        let built_in = BTreeSet::from_iter(BuiltInFramework::all_package_ids());
        let extra: Vec<ObjectId> = OVERRIDE.with(|cfg| {
            cfg.borrow()
                .keys()
                .filter_map(|package| (!built_in.contains(package)).then_some(*package))
                .collect()
        });

        extra
            .into_iter()
            .map(|package| SystemPackage {
                id: package,
                bytes: get_override_bytes(&package, name).unwrap(),
                dependencies: BuiltInFramework::all_package_ids(),
            })
            .collect()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ObjDumpFormat {
    pub id: ObjectId,
    pub version: VersionNumber,
    pub digest: ObjectDigest,
    pub object: Object,
}

impl ObjDumpFormat {
    fn new(object: Object) -> Self {
        let oref = object.object_ref();
        Self {
            id: oref.object_id,
            version: oref.version,
            digest: oref.digest,
            object,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeStateDump {
    pub tx_digest: TransactionDigest,
    pub sender_signed_data: SenderSignedTransaction,
    pub executed_epoch: u64,
    pub reference_gas_price: u64,
    pub protocol_version: u64,
    pub epoch_start_timestamp_ms: u64,
    pub computed_effects: TransactionEffects,
    pub expected_effects_digest: TransactionEffectsDigest,
    pub relevant_system_packages: Vec<ObjDumpFormat>,
    pub shared_objects: Vec<ObjDumpFormat>,
    pub loaded_child_objects: Vec<ObjDumpFormat>,
    pub modified_at_versions: Vec<ObjDumpFormat>,
    pub runtime_reads: Vec<ObjDumpFormat>,
    pub input_objects: Vec<ObjDumpFormat>,
}

impl NodeStateDump {
    pub fn new(
        tx_digest: &TransactionDigest,
        effects: &TransactionEffects,
        expected_effects_digest: TransactionEffectsDigest,
        object_store: &dyn ObjectStore,
        epoch_store: &Arc<AuthorityPerEpochStore>,
        inner_temporary_store: &InnerTemporaryStore,
        transaction: &VerifiedExecutableTransaction,
    ) -> IotaResult<Self> {
        // Epoch info
        let executed_epoch = epoch_store.epoch();
        let reference_gas_price = epoch_store.reference_gas_price();
        let epoch_start_config = epoch_store.epoch_start_config();
        let protocol_version = epoch_store.protocol_version().as_u64();
        let epoch_start_timestamp_ms = epoch_start_config.epoch_data().epoch_start_timestamp();

        // Record all system packages at this version
        let mut relevant_system_packages = Vec::new();
        for sys_package_id in BuiltInFramework::all_package_ids() {
            if let Some(w) = object_store.try_get_object(&sys_package_id)? {
                relevant_system_packages.push(ObjDumpFormat::new(w))
            }
        }

        // Record all the shared objects
        let mut shared_objects = Vec::new();
        for kind in effects.input_shared_objects() {
            match kind {
                InputSharedObject::Mutate(obj_ref) | InputSharedObject::ReadOnly(obj_ref) => {
                    if let Some(w) =
                        object_store.try_get_object_by_key(&obj_ref.object_id, obj_ref.version)?
                    {
                        shared_objects.push(ObjDumpFormat::new(w))
                    }
                }
                InputSharedObject::ReadDeleted(..)
                | InputSharedObject::MutateDeleted(..)
                | InputSharedObject::Canceled(..) => (), /* TODO: consider record congested
                                                          * objects. */
            }
        }

        // Record all loaded child objects
        // Child objects which are read but not mutated are not tracked anywhere else
        let mut loaded_child_objects = Vec::new();
        for (id, meta) in &inner_temporary_store.loaded_runtime_objects {
            if let Some(w) = object_store.try_get_object_by_key(id, meta.version)? {
                loaded_child_objects.push(ObjDumpFormat::new(w))
            }
        }

        // Record all modified objects
        let mut modified_at_versions = Vec::new();
        for modified in effects.modified_at_versions() {
            let (id, ver) = (modified.object_id, modified.version);
            if let Some(w) = object_store.try_get_object_by_key(&id, ver)? {
                modified_at_versions.push(ObjDumpFormat::new(w))
            }
        }

        // Packages read at runtime, which were not previously loaded into the temoorary
        // store Some packages may be fetched at runtime and wont show up in
        // input objects
        let mut runtime_reads = Vec::new();
        for obj in inner_temporary_store
            .runtime_packages_loaded_from_db
            .values()
        {
            runtime_reads.push(ObjDumpFormat::new(obj.object().clone()));
        }

        // All other input objects should already be in `inner_temporary_store.objects`

        Ok(Self {
            tx_digest: *tx_digest,
            executed_epoch,
            reference_gas_price,
            epoch_start_timestamp_ms,
            protocol_version,
            relevant_system_packages,
            shared_objects,
            loaded_child_objects,
            modified_at_versions,
            runtime_reads,
            sender_signed_data: transaction.clone().into_message(),
            input_objects: inner_temporary_store
                .input_objects
                .values()
                .map(|o| ObjDumpFormat::new(o.clone()))
                .collect(),
            computed_effects: effects.clone(),
            expected_effects_digest,
        })
    }

    pub fn all_objects(&self) -> Vec<ObjDumpFormat> {
        let mut objects = Vec::new();
        objects.extend(self.relevant_system_packages.clone());
        objects.extend(self.shared_objects.clone());
        objects.extend(self.loaded_child_objects.clone());
        objects.extend(self.modified_at_versions.clone());
        objects.extend(self.runtime_reads.clone());
        objects.extend(self.input_objects.clone());
        objects
    }

    pub fn write_to_file(&self, path: &Path) -> Result<PathBuf, anyhow::Error> {
        let file_name = format!(
            "{}_{}_NODE_DUMP.json",
            self.tx_digest,
            AuthorityState::unixtime_now_ms()
        );
        let mut path = path.to_path_buf();
        path.push(&file_name);
        let mut file = File::create(path.clone())?;
        file.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
        Ok(path)
    }

    pub fn read_from_file(path: &PathBuf) -> Result<Self, anyhow::Error> {
        let file = File::open(path)?;
        serde_json::from_reader(file).map_err(|e| anyhow::anyhow!(e))
    }
}

/// Returns the [`MoveAuthenticator`]s to execute during the pre-consensus
/// phase.
///
/// When `pre_consensus_sponsor_only_move_authentication` is enabled:
/// - For sponsored transactions: only the sponsor's [`MoveAuthenticator`] is
///   returned (empty if the sponsor does not use one).
/// - For non-sponsored transactions: all [`MoveAuthenticator`]s are returned
///   (currently only the sender's).
///
/// When the flag is not set, all [`MoveAuthenticator`]s are returned for
/// compatibility.
fn pre_consensus_move_authenticators<'a>(
    tx: &'a VerifiedTransaction,
    protocol_config: &ProtocolConfig,
) -> Vec<&'a MoveAuthenticator> {
    if protocol_config.pre_consensus_sponsor_only_move_authentication() {
        if tx.transaction().is_sponsored_tx() {
            if let Some(sponsor_move_authenticator) = tx.sponsor_move_authenticator() {
                vec![sponsor_move_authenticator]
            } else {
                vec![]
            }
        } else {
            tx.move_authenticators()
        }
    } else {
        tx.move_authenticators()
    }
}
