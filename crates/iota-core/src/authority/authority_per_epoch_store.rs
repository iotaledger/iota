// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
};

use arc_swap::ArcSwapOption;
use enum_dispatch::enum_dispatch;
use fastcrypto::{groups::bls12381, traits::ToFromBytes};
use fastcrypto_tbls::{dkg_v1, nodes::PartyId};
use fastcrypto_zkp::bn254::{
    zk_login::{JWK, JwkId},
    zk_login_api::ZkLoginEnv,
};
use futures::{
    FutureExt, StreamExt,
    future::{Either, join_all, select},
    stream::FuturesUnordered,
};
use iota_common::{
    fatal,
    sync::{notify_once::NotifyOnce, notify_read::NotifyRead},
};
use iota_config::node::ExpensiveSafetyCheckConfig;
use iota_execution::{self, Executor};
use iota_macros::{fail_point, fail_point_arg};
use iota_metrics::monitored_scope;
use iota_protocol_config::{
    Chain, PerObjectCongestionControlMode, ProtocolConfig, ProtocolVersion,
};
use iota_storage::mutex_table::{MutexGuard, MutexTable};
use iota_types::{
    accumulator::Accumulator,
    authenticator_state::{ActiveJwk, get_authenticator_state},
    base_types::{
        AuthorityName, CommitRound, ConciseableName, EpochId, ObjectID, ObjectRef, SequenceNumber,
        TransactionDigest,
    },
    committee::{Committee, CommitteeTrait},
    crypto::{AuthoritySignInfo, AuthorityStrongQuorumSignInfo, RandomnessRound},
    digests::{ChainIdentifier, TransactionEffectsDigest},
    effects::TransactionEffects,
    error::{IotaError, IotaResult},
    executable_transaction::VerifiedExecutableTransaction,
    iota_system_state::epoch_start_iota_system_state::{
        EpochStartSystemState, EpochStartSystemStateTrait,
    },
    message_envelope::TrustedEnvelope,
    messages_checkpoint::{
        CheckpointContents, CheckpointSequenceNumber, CheckpointSignatureMessage, CheckpointSummary,
    },
    messages_consensus::{
        AuthorityCapabilitiesV1, ConsensusTransaction, ConsensusTransactionKey,
        ConsensusTransactionKind, SignedAuthorityCapabilitiesV1, TimestampMs,
        VerifiedAuthorityCapabilitiesV1, VersionedDkgConfirmation, check_total_jwk_size,
    },
    signature::GenericSignature,
    storage::{BackingPackageStore, InputKey, ObjectStore},
    transaction::{
        AuthenticatorStateUpdateV1, CertifiedTransaction, InputObjectKind, SenderSignedData,
        Transaction, TransactionDataAPI, TransactionKey, TransactionKind, VerifiedCertificate,
        VerifiedSignedTransaction, VerifiedTransaction,
    },
};
use itertools::{Itertools, izip};
use move_bytecode_utils::module_cache::SyncModuleCache;
use nonempty::NonEmpty;
use parking_lot::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use prometheus::IntCounter;
use serde::{Deserialize, Serialize};
use tap::TapOptional;
use tokio::{sync::OnceCell, time::Instant};
use tracing::{debug, error, info, instrument, trace, warn};
use typed_store::{
    DBMapUtils, Map,
    rocks::{
        DBBatch, DBMap, DBOptions, MetricConf, ReadWriteOptions, default_db_options,
        read_size_from_env,
    },
    rocksdb::Options,
};

use super::{
    authority_store_tables::ENV_VAR_LOCKS_BLOCK_CACHE_SIZE,
    epoch_start_configuration::EpochStartConfigTrait,
    shared_object_congestion_tracker::{
        ExecutionTime, SequencingResult, SharedObjectCongestionTracker,
    },
    transaction_deferral::{DeferralKey, DeferralReason, transaction_deferral_within_limit},
};
use crate::{
    authority::{
        AuthorityMetrics, ResolverWrapper,
        epoch_start_configuration::EpochStartConfiguration,
        shared_object_congestion_tracker::CongestionPerObjectDebt,
        shared_object_version_manager::{
            AssignedTxAndVersions, ConsensusSharedObjVerAssignment, SharedObjVerManager,
        },
        suggested_gas_price_calculator::SuggestedGasPriceCalculator,
    },
    checkpoints::{
        BuilderCheckpointSummary, CheckpointHeight, CheckpointServiceNotify, EpochStats,
        PendingCheckpoint, PendingCheckpointContentsV1, PendingCheckpointInfo,
    },
    consensus_handler::{
        ConsensusCommitInfo, SequencedConsensusTransaction, SequencedConsensusTransactionKey,
        SequencedConsensusTransactionKind, VerifiedSequencedConsensusTransaction,
    },
    epoch::{
        epoch_metrics::EpochMetrics,
        randomness::{
            CommitTimestampMs, DkgStatus, RandomnessManager, RandomnessReporter, SINGLETON_KEY,
            VersionedProcessedMessage, VersionedUsedProcessedMessages,
        },
        reconfiguration::ReconfigState,
    },
    execution_cache::{ObjectCacheRead, TransactionCacheRead, cache_types::CacheResult},
    fallback_fetch::do_fallback_lookup,
    module_cache_metrics::ResolverMetrics,
    post_consensus_tx_reorder::PostConsensusTxReorder,
    signature_verifier::*,
    stake_aggregator::{GenericMultiStakeAggregator, StakeAggregator},
};

/// The key where the latest consensus index is stored in the database.
// TODO: Make a single table (e.g., called `variables`) storing all our lonely
// variables in one place.
const LAST_CONSENSUS_STATS_ADDR: u64 = 0;
const RECONFIG_STATE_INDEX: u64 = 0;
const OVERRIDE_PROTOCOL_UPGRADE_BUFFER_STAKE_INDEX: u64 = 0;
pub const EPOCH_DB_PREFIX: &str = "epoch_";

// Types for randomness DKG.
pub(crate) type PkG = bls12381::G2Element;
pub(crate) type EncG = bls12381::G2Element;

#[path = "consensus_quarantine.rs"]
pub(crate) mod consensus_quarantine;

#[path = "scorer.rs"]
pub(crate) mod scorer;

use consensus_quarantine::{
    ConsensusCommitOutput, ConsensusOutputCache, ConsensusOutputQuarantine,
};
use iota_types::crypto::AuthorityPublicKey;
use scorer::Scorer;

// CertLockGuard and CertTxGuard are functionally identical right now, but we
// retain a distinction anyway. If we need to support distributed object
// storage, having this distinction will be useful, as we will most likely have
// to re-implement a retry / write-ahead-log at that point.
pub struct CertLockGuard(#[expect(unused)] MutexGuard);
pub struct CertTxGuard(CertLockGuard);

impl CertTxGuard {
    pub fn release(self) {}
    pub fn commit_tx(self) {}
    pub fn as_lock_guard(&self) -> &CertLockGuard {
        &self.0
    }
}

impl CertLockGuard {
    pub fn guard_for_tests() -> Self {
        let lock = Arc::new(parking_lot::Mutex::new(()));
        Self(lock.try_lock_arc().unwrap())
    }
}

type JwkAggregator = GenericMultiStakeAggregator<(JwkId, JWK), true>;

/// Container for congestion control parameters commonly used in
/// `SharedObjectCongestionTracker` and `SuggestedGasPriceCalculator`.
/// It can be initialized from a `ProtocolConfig` instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CongestionControlParameters {
    /// Controls the behavior of per-object congestion control. This
    /// field determines how the estimated execution duration of a
    /// transaction is calculated.
    per_object_congestion_control_mode: PerObjectCongestionControlMode,

    /// This field determines how the start time of a transaction should be
    /// assigned. If `true`, the tracker will assign the start time according
    /// to the minimum free execution slot for a transaction over all its
    /// shared objects. If `false`, the tracker will assign the start time
    /// according to the maximum end time of the occupied execution slots
    /// for a transaction over all its shared objects.
    congestion_control_min_free_execution_slot: bool,

    /// Maximum execution duration per shared object per commit. If `None`,
    /// it means that shared-object congestion control is disabled.
    max_execution_duration_per_commit: Option<ExecutionTime>,

    /// Maximum amount that is allowed to overshoot
    /// `max_execution_duration_per_commit`. If `None`, it means that
    /// congestion limit overshoot is disabled.
    max_congestion_limit_overshoot_per_commit: Option<ExecutionTime>,

    /// Maximum gas price that can be set in transactions. This field
    /// is only used in `SuggestedGasPriceCalculator` to prevent
    /// suggesting feedback gas price larger this value.
    max_gas_price: u64,

    /// Whether to use congestion limit overshoot in the gas price feedback
    /// mechanism, i.e., this is only used in `SuggestedGasPriceCalculator`.
    use_congestion_limit_overshoot_in_gas_price_feedback_mechanism: bool,

    /// Whether to use a separate gas price feedback mechanism for transactions
    /// using randomness.
    use_separate_gas_price_feedback_mechanism_for_randomness: bool,
}

impl CongestionControlParameters {
    /// Create a new `CongestionControlParameters` from `ProtocolConfig`.
    fn new(protocol_config: &ProtocolConfig) -> Self {
        Self {
            per_object_congestion_control_mode: protocol_config
                .per_object_congestion_control_mode(),
            congestion_control_min_free_execution_slot: protocol_config
                .congestion_control_min_free_execution_slot(),
            max_execution_duration_per_commit: protocol_config
                .max_accumulated_txn_cost_per_object_in_mysticeti_commit_as_option(),
            max_congestion_limit_overshoot_per_commit: protocol_config
                .max_congestion_limit_overshoot_per_commit_as_option(),
            max_gas_price: protocol_config.max_gas_price(),
            use_congestion_limit_overshoot_in_gas_price_feedback_mechanism: protocol_config
                .congestion_limit_overshoot_in_gas_price_feedback_mechanism(),
            use_separate_gas_price_feedback_mechanism_for_randomness: protocol_config
                .separate_gas_price_feedback_mechanism_for_randomness(),
        }
    }

    /// Create a new `CongestionControlParameters` for testing.
    #[cfg(test)]
    pub(crate) fn new_for_test(
        per_object_congestion_control_mode: PerObjectCongestionControlMode,
        congestion_control_min_free_execution_slot: bool,
        max_execution_duration_per_commit: Option<ExecutionTime>,
        max_congestion_limit_overshoot_per_commit: Option<ExecutionTime>,
        max_gas_price: u64,
        use_congestion_limit_overshoot_in_gas_price_feedback_mechanism: bool,
        use_separate_gas_price_feedback_mechanism_for_randomness: bool,
    ) -> Self {
        Self {
            per_object_congestion_control_mode,
            congestion_control_min_free_execution_slot,
            max_execution_duration_per_commit,
            max_congestion_limit_overshoot_per_commit,
            max_gas_price,
            use_congestion_limit_overshoot_in_gas_price_feedback_mechanism,
            use_separate_gas_price_feedback_mechanism_for_randomness,
        }
    }

    /// Get per-object congestion control mode.
    #[cfg(test)]
    pub(super) fn per_object_congestion_control_mode_for_test(
        &self,
    ) -> PerObjectCongestionControlMode {
        self.per_object_congestion_control_mode
    }

    /// Depending on the `PerObjectCongestionControlMode`, different metrics are
    /// used to approximate the expected execution duration of a transaction.
    /// The expected execution duration is what is used to schedule transactions
    /// and allocate resources based on how many transactions can be executed
    /// from a given consensus commit.
    pub(super) fn get_estimated_execution_duration(
        &self,
        cert: &VerifiedExecutableTransaction,
    ) -> ExecutionTime {
        match self.per_object_congestion_control_mode {
            PerObjectCongestionControlMode::None => 0,
            PerObjectCongestionControlMode::TotalGasBudget => cert.gas_budget(),
            PerObjectCongestionControlMode::TotalTxCount => 1,
        }
    }

    /// Check whether to use the minimum free execution slot to schedule
    /// execution of a transaction.
    pub(super) fn congestion_control_min_free_execution_slot(&self) -> bool {
        self.congestion_control_min_free_execution_slot
    }

    /// Check whether shared-object congestion control is enabled.
    fn is_congestion_control_enabled(&self) -> bool {
        self.max_execution_duration_per_commit.is_some()
    }

    /// Get maximum execution duration per shared object per commit.
    pub(super) fn max_execution_duration_per_commit(&self) -> Option<ExecutionTime> {
        self.max_execution_duration_per_commit
    }

    /// Get the maximum gas price that can be set in transactions.
    pub(super) fn max_gas_price(&self) -> u64 {
        self.max_gas_price
    }

    /// Whether to use congestion limit overshoot in the gas price feedback
    /// mechanism.
    pub(super) fn use_congestion_limit_overshoot_in_gas_price_feedback_mechanism(&self) -> bool {
        self.use_congestion_limit_overshoot_in_gas_price_feedback_mechanism
    }

    /// Whether to use a separate gas price feedback mechanism for transactions
    /// using randomness.
    pub(super) fn use_separate_gas_price_feedback_mechanism_for_randomness(&self) -> bool {
        self.use_separate_gas_price_feedback_mechanism_for_randomness
    }

    /// Get effective congestion limit per commit, i.e.,
    /// `max_execution_duration_per_commit` plus
    /// `max_congestion_limit_overshoot_per_commit`.
    /// Returns `None` if `max_execution_duration_per_commit` is not set,
    /// i.e., shared-object congestion control is disabled.
    pub(super) fn get_effective_congestion_limit_per_commit(&self) -> Option<ExecutionTime> {
        self.max_execution_duration_per_commit
            .map(|max_execution_duration_per_commit| {
                max_execution_duration_per_commit.saturating_add(
                    // If `max_congestion_limit_overshoot_per_commit` is not set,
                    // add 0 to `max_execution_duration_per_commit`, that is,
                    // ignore congestion limit overshoot.
                    self.max_congestion_limit_overshoot_per_commit
                        .unwrap_or_default(),
                )
            })
    }
}

/// An alias type for a collection used to hold previously deferred
/// transactions, where `Option<u64>` is used to hold suggested gas
/// price for transactions deferred due to shared object congestion
/// (`None` for transactions deferred due to "randomness not available").
pub(crate) type PreviouslyDeferredTransactions =
    HashMap<TransactionDigest, (DeferralKey, Option<u64>)>;

/// Holds a verified sequenced consensus transaction that is deferred
/// and optionally a suggested gas price for that transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeferredTransaction {
    /// Deferred verified sequenced consensus transaction.
    transaction: VerifiedSequencedConsensusTransaction,

    /// Suggested gas price is `Some(u64)` for transactions deferred due
    /// to shared object congestion if gas price feedback is enabled, and
    /// it is `None` otherwise and for transactions deferred due to
    /// "randomness not available".
    suggested_gas_price: Option<u64>,
}

impl DeferredTransaction {
    /// Construct a new `DeferredTransaction` instance from a deferred
    /// verified sequenced consensus transaction and optionally a suggested
    /// gas price for that transaction.
    pub fn new(
        transaction: VerifiedSequencedConsensusTransaction,
        suggested_gas_price: Option<u64>,
    ) -> Self {
        Self {
            transaction,
            suggested_gas_price,
        }
    }

    pub fn suggested_gas_price(&self) -> Option<u64> {
        self.suggested_gas_price
    }
}

/// Represents a scheduling result: a transaction can be either scheduled
/// for execution, or deferred for some reason. Scheduling result is
/// returned by the `try_schedule` method of `AuthorityPerEpochStore`.
#[derive(Debug)]
pub(crate) enum SchedulingResult {
    /// Scheduling result indicating that a transaction is scheduled to be
    /// executed at start time
    Schedule(/* start_time */ ExecutionTime),

    /// Scheduling result indicating that a transaction is deferred
    Defer(DeferralKey, DeferralReason),
}

pub enum CancelConsensusCertificateReason {
    CongestionOnObjects {
        congested_objects: Vec<ObjectID>,
        suggested_gas_price: Option<u64>,
    },
    DkgFailed,
}

pub enum ConsensusCertificateResult {
    /// The consensus message was ignored (e.g. because it has already been
    /// processed).
    Ignored,
    /// The transaction is scheduled for execution (can be a user tx or a
    /// system tx) with start_time. The start_time is an ExecutionTime assigned
    /// by the SharedObjectCongestionTracker and it implies its
    /// execution order. Before a batch of scheduled transactions are sent
    /// for execution, they will be ordered by their start_time
    /// (ascendingly). start_times of shared object transactions imply
    /// causal ordering. Owned object transactions will always have
    /// start_time 0, meaning they are not dependent on another transaction
    /// and they will not wait for another transaction.
    Scheduled {
        transaction: VerifiedExecutableTransaction,
        start_time: ExecutionTime,
    },
    /// The transaction should be re-processed at a future commit, specified by
    /// `deferral_key`. If the gas price feedback is enabled,
    /// `suggested_gas_price` is `Some(...)` and indicates a gas price that
    /// the certificate would need to pay to be scheduled in a consensus
    /// commit. If the feedback mechanism is not enabled and for
    /// certificates deferred due to "randomness not available",
    /// the `suggested_gas_price` price field will be set to `None`.
    Deferred {
        deferral_key: DeferralKey,
        suggested_gas_price: Option<u64>,
    },
    /// A message was processed which updates randomness state.
    RandomnessConsensusMessage,
    /// Everything else, e.g. AuthorityCapabilities, CheckpointSignatures, etc.
    ConsensusMessage,
    /// A system message in consensus was ignored (e.g. because of end of
    /// epoch).
    IgnoredSystem,
    /// A will-be-cancelled transaction. It'll still go through execution engine
    /// (but not be executed), unlock any owned objects, and return
    /// corresponding cancellation error according to
    /// `CancelConsensusCertificateReason`.
    Cancelled(
        (
            VerifiedExecutableTransaction,
            CancelConsensusCertificateReason,
        ),
    ),
}

/// ConsensusStats is versioned because we may iterate on the struct, and it is
/// stored on disk.
#[enum_dispatch]
pub trait ConsensusStatsAPI {
    fn is_initialized(&self) -> bool;

    fn get_num_messages(&self, authority: usize) -> u64;
    fn inc_num_messages(&mut self, authority: usize, num_of_new_messages: u64) -> u64;

    fn get_num_user_transactions(&self, authority: usize) -> u64;
    fn inc_num_user_transactions(&mut self, authority: usize) -> u64;
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[enum_dispatch(ConsensusStatsAPI)]
pub enum ConsensusStats {
    V1(ConsensusStatsV1),
}

impl ConsensusStats {
    pub fn new(size: usize) -> Self {
        Self::V1(ConsensusStatsV1 {
            num_messages: vec![0; size],
            num_user_transactions: vec![0; size],
        })
    }
}

impl Default for ConsensusStats {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ConsensusStatsV1 {
    pub num_messages: Vec<u64>,
    pub num_user_transactions: Vec<u64>,
}

impl ConsensusStatsAPI for ConsensusStatsV1 {
    fn is_initialized(&self) -> bool {
        !self.num_messages.is_empty()
    }

    fn get_num_messages(&self, authority: usize) -> u64 {
        self.num_messages[authority]
    }

    fn inc_num_messages(&mut self, authority: usize, num_of_new_messages: u64) -> u64 {
        self.num_messages[authority] += num_of_new_messages;
        self.num_messages[authority]
    }

    fn get_num_user_transactions(&self, authority: usize) -> u64 {
        self.num_user_transactions[authority]
    }

    fn inc_num_user_transactions(&mut self, authority: usize) -> u64 {
        self.num_user_transactions[authority] += 1;
        self.num_user_transactions[authority]
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, Copy)]
pub struct ExecutionIndices {
    /// The round number of the last committed leader.
    pub last_committed_round: u64,
    /// The index of the last sub-DAG that was executed (either fully or
    /// partially).
    pub sub_dag_index: u64,
    /// The index of the last transaction was executed (used for
    /// crash-recovery).
    pub transaction_index: u64,
}

impl Ord for ExecutionIndices {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            self.last_committed_round,
            self.sub_dag_index,
            self.transaction_index,
        )
            .cmp(&(
                other.last_committed_round,
                other.sub_dag_index,
                other.transaction_index,
            ))
    }
}

impl PartialOrd for ExecutionIndices {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionIndicesWithStats {
    pub index: ExecutionIndices,
    // Hash is always 0 and kept for compatibility only.
    pub hash: u64,
    pub stats: ConsensusStats,
}

type ExecutionModuleCache = SyncModuleCache<ResolverWrapper>;

// Data related to VM and Move execution and type layout
pub struct ExecutionComponents {
    pub(crate) executor: Arc<dyn Executor + Send + Sync>,
    // TODO: use strategies (e.g. LRU?) to constraint memory usage
    pub(crate) module_cache: Arc<ExecutionModuleCache>,
    metrics: Arc<ResolverMetrics>,
}

#[cfg(test)]
#[path = "../unit_tests/authority_per_epoch_store_tests.rs"]
pub mod authority_per_epoch_store_tests;

/// The `AuthorityPerEpochStore` struct manages state and resources specific to
/// each epoch within a validator's lifecycle. It includes the validator's name,
/// the committee for the current epoch, and various in-memory caches and
/// notification mechanisms to track the state of transactions and certificates.
/// The struct also incorporates mechanisms for managing signature verification,
/// epoch transitions, and randomness generation. Additionally, it maintains
/// locks and barriers to ensure that tasks related to reconfiguration and epoch
/// transitions are handled correctly and without interference. This struct is
/// designed to manage tasks and data that are valid only within a single epoch.
pub struct AuthorityPerEpochStore {
    /// The name of this authority.
    pub(crate) name: AuthorityName,

    /// Committee of validators for the current epoch.
    committee: Arc<Committee>,

    /// Holds the underlying per-epoch typed store tables.
    /// This is an ArcSwapOption because it needs to be used concurrently,
    /// and it needs to be cleared at the end of the epoch.
    tables: ArcSwapOption<AuthorityEpochTables>,

    /// Holds the outputs of both consensus handler and checkpoint builder in
    /// memory until they are proven not to have forked by a certified
    /// checkpoint.
    consensus_quarantine: RwLock<ConsensusOutputQuarantine>,
    /// Holds various data from consensus_quarantine in a more easily
    /// accessible form.
    consensus_output_cache: ConsensusOutputCache,

    protocol_config: ProtocolConfig,

    // needed for re-opening epoch db.
    parent_path: PathBuf,
    db_options: Option<Options>,

    /// In-memory cache of the content from the reconfig_state db table.
    reconfig_state_mem: RwLock<ReconfigState>,
    consensus_notify_read: NotifyRead<SequencedConsensusTransactionKey, ()>,

    // Subscribers will get notified when a transaction is executed via checkpoint execution.
    // The value is (checkpoint_sequence_number, checkpoint_timestamp_ms).
    executed_transactions_to_checkpoint_notify_read:
        NotifyRead<TransactionDigest, (CheckpointSequenceNumber, u64)>,

    /// Batch verifier for certificates - also caches certificates and tx sigs
    /// that are known to have valid signatures. Lives in per-epoch store
    /// because the caching/batching is only valid within for certs within
    /// the current epoch.
    pub(crate) signature_verifier: SignatureVerifier,

    pub(crate) checkpoint_state_notify_read: NotifyRead<CheckpointSequenceNumber, Accumulator>,

    running_root_notify_read: NotifyRead<CheckpointSequenceNumber, Accumulator>,

    executed_digests_notify_read: NotifyRead<TransactionKey, TransactionDigest>,

    /// This is used to notify all epoch specific tasks that epoch has ended.
    epoch_alive_notify: NotifyOnce,

    /// Used to notify all epoch specific tasks that user certs are closed.
    user_certs_closed_notify: NotifyOnce,

    /// This lock acts as a barrier for tasks that should not be executed in
    /// parallel with reconfiguration See comments in
    /// AuthorityPerEpochStore::epoch_terminated() on how this is used Crash
    /// recovery note: we write next epoch in the database first, and then use
    /// this lock to wait for in-memory tasks for the epoch to finish. If
    /// node crashes at this stage validator will start with the new
    /// epoch(and will open instance of per-epoch store for a new epoch).
    epoch_alive: tokio::sync::RwLock<bool>,
    end_of_publish: Mutex<StakeAggregator<(), true>>,
    /// Pending certificates that are waiting to be sequenced by the consensus.
    /// This is an in-memory 'index' of a
    /// AuthorityPerEpochTables::pending_consensus_transactions. We need to
    /// keep track of those in order to know when to send EndOfPublish message.
    /// Lock ordering: this is a 'leaf' lock, no other locks should be acquired
    /// in the scope of this lock In particular, this lock is always
    /// acquired after taking read or write lock on reconfig state
    pending_consensus_certificates: RwLock<HashSet<TransactionDigest>>,

    /// MutexTable for transaction locks (prevent concurrent execution of same
    /// transaction)
    mutex_table: MutexTable<TransactionDigest>,
    /// Mutex table for shared version assignment
    version_assignment_mutex_table: MutexTable<ObjectID>,

    /// The moment when the current epoch started locally on this validator.
    /// Note that this value could be skewed if the node crashed and
    /// restarted in the middle of the epoch. That's ok because this is used
    /// for metric purposes and we could tolerate some skews occasionally.
    pub(crate) epoch_open_time: Instant,

    /// The moment when epoch is closed. We don't care much about crash recovery
    /// because it's a metric that doesn't have to be available for each
    /// epoch, and it's only used during the last few seconds of an epoch.
    epoch_close_time: RwLock<Option<Instant>>,
    pub(crate) metrics: Arc<EpochMetrics>,
    epoch_start_configuration: Arc<EpochStartConfiguration>,

    /// Execution state that has to restart at each epoch change
    execution_component: ExecutionComponents,

    /// Chain identifier
    /// ChainIdentifier is always the true id (digest of genesis checkpoint).
    /// Chain is the nominal identifier and can be overridden for testing
    /// purposes.
    chain: (ChainIdentifier, Chain),

    /// aggregator for JWK votes
    jwk_aggregator: Mutex<JwkAggregator>,

    /// State machine managing randomness DKG and generation.
    randomness_manager: OnceCell<tokio::sync::Mutex<RandomnessManager>>,
    randomness_reporter: OnceCell<RandomnessReporter>,

    /// Component including the local view about the other authorities'
    /// misbehavior metrics, and received reports.
    pub(crate) scorer: Arc<Scorer>,
}

/// AuthorityEpochTables contains tables that contain data that is only valid
/// within an epoch.
#[derive(DBMapUtils)]
pub struct AuthorityEpochTables {
    /// This is map between the transaction digest and transactions found in the
    /// `transaction_lock`.
    #[default_options_override_fn = "signed_transactions_table_default_config"]
    signed_transactions:
        DBMap<TransactionDigest, TrustedEnvelope<SenderSignedData, AuthoritySignInfo>>,

    /// Map from ObjectRef to transaction locking that object
    #[default_options_override_fn = "owned_object_locked_transactions_table_default_config"]
    owned_object_locked_transactions: DBMap<ObjectRef, LockDetailsWrapper>,

    /// Signatures over transaction effects that we have signed and returned to
    /// users. We store this to avoid re-signing the same effects twice.
    /// Note that this may contain signatures for effects from previous epochs,
    /// in the case that a user requests a signature for effects from a
    /// previous epoch. However, the signature is still epoch-specific and
    /// so is stored in the epoch store.
    effects_signatures: DBMap<TransactionDigest, AuthoritySignInfo>,

    /// When we sign a TransactionEffects, we must record the digest of the
    /// effects in order to detect and prevent equivocation when
    /// re-executing a transaction that may not have been committed to disk.
    /// Entries are removed from this table after the transaction in question
    /// has been committed to disk.
    signed_effects_digests: DBMap<TransactionDigest, TransactionEffectsDigest>,

    /// Signatures of transaction certificates that are executed locally.
    transaction_cert_signatures: DBMap<TransactionDigest, AuthorityStrongQuorumSignInfo>,

    /// Next available shared object versions for each shared object.
    next_shared_object_versions: DBMap<ObjectID, SequenceNumber>,

    /// Track which transactions have been processed in
    /// handle_consensus_transaction. We must be sure to advance
    /// next_shared_object_versions exactly once for each transaction we receive
    /// from consensus. But, we may also be processing transactions from
    /// checkpoints, so we need to track this state separately.
    ///
    /// Entries in this table can be garbage collected whenever we can prove
    /// that we won't receive another handle_consensus_transaction call for
    /// the given digest. This probably means at epoch change.
    consensus_message_processed: DBMap<SequencedConsensusTransactionKey, bool>,

    /// Map stores pending transactions that this authority submitted to
    /// consensus
    #[default_options_override_fn = "pending_consensus_transactions_table_default_config"]
    pending_consensus_transactions: DBMap<ConsensusTransactionKey, ConsensusTransaction>,

    /// The following table is used to store a single value (the corresponding
    /// key is a constant). The value represents the index of the latest
    /// consensus message this authority processed, running hash of
    /// transactions, and accumulated stats of consensus output.
    /// This field is written by a single process (consensus handler).
    last_consensus_stats: DBMap<u64, ExecutionIndicesWithStats>,

    /// This table contains current reconfiguration state for validator for
    /// current epoch
    reconfig_state: DBMap<u64, ReconfigState>,

    /// Validators that have sent EndOfPublish message in this epoch
    end_of_publish: DBMap<AuthorityName, ()>,

    /// Checkpoint builder maintains internal list of transactions it included
    /// in checkpoints here
    builder_digest_to_checkpoint: DBMap<TransactionDigest, CheckpointSequenceNumber>,

    /// Maps non-digest TransactionKeys to the corresponding digest after
    /// execution, for use by checkpoint builder.
    transaction_key_to_digest: DBMap<TransactionKey, TransactionDigest>,

    /// Stores pending signatures
    /// The key in this table is checkpoint sequence number and an arbitrary
    /// integer
    pub(crate) pending_checkpoint_signatures:
        DBMap<(CheckpointSequenceNumber, u64), CheckpointSignatureMessage>,

    /// Maps sequence number to checkpoint summary, used by CheckpointBuilder to
    /// build checkpoint within epoch
    builder_checkpoint_summary: DBMap<CheckpointSequenceNumber, BuilderCheckpointSummary>,

    // Maps checkpoint sequence number to an accumulator with accumulated state
    // only for the checkpoint that the key references. Append-only, i.e.,
    // the accumulator is complete wrt the checkpoint
    pub state_hash_by_checkpoint: DBMap<CheckpointSequenceNumber, Accumulator>,

    /// Maps checkpoint sequence number to the running (non-finalized) root
    /// state accumulator up th that checkpoint. This should be equivalent
    /// to the root state hash at end of epoch. Guaranteed to be written to
    /// in checkpoint sequence number order.
    pub running_root_accumulators: DBMap<CheckpointSequenceNumber, Accumulator>,

    /// Record of the capabilities advertised by each authority.
    authority_capabilities_v1: DBMap<AuthorityName, AuthorityCapabilitiesV1>,

    /// Contains a single key, which overrides the value of
    /// ProtocolConfig::buffer_stake_for_protocol_upgrade_bps
    override_protocol_upgrade_buffer_stake: DBMap<u64, u64>,

    /// When transaction is executed via checkpoint executor, we store
    /// association here
    pub(crate) executed_transactions_to_checkpoint:
        DBMap<TransactionDigest, CheckpointSequenceNumber>,

    /// JWKs that have been voted for by one or more authorities but are not yet
    /// active.
    pending_jwks: DBMap<(AuthorityName, JwkId, JWK), ()>,

    /// JWKs that are currently available for zklogin authentication, and the
    /// round in which they became active.
    /// This would normally be stored as (JwkId, JWK) -> u64, but we need to be
    /// able to scan to find all Jwks for a given round
    active_jwks: DBMap<(u64, (JwkId, JWK)), ()>,

    /// Transactions that are being deferred until some future time
    deferred_transactions: DBMap<DeferralKey, Vec<VerifiedSequencedConsensusTransaction>>,

    /// Transactions that are being deferred until some future time.
    /// V2 additionally includes suggested gas price for transactions
    /// deferred due to congestion.
    deferred_transactions_v2: DBMap<DeferralKey, Vec<DeferredTransaction>>,

    // Tables for recording state for RandomnessManager.

    //
    /// Records messages processed from other nodes. Updated when receiving a
    /// new dkg::Message via consensus.
    pub(crate) dkg_processed_messages: DBMap<PartyId, VersionedProcessedMessage>,

    /// Records messages used to generate a DKG confirmation. Updated when
    /// enough DKG messages are received to progress to the next phase.
    pub(crate) dkg_used_messages: DBMap<u64, VersionedUsedProcessedMessages>,

    /// Records confirmations received from other nodes. Updated when receiving
    /// a new dkg::Confirmation via consensus.
    pub(crate) dkg_confirmations: DBMap<PartyId, VersionedDkgConfirmation>,

    /// Records the final output of DKG after completion, including the public
    /// VSS key and any local private shares.
    pub(crate) dkg_output: DBMap<u64, dkg_v1::Output<PkG, EncG>>,

    /// Holds the value of the next RandomnessRound to be generated.
    pub(crate) randomness_next_round: DBMap<u64, RandomnessRound>,

    /// Holds the value of the highest completed RandomnessRound (as reported to
    /// RandomnessReporter).
    pub(crate) randomness_highest_completed_round: DBMap<u64, RandomnessRound>,

    /// Holds the timestamp of the most recently generated round of randomness.
    pub(crate) randomness_last_round_timestamp: DBMap<u64, CommitTimestampMs>,

    // Tables for recording per-object debts for congestion control.

    //
    /// Accumulated per-object debts for congestion control.
    congestion_control_object_debts: DBMap<ObjectID, CongestionPerObjectDebt>,

    /// Accumulated per-object debts for randomness congestion control.
    congestion_control_randomness_object_debts: DBMap<ObjectID, CongestionPerObjectDebt>,
}

fn signed_transactions_table_default_config() -> DBOptions {
    default_db_options()
        .optimize_for_write_throughput()
        .optimize_for_large_values_no_scan(1 << 10)
}

fn owned_object_locked_transactions_table_default_config() -> DBOptions {
    DBOptions {
        options: default_db_options()
            .optimize_for_write_throughput()
            .optimize_for_read(read_size_from_env(ENV_VAR_LOCKS_BLOCK_CACHE_SIZE).unwrap_or(1024))
            .options,
        rw_options: ReadWriteOptions::default().set_ignore_range_deletions(false),
    }
}

fn pending_consensus_transactions_table_default_config() -> DBOptions {
    default_db_options()
        .optimize_for_write_throughput()
        .optimize_for_large_values_no_scan(1 << 10)
}

impl AuthorityEpochTables {
    pub fn open(epoch: EpochId, parent_path: &Path, db_options: Option<Options>) -> Self {
        Self::open_tables_read_write(
            Self::path(epoch, parent_path),
            MetricConf::new("epoch"),
            db_options,
            None,
        )
    }

    pub fn open_readonly(epoch: EpochId, parent_path: &Path) -> AuthorityEpochTablesReadOnly {
        Self::get_read_only_handle(
            Self::path(epoch, parent_path),
            None,
            None,
            MetricConf::new("epoch_readonly"),
        )
    }

    pub fn path(epoch: EpochId, parent_path: &Path) -> PathBuf {
        parent_path.join(format!("{EPOCH_DB_PREFIX}{epoch}"))
    }

    fn load_reconfig_state(&self) -> IotaResult<ReconfigState> {
        let state = self
            .reconfig_state
            .get(&RECONFIG_STATE_INDEX)?
            .unwrap_or_default();
        Ok(state)
    }

    pub fn get_all_pending_consensus_transactions(&self) -> IotaResult<Vec<ConsensusTransaction>> {
        Ok(self
            .pending_consensus_transactions
            .safe_iter()
            .map(|item| item.map(|(_k, v)| v))
            .collect::<Result<Vec<_>, _>>()?)
    }

    pub fn reset_db_for_execution_since_genesis(&self) -> IotaResult {
        // TODO: Add new tables that get added to the db automatically
        self.executed_transactions_to_checkpoint.unsafe_clear()?;
        Ok(())
    }

    pub fn get_transaction_checkpoint(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<CheckpointSequenceNumber>> {
        Ok(self.executed_transactions_to_checkpoint.get(digest)?)
    }

    /// WARNING: This method is very subtle and can corrupt the database if used
    /// incorrectly. It should only be used in one-off cases or tests after
    /// fully understanding the risk.
    pub fn remove_executed_tx_subtle(&self, digest: &TransactionDigest) -> IotaResult {
        self.executed_transactions_to_checkpoint.remove(digest)?;
        Ok(())
    }

    pub fn get_last_consensus_index(&self) -> IotaResult<Option<ExecutionIndices>> {
        Ok(self
            .last_consensus_stats
            .get(&LAST_CONSENSUS_STATS_ADDR)?
            .map(|s| s.index))
    }

    pub fn get_last_consensus_stats(&self) -> IotaResult<Option<ExecutionIndicesWithStats>> {
        Ok(self.last_consensus_stats.get(&LAST_CONSENSUS_STATS_ADDR)?)
    }

    pub fn get_locked_transaction(&self, obj_ref: &ObjectRef) -> IotaResult<Option<LockDetails>> {
        Ok(self
            .owned_object_locked_transactions
            .get(obj_ref)?
            .map(|l| l.migrate().into_inner()))
    }

    pub fn multi_get_locked_transactions(
        &self,
        owned_input_objects: &[ObjectRef],
    ) -> IotaResult<Vec<Option<LockDetails>>> {
        Ok(self
            .owned_object_locked_transactions
            .multi_get(owned_input_objects)?
            .into_iter()
            .map(|l| l.map(|l| l.migrate().into_inner()))
            .collect())
    }

    pub fn write_transaction_locks(
        &self,
        transaction: VerifiedSignedTransaction,
        locks_to_write: impl Iterator<Item = (ObjectRef, LockDetails)>,
    ) -> IotaResult {
        let mut batch = self.owned_object_locked_transactions.batch();
        batch.insert_batch(
            &self.owned_object_locked_transactions,
            locks_to_write.map(|(obj_ref, lock)| (obj_ref, LockDetailsWrapper::from(lock))),
        )?;
        batch.insert_batch(
            &self.signed_transactions,
            std::iter::once((*transaction.digest(), transaction.serializable_ref())),
        )?;
        batch.write()?;
        Ok(())
    }

    fn get_all_deferred_transactions(
        &self,
    ) -> IotaResult<BTreeMap<DeferralKey, Vec<VerifiedSequencedConsensusTransaction>>> {
        Ok(self
            .deferred_transactions
            .safe_iter()
            .collect::<Result<_, _>>()?)
    }

    fn get_all_deferred_transactions_v2(
        &self,
    ) -> IotaResult<BTreeMap<DeferralKey, Vec<DeferredTransaction>>> {
        Ok(self
            .deferred_transactions_v2
            .safe_iter()
            .collect::<Result<_, _>>()?)
    }
}

pub(crate) const MUTEX_TABLE_SIZE: usize = 1024;

impl AuthorityPerEpochStore {
    #[instrument(name = "AuthorityPerEpochStore::new", level = "error", skip_all, fields(epoch = committee.epoch))]
    pub fn new(
        name: AuthorityName,
        committee: Arc<Committee>,
        parent_path: &Path,
        db_options: Option<Options>,
        metrics: Arc<EpochMetrics>,
        epoch_start_configuration: EpochStartConfiguration,
        backing_package_store: Arc<dyn BackingPackageStore + Send + Sync>,
        object_store: Arc<dyn ObjectStore + Send + Sync>,
        cache_metrics: Arc<ResolverMetrics>,
        signature_verifier_metrics: Arc<SignatureVerifierMetrics>,
        expensive_safety_check_config: &ExpensiveSafetyCheckConfig,
        chain: (ChainIdentifier, Chain),
        highest_executed_checkpoint: CheckpointSequenceNumber,
    ) -> IotaResult<Arc<Self>> {
        let current_time = Instant::now();
        let epoch_id = committee.epoch;

        let tables = AuthorityEpochTables::open(epoch_id, parent_path, db_options.clone());
        let end_of_publish =
            StakeAggregator::from_iter(committee.clone(), tables.end_of_publish.safe_iter())?;
        let reconfig_state = tables
            .load_reconfig_state()
            .expect("Load reconfig state at initialization cannot fail");

        let epoch_alive_notify = NotifyOnce::new();
        let pending_consensus_transactions = tables.get_all_pending_consensus_transactions()?;
        let pending_consensus_certificates: HashSet<_> = pending_consensus_transactions
            .iter()
            .filter_map(|transaction| {
                if let ConsensusTransactionKind::CertifiedTransaction(certificate) =
                    &transaction.kind
                {
                    Some(*certificate.digest())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            epoch_start_configuration.epoch_start_state().epoch(),
            epoch_id
        );
        let epoch_start_configuration = Arc::new(epoch_start_configuration);
        info!("epoch flags: {:?}", epoch_start_configuration.flags());
        metrics.current_epoch.set(epoch_id as i64);
        metrics
            .current_voting_right
            .set(committee.weight(&name) as i64);
        let protocol_version = epoch_start_configuration
            .epoch_start_state()
            .protocol_version();

        let chain_from_id = chain.0.chain();
        if chain_from_id == Chain::Mainnet || chain_from_id == Chain::Testnet {
            assert_eq!(
                chain_from_id, chain.1,
                "cannot override chain on production networks!"
            );
        }
        info!(
            "initializing epoch store from chain id {chain_from_id:?} to chain id {:?}",
            chain.1
        );

        let protocol_config = ProtocolConfig::get_for_version(protocol_version, chain.1);

        let execution_component = ExecutionComponents::new(
            &protocol_config,
            backing_package_store,
            cache_metrics,
            expensive_safety_check_config,
        );

        let zklogin_env = match chain.1 {
            // Testnet and mainnet are treated the same since it is permanent.
            Chain::Mainnet | Chain::Testnet => ZkLoginEnv::Prod,
            _ => ZkLoginEnv::Test,
        };

        // Get all active validators and filter out committee members to get
        // non-committee validators
        let non_committee_validators: BTreeSet<AuthorityName> = epoch_start_configuration
            .epoch_start_state()
            .get_active_validators()
            .into_iter()
            .filter_map(|pubkey| AuthorityName::from_bytes(pubkey.as_bytes()).ok())
            .filter(|authority_name| !committee.authority_exists(authority_name))
            .collect();

        let signature_verifier = SignatureVerifier::new(
            committee.clone(),
            non_committee_validators,
            signature_verifier_metrics,
            zklogin_env,
            protocol_config.accept_zklogin_in_multisig(),
            protocol_config.accept_passkey_in_multisig(),
            protocol_config.zklogin_max_epoch_upper_bound_delta(),
            protocol_config.additional_multisig_checks(),
        );

        let authenticator_state_exists = epoch_start_configuration
            .authenticator_obj_initial_shared_version()
            .is_some();
        let authenticator_state_enabled =
            authenticator_state_exists && protocol_config.enable_jwk_consensus_updates();

        if authenticator_state_enabled {
            info!("authenticator_state enabled");
            let authenticator_state = get_authenticator_state(&*object_store)
                .expect("Read cannot fail")
                .expect("Authenticator state must exist");

            for active_jwk in &authenticator_state.active_jwks {
                let ActiveJwk { jwk_id, jwk, epoch } = active_jwk;
                assert!(epoch <= &epoch_id);
                signature_verifier.insert_jwk(jwk_id, jwk);
            }
        } else {
            info!("authenticator_state disabled");
        }

        let mut jwk_aggregator = JwkAggregator::new(committee.clone());

        for item in tables.pending_jwks.safe_iter() {
            let ((authority, id, jwk), _) = item?;
            jwk_aggregator.insert(authority, (id, jwk));
        }

        let jwk_aggregator = Mutex::new(jwk_aggregator);

        let consensus_output_cache = ConsensusOutputCache::new(&tables, metrics.clone());

        let voting_power = committee.members().map(|(_, v)| *v).collect::<Vec<u64>>();

        let s = Arc::new(Self {
            name,
            committee,
            protocol_config: protocol_config.clone(),
            tables: ArcSwapOption::new(Some(Arc::new(tables))),
            consensus_output_cache,
            consensus_quarantine: RwLock::new(ConsensusOutputQuarantine::new(
                highest_executed_checkpoint,
                metrics.clone(),
            )),
            parent_path: parent_path.to_path_buf(),
            db_options,
            reconfig_state_mem: RwLock::new(reconfig_state),
            epoch_alive_notify,
            user_certs_closed_notify: NotifyOnce::new(),
            epoch_alive: tokio::sync::RwLock::new(true),
            consensus_notify_read: NotifyRead::new(),
            executed_transactions_to_checkpoint_notify_read: NotifyRead::new(),
            signature_verifier,
            checkpoint_state_notify_read: NotifyRead::new(),
            running_root_notify_read: NotifyRead::new(),
            executed_digests_notify_read: NotifyRead::new(),
            end_of_publish: Mutex::new(end_of_publish),
            pending_consensus_certificates: RwLock::new(pending_consensus_certificates),
            mutex_table: MutexTable::new(MUTEX_TABLE_SIZE),
            version_assignment_mutex_table: MutexTable::new(MUTEX_TABLE_SIZE),
            epoch_open_time: current_time,
            epoch_close_time: Default::default(),
            metrics,
            epoch_start_configuration,
            execution_component,
            chain,
            jwk_aggregator,
            randomness_manager: OnceCell::new(),
            randomness_reporter: OnceCell::new(),
            scorer: Arc::new(Scorer::new(voting_power, &protocol_config)),
        });

        s.update_buffer_stake_metric();
        Ok(s)
    }

    pub fn tables(&self) -> IotaResult<Arc<AuthorityEpochTables>> {
        match self.tables.load_full() {
            Some(tables) => Ok(tables),
            None => Err(IotaError::EpochEnded(self.epoch())),
        }
    }

    // Ideally the epoch tables handle should have the same lifetime as the outer
    // AuthorityPerEpochStore, and this function should be unnecessary. But
    // unfortunately, Arc<AuthorityPerEpochStore> outlives the
    // epoch significantly right now, so we need to manually release the tables to
    // release its memory usage.
    pub fn release_db_handles(&self) {
        // When the logic to release DB handles becomes obsolete, it may still be useful
        // to make sure AuthorityEpochTables is not used after the next epoch starts.
        self.tables.store(None);
    }

    // Returns true if authenticator state is enabled in the protocol config *and*
    // the authenticator state object already exists
    pub fn authenticator_state_enabled(&self) -> bool {
        self.protocol_config().enable_jwk_consensus_updates() && self.authenticator_state_exists()
    }

    pub fn authenticator_state_exists(&self) -> bool {
        self.epoch_start_configuration
            .authenticator_obj_initial_shared_version()
            .is_some()
    }

    pub fn randomness_reporter(&self) -> Option<RandomnessReporter> {
        self.randomness_reporter.get().cloned()
    }

    pub async fn set_randomness_manager(
        &self,
        mut randomness_manager: RandomnessManager,
    ) -> IotaResult<()> {
        let reporter = randomness_manager.reporter();
        let result = randomness_manager.start_dkg().await;
        if self
            .randomness_manager
            .set(tokio::sync::Mutex::new(randomness_manager))
            .is_err()
        {
            error!("BUG: `set_randomness_manager` called more than once; this should never happen");
        }
        if self.randomness_reporter.set(reporter).is_err() {
            error!("BUG: `set_randomness_manager` called more than once; this should never happen");
        }
        result
    }

    pub fn get_parent_path(&self) -> PathBuf {
        self.parent_path.clone()
    }

    /// Returns `&Arc<EpochStartConfiguration>`
    /// User can treat this `Arc` as `&EpochStartConfiguration`, or clone the
    /// Arc to pass as owned object
    pub fn epoch_start_config(&self) -> &Arc<EpochStartConfiguration> {
        &self.epoch_start_configuration
    }

    pub fn epoch_start_state(&self) -> &EpochStartSystemState {
        self.epoch_start_configuration.epoch_start_state()
    }

    pub fn get_chain_identifier(&self) -> ChainIdentifier {
        self.chain.0
    }

    pub fn get_chain(&self) -> Chain {
        self.chain.1
    }

    pub fn new_at_next_epoch(
        &self,
        name: AuthorityName,
        new_committee: Committee,
        epoch_start_configuration: EpochStartConfiguration,
        backing_package_store: Arc<dyn BackingPackageStore + Send + Sync>,
        object_store: Arc<dyn ObjectStore + Send + Sync>,
        expensive_safety_check_config: &ExpensiveSafetyCheckConfig,
        previous_epoch_last_checkpoint: CheckpointSequenceNumber,
    ) -> IotaResult<Arc<Self>> {
        assert_eq!(self.epoch() + 1, new_committee.epoch);
        self.record_reconfig_halt_duration_metric();
        self.record_epoch_total_duration_metric();
        Self::new(
            name,
            Arc::new(new_committee),
            &self.parent_path,
            self.db_options.clone(),
            self.metrics.clone(),
            epoch_start_configuration,
            backing_package_store,
            object_store,
            self.execution_component.metrics(),
            self.signature_verifier.metrics.clone(),
            expensive_safety_check_config,
            self.chain,
            previous_epoch_last_checkpoint,
        )
    }

    pub fn new_at_next_epoch_for_testing(
        &self,
        backing_package_store: Arc<dyn BackingPackageStore + Send + Sync>,
        object_store: Arc<dyn ObjectStore + Send + Sync>,
        expensive_safety_check_config: &ExpensiveSafetyCheckConfig,
        previous_epoch_last_checkpoint: CheckpointSequenceNumber,
    ) -> Arc<Self> {
        let next_epoch = self.epoch() + 1;
        let next_committee = Committee::new(
            next_epoch,
            self.committee.voting_rights.iter().cloned().collect(),
        );
        self.new_at_next_epoch(
            self.name,
            next_committee,
            self.epoch_start_configuration
                .new_at_next_epoch_for_testing(),
            backing_package_store,
            object_store,
            expensive_safety_check_config,
            previous_epoch_last_checkpoint,
        )
        .expect("failed to create new authority per epoch store")
    }

    pub fn committee(&self) -> &Arc<Committee> {
        &self.committee
    }

    pub fn protocol_config(&self) -> &ProtocolConfig {
        &self.protocol_config
    }

    pub fn epoch(&self) -> EpochId {
        self.committee.epoch
    }

    pub fn get_state_hash_for_checkpoint(
        &self,
        checkpoint: &CheckpointSequenceNumber,
    ) -> IotaResult<Option<Accumulator>> {
        Ok(self
            .tables()?
            .state_hash_by_checkpoint
            .get(checkpoint)
            .expect("db error"))
    }

    pub fn insert_state_hash_for_checkpoint(
        &self,
        checkpoint: &CheckpointSequenceNumber,
        accumulator: &Accumulator,
    ) -> IotaResult {
        self.tables()?
            .state_hash_by_checkpoint
            .insert(checkpoint, accumulator)
            .expect("db error");
        Ok(())
    }

    pub fn get_running_root_accumulator(
        &self,
        checkpoint: CheckpointSequenceNumber,
    ) -> IotaResult<Option<Accumulator>> {
        Ok(self
            .tables()?
            .running_root_accumulators
            .get(&checkpoint)
            .expect("db error"))
    }

    pub fn get_highest_running_root_accumulator(
        &self,
    ) -> IotaResult<Option<(CheckpointSequenceNumber, Accumulator)>> {
        Ok(self
            .tables()?
            .running_root_accumulators
            .reversed_safe_iter_with_bounds(None, None)?
            .next()
            .transpose()?)
    }

    pub fn insert_running_root_accumulator(
        &self,
        checkpoint: &CheckpointSequenceNumber,
        acc: &Accumulator,
    ) -> IotaResult {
        self.tables()?
            .running_root_accumulators
            .insert(checkpoint, acc)?;
        self.running_root_notify_read.notify(checkpoint, acc);

        Ok(())
    }

    pub fn reference_gas_price(&self) -> u64 {
        // Determine what to use as reference gas price based on protocol config.
        if self.protocol_config().protocol_defined_base_fee() {
            self.protocol_config().base_gas_price()
        } else {
            self.epoch_start_state().reference_gas_price()
        }
    }

    pub fn protocol_version(&self) -> ProtocolVersion {
        self.epoch_start_state().protocol_version()
    }

    pub fn active_validators(&self) -> Vec<AuthorityPublicKey> {
        self.epoch_start_state().get_active_validators()
    }

    pub fn module_cache(&self) -> &Arc<ExecutionModuleCache> {
        &self.execution_component.module_cache
    }

    pub fn executor(&self) -> &Arc<dyn Executor + Send + Sync> {
        &self.execution_component.executor
    }

    pub fn acquire_tx_guard(
        &self,
        cert: &VerifiedExecutableTransaction,
    ) -> IotaResult<CertTxGuard> {
        let digest = cert.digest();
        Ok(CertTxGuard(self.acquire_tx_lock(digest)))
    }

    /// Acquire the lock for a tx without writing to the WAL.
    pub fn acquire_tx_lock(&self, digest: &TransactionDigest) -> CertLockGuard {
        CertLockGuard(self.mutex_table.acquire_lock(*digest))
    }

    pub fn store_reconfig_state(&self, new_state: &ReconfigState) -> IotaResult {
        self.tables()?
            .reconfig_state
            .insert(&RECONFIG_STATE_INDEX, new_state)?;
        Ok(())
    }

    pub fn insert_signed_transaction(&self, transaction: VerifiedSignedTransaction) -> IotaResult {
        Ok(self
            .tables()?
            .signed_transactions
            .insert(transaction.digest(), transaction.serializable_ref())?)
    }

    #[cfg(test)]
    pub fn delete_signed_transaction_for_test(&self, transaction: &TransactionDigest) {
        self.tables()
            .expect("test should not cross epoch boundary")
            .signed_transactions
            .remove(transaction)
            .unwrap();
    }

    #[cfg(test)]
    pub fn delete_object_locks_for_test(&self, objects: &[ObjectRef]) {
        for object in objects {
            self.tables()
                .expect("test should not cross epoch boundary")
                .owned_object_locked_transactions
                .remove(object)
                .unwrap();
        }
    }

    pub fn get_signed_transaction(
        &self,
        tx_digest: &TransactionDigest,
    ) -> IotaResult<Option<VerifiedSignedTransaction>> {
        Ok(self
            .tables()?
            .signed_transactions
            .get(tx_digest)?
            .map(|t| t.into()))
    }

    #[instrument(level = "trace", skip_all)]
    pub fn insert_tx_cert_sig(
        &self,
        tx_digest: &TransactionDigest,
        cert_sig: &AuthorityStrongQuorumSignInfo,
    ) -> IotaResult {
        let tables = self.tables()?;
        Ok(tables
            .transaction_cert_signatures
            .insert(tx_digest, cert_sig)?)
    }

    #[instrument(level = "trace", skip_all)]
    pub fn insert_tx_key_and_digest(
        &self,
        tx_key: &TransactionKey,
        tx_digest: &TransactionDigest,
    ) -> IotaResult {
        let _metrics_scope = iota_metrics::monitored_scope("AuthorityPerEpochStore::insert_tx_key");
        let tables = self.tables()?;

        self.consensus_output_cache
            .insert_executed_in_epoch(*tx_digest);

        if !matches!(tx_key, TransactionKey::Digest(_)) {
            tables.transaction_key_to_digest.insert(tx_key, tx_digest)?;
            self.executed_digests_notify_read.notify(tx_key, tx_digest);
        }

        Ok(())
    }

    pub(crate) fn remove_shared_version_assignments<'a>(
        &self,
        keys: impl IntoIterator<Item = &'a TransactionKey>,
    ) {
        self.consensus_output_cache
            .remove_shared_object_assignments(keys);
    }

    pub fn num_shared_version_assignments(&self) -> usize {
        self.consensus_output_cache.num_shared_version_assignments()
    }

    pub fn revert_executed_transaction(&self, tx_digest: &TransactionDigest) -> IotaResult {
        self.consensus_output_cache
            .remove_reverted_transaction(tx_digest);
        let tables = self.tables()?;
        let mut batch = tables.effects_signatures.batch();
        batch.delete_batch(&tables.effects_signatures, [*tx_digest])?;
        batch.write()?;
        Ok(())
    }

    pub fn insert_effects_digest_and_signature(
        &self,
        tx_digest: &TransactionDigest,
        effects_digest: &TransactionEffectsDigest,
        effects_signature: &AuthoritySignInfo,
    ) -> IotaResult {
        let tables = self.tables()?;
        let mut batch = tables.effects_signatures.batch();
        batch.insert_batch(&tables.effects_signatures, [(tx_digest, effects_signature)])?;
        batch.insert_batch(
            &tables.signed_effects_digests,
            [(tx_digest, effects_digest)],
        )?;
        batch.write()?;
        Ok(())
    }

    pub fn transactions_executed_in_cur_epoch(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<bool>> {
        let tables = self.tables()?;
        Ok(do_fallback_lookup(
            digests,
            |digest| {
                if self
                    .consensus_output_cache
                    .executed_in_current_epoch(digest)
                {
                    CacheResult::Hit(true)
                } else {
                    CacheResult::Miss
                }
            },
            |digests| {
                tables
                    .executed_transactions_to_checkpoint
                    .multi_contains_keys(digests)
                    .expect("db error")
            },
        ))
    }

    pub fn get_effects_signature(
        &self,
        tx_digest: &TransactionDigest,
    ) -> IotaResult<Option<AuthoritySignInfo>> {
        let tables = self.tables()?;
        Ok(tables.effects_signatures.get(tx_digest)?)
    }

    pub fn get_signed_effects_digest(
        &self,
        tx_digest: &TransactionDigest,
    ) -> IotaResult<Option<TransactionEffectsDigest>> {
        let tables = self.tables()?;
        Ok(tables.signed_effects_digests.get(tx_digest)?)
    }

    pub fn get_transaction_cert_sig(
        &self,
        tx_digest: &TransactionDigest,
    ) -> IotaResult<Option<AuthorityStrongQuorumSignInfo>> {
        Ok(self.tables()?.transaction_cert_signatures.get(tx_digest)?)
    }

    /// Resolves InputObjectKinds into InputKeys, by consulting the shared
    /// object version assignment table.
    pub(crate) fn get_input_object_keys(
        &self,
        key: &TransactionKey,
        objects: &[InputObjectKind],
    ) -> IotaResult<BTreeSet<InputKey>> {
        let assigned_shared_versions =
            once_cell::unsync::OnceCell::<Option<HashMap<ObjectID, SequenceNumber>>>::new();
        objects
            .iter()
            .map(|kind| {
                Ok(match kind {
                    InputObjectKind::SharedMoveObject { id, .. } => {
                        let assigned_shared_versions = assigned_shared_versions
                            .get_or_init(|| {
                                self.get_assigned_shared_object_versions(key)
                                    .map(|versions| versions.into_iter().collect())
                            })
                            .as_ref()
                            // Shared version assignments could have been deleted if the tx just
                            // finished executing concurrently.
                            .ok_or(IotaError::GenericAuthority {
                                error: "no assigned shared versions".to_string(),
                            })?;
                        // If we found assigned versions, but they are missing the assignment for
                        // this object, it indicates a serious inconsistency!
                        let Some(version) = assigned_shared_versions.get(id) else {
                            panic!(
                                "Shared object version should have been assigned. key: {key:?}, \
                                obj id: {id:?}, assigned_shared_versions: {assigned_shared_versions:?}",
                            )
                        };
                        InputKey::VersionedObject {
                            id: *id,
                            version: *version,
                        }
                    }
                    InputObjectKind::MovePackage(id) => InputKey::Package { id: *id },
                    InputObjectKind::ImmOrOwnedMoveObject(objref) => InputKey::VersionedObject {
                        id: objref.0,
                        version: objref.1,
                    },
                })
            })
            .collect()
    }

    pub fn get_last_consensus_stats(&self) -> IotaResult<ExecutionIndicesWithStats> {
        assert!(
            self.consensus_quarantine.read().is_empty(),
            "get_last_consensus_stats should only be called at startup"
        );
        match self.tables()?.get_last_consensus_stats()? {
            Some(stats) => Ok(stats),
            None => {
                let indices = self
                    .tables()?
                    .get_last_consensus_index()
                    .map(|x| x.unwrap_or_default())?;
                Ok(ExecutionIndicesWithStats {
                    index: indices,
                    hash: 0, // unused
                    stats: ConsensusStats::default(),
                })
            }
        }
    }

    pub fn get_accumulators_in_checkpoint_range(
        &self,
        from_checkpoint: CheckpointSequenceNumber,
        to_checkpoint: CheckpointSequenceNumber,
    ) -> IotaResult<Vec<(CheckpointSequenceNumber, Accumulator)>> {
        self.tables()?
            .state_hash_by_checkpoint
            .safe_range_iter(from_checkpoint..=to_checkpoint)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Returns future containing the state accumulator for the given epoch
    /// once available.
    pub async fn notify_read_checkpoint_state_accumulator(
        &self,
        checkpoints: &[CheckpointSequenceNumber],
    ) -> IotaResult<Vec<Accumulator>> {
        let tables = self.tables()?;
        self.checkpoint_state_notify_read
            .read(checkpoints, |checkpoints| {
                tables
                    .state_hash_by_checkpoint
                    .multi_get(checkpoints)
                    .map_err(Into::into)
            })
            .await
    }

    pub async fn notify_read_running_root(
        &self,
        checkpoint: CheckpointSequenceNumber,
    ) -> IotaResult<Accumulator> {
        let registration = self.running_root_notify_read.register_one(&checkpoint);
        let acc = self.tables()?.running_root_accumulators.get(&checkpoint)?;

        let result = match acc {
            Some(ready) => Either::Left(futures::future::ready(ready)),
            None => Either::Right(registration),
        }
        .await;

        Ok(result)
    }

    /// Called when transaction outputs are committed to disk
    #[instrument(level = "trace", skip_all)]
    pub fn handle_finalized_checkpoint(
        &self,
        checkpoint: &CheckpointSummary,
        digests: &[TransactionDigest],
    ) -> IotaResult<()> {
        let tables = match self.tables() {
            Ok(tables) => tables,
            // After Epoch ends, it is no longer necessary to remove pending transactions
            // because the table will not be used anymore and be deleted eventually.
            Err(IotaError::EpochEnded(_)) => return Ok(()),
            Err(e) => return Err(e),
        };
        let mut batch = tables.signed_effects_digests.batch();

        // Now that the transaction effects are committed, we will never re-execute, so
        // we don't need to worry about equivocating.
        batch.delete_batch(&tables.signed_effects_digests, digests)?;

        let seq = *checkpoint.sequence_number();

        let mut quarantine = self.consensus_quarantine.write();
        quarantine.update_highest_executed_checkpoint(seq, self, &mut batch)?;
        batch.write()?;

        self.consensus_output_cache
            .remove_executed_in_epoch(digests);

        Ok(())
    }

    pub fn get_all_pending_consensus_transactions(&self) -> Vec<ConsensusTransaction> {
        self.tables()
            .expect("recovery should not cross epoch boundary")
            .get_all_pending_consensus_transactions()
            .expect("failed to get pending consensus transactions")
    }

    #[cfg(test)]
    pub fn get_next_object_version(&self, obj: &ObjectID) -> Option<SequenceNumber> {
        self.tables()
            .expect("test should not cross epoch boundary")
            .next_shared_object_versions
            .get(obj)
            .unwrap()
    }

    pub fn set_shared_object_versions_for_testing(
        &self,
        tx_digest: &TransactionDigest,
        assigned_versions: &[(ObjectID, SequenceNumber)],
    ) -> IotaResult {
        self.consensus_output_cache
            .set_shared_object_versions_for_testing(tx_digest, assigned_versions);

        Ok(())
    }

    pub fn insert_finalized_transactions(
        &self,
        digests: &[TransactionDigest],
        sequence: CheckpointSequenceNumber,
        timestamp_ms: u64,
    ) -> IotaResult {
        let _metrics_scope =
            iota_metrics::monitored_scope("AuthorityPerEpochStore::insert_finalized_transactions");

        let mut batch = self.tables()?.executed_transactions_to_checkpoint.batch();
        batch.insert_batch(
            &self.tables()?.executed_transactions_to_checkpoint,
            digests.iter().map(|d| (*d, sequence)),
        )?;
        batch.write()?;
        trace!("Transactions {digests:?} finalized at checkpoint {sequence}");

        // Notify all readers that the transactions have been finalized as part of a
        // checkpoint execution.
        for digest in digests {
            self.executed_transactions_to_checkpoint_notify_read
                .notify(digest, &(sequence, timestamp_ms));
        }

        Ok(())
    }

    pub fn is_transaction_executed_in_checkpoint(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<bool> {
        Ok(self
            .tables()?
            .executed_transactions_to_checkpoint
            .contains_key(digest)?)
    }

    pub fn transactions_executed_in_checkpoint(
        &self,
        digests: impl Iterator<Item = TransactionDigest>,
    ) -> IotaResult<Vec<bool>> {
        Ok(self
            .tables()?
            .executed_transactions_to_checkpoint
            .multi_contains_keys(digests)?)
    }

    pub fn get_transaction_checkpoint(
        &self,
        digest: &TransactionDigest,
    ) -> IotaResult<Option<CheckpointSequenceNumber>> {
        Ok(self
            .tables()?
            .executed_transactions_to_checkpoint
            .get(digest)?)
    }

    pub fn multi_get_transaction_checkpoint(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<CheckpointSequenceNumber>>> {
        Ok(self
            .tables()?
            .executed_transactions_to_checkpoint
            .multi_get(digests)?)
    }

    // For each id in objects_to_init, return the next version for that id as
    // recorded in the next_shared_object_versions table.
    //
    // If any ids are missing, then we need to initialize the table. We first check
    // if a previous version of that object has been written. If so, then the
    // object was written in a previous epoch, and we initialize
    // next_shared_object_versions to that value. If no version of the
    // object has yet been written, we initialize the object to the initial version
    // recorded in the certificate (which is a function of the lamport version
    // computation of the transaction that created the shared object originally
    // - which transaction may not yet have been executed on this node).
    //
    // Because all paths that assign shared versions for a shared object transaction
    // call this function, it is impossible for parent_sync to be updated before
    // this function completes successfully for each affected object id.
    pub(crate) fn get_or_init_next_object_versions(
        &self,
        objects_to_init: &[(ObjectID, SequenceNumber)],
        cache_reader: &dyn ObjectCacheRead,
    ) -> IotaResult<HashMap<ObjectID, SequenceNumber>> {
        // get_or_init_next_object_versions can be called
        // from consensus or checkpoint executor,
        // so we need to protect version assignment with a critical section
        let _locks = self
            .version_assignment_mutex_table
            .acquire_locks(objects_to_init.iter().map(|(id, _)| *id));
        let tables = self.tables()?;

        let next_versions = self
            .consensus_quarantine
            .read()
            .get_next_shared_object_versions(&tables, objects_to_init)?;

        let uninitialized_objects: Vec<(ObjectID, SequenceNumber)> = next_versions
            .iter()
            .zip(objects_to_init)
            .filter_map(|(next_version, id_and_version)| match next_version {
                None => Some(*id_and_version),
                Some(_) => None,
            })
            .collect();

        // The common case is that there are no uninitialized versions - this early
        // return will happen every time except the first time an object is
        // used in an epoch.
        if uninitialized_objects.is_empty() {
            // unwrap ok - we already verified that next_versions is not missing any keys.
            return Ok(izip!(
                objects_to_init.iter().map(|(id, _)| *id),
                next_versions.into_iter().map(|v| v.unwrap())
            )
            .collect());
        }

        let versions_to_write: Vec<_> = uninitialized_objects
            .iter()
            .map(|(id, initial_version)| {
                // Note: we don't actually need to read from the transaction here, as no writer
                // can update object_store until after get_or_init_next_object_versions
                // completes.
                match cache_reader.get_object(id) {
                    Some(obj) => (*id, obj.version()),
                    None => (*id, *initial_version),
                }
            })
            .collect();

        let ret = izip!(objects_to_init.iter().cloned(), next_versions.into_iter(),)
            // take all the previously initialized versions
            .filter_map(|(id, next_version)| next_version.map(|v| (id.0, v)))
            // add all the versions we're going to write
            .chain(versions_to_write.iter().cloned())
            .collect();

        debug!(
            ?versions_to_write,
            "initializing next_shared_object_versions"
        );
        let mut batch = tables.next_shared_object_versions.batch();
        batch.insert_batch(&tables.next_shared_object_versions, versions_to_write)?;
        batch.write()?;

        Ok(ret)
    }

    pub fn get_assigned_shared_object_versions(
        &self,
        key: &TransactionKey,
    ) -> Option<Vec<(ObjectID, SequenceNumber)>> {
        self.consensus_output_cache
            .get_assigned_shared_object_versions(key)
    }

    fn set_assigned_shared_object_versions(&self, versions: AssignedTxAndVersions) {
        self.consensus_output_cache
            .insert_shared_object_assignments(&versions);
    }

    /// Given list of certificates, assign versions for all shared objects used
    /// in them. We start with the current next_shared_object_versions table
    /// for each object, and build up the versions based on the dependencies
    /// of each certificate. However, in the end we do not update the
    /// next_shared_object_versions table, which keeps this function
    /// idempotent. We should call this function when we are assigning shared
    /// object versions outside of consensus and do not want to taint the
    /// next_shared_object_versions table.
    pub fn assign_shared_object_versions_idempotent(
        &self,
        cache_reader: &dyn ObjectCacheRead,
        certificates: &[VerifiedExecutableTransaction],
    ) -> IotaResult {
        let assigned_versions = SharedObjVerManager::assign_versions_from_consensus(
            self,
            cache_reader,
            certificates.iter(),
            &BTreeMap::new(),
        )?
        .assigned_versions;
        self.set_assigned_shared_object_versions(assigned_versions);
        Ok(())
    }

    fn load_deferred_transactions_for_randomness(
        &self,
        output: &mut ConsensusCommitOutput,
    ) -> IotaResult<Vec<(DeferralKey, Vec<DeferredTransaction>)>> {
        let (min, max) = DeferralKey::full_range_for_randomness();
        self.load_deferred_transactions(output, min, max)
    }

    fn load_and_process_deferred_transactions_for_randomness(
        &self,
        output: &mut ConsensusCommitOutput,
        previously_deferred_tx_digests: &mut PreviouslyDeferredTransactions,
        sequenced_randomness_transactions: &mut Vec<VerifiedSequencedConsensusTransaction>,
    ) -> IotaResult {
        let deferred_randomness_txs = self.load_deferred_transactions_for_randomness(output)?;
        trace!(
            "loading deferred randomness transactions: {:?}",
            deferred_randomness_txs
        );
        previously_deferred_tx_digests.extend(deferred_randomness_txs.iter().flat_map(
            |(deferral_key, txs)| {
                txs.iter()
                    .map(|tx| match tx.transaction.0.transaction.key() {
                        SequencedConsensusTransactionKey::External(
                            ConsensusTransactionKey::Certificate(digest),
                        ) => (digest, (*deferral_key, tx.suggested_gas_price)),
                        _ => {
                            panic!(
                                "deferred randomness transaction was not a user certificate: {tx:?}"
                            )
                        }
                    })
            },
        ));
        sequenced_randomness_transactions.extend(
            deferred_randomness_txs
                .into_iter()
                .flat_map(|(_, txs)| txs.into_iter().map(|tx| tx.transaction).collect::<Vec<_>>()),
        );
        Ok(())
    }

    fn load_deferred_transactions_for_up_to_consensus_round(
        &self,
        output: &mut ConsensusCommitOutput,
        consensus_round: u64,
    ) -> IotaResult<Vec<(DeferralKey, Vec<DeferredTransaction>)>> {
        let (min, max) = DeferralKey::range_for_up_to_consensus_round(consensus_round);
        self.load_deferred_transactions(output, min, max)
    }

    // factoring of the above
    #[instrument(level = "trace", skip_all)]
    fn load_deferred_transactions(
        &self,
        output: &mut ConsensusCommitOutput,
        min: DeferralKey,
        max: DeferralKey,
    ) -> IotaResult<Vec<(DeferralKey, Vec<DeferredTransaction>)>> {
        debug!("Query epoch store to load deferred txn {:?} {:?}", min, max);

        let (keys, txns) = if self
            .protocol_config
            .congestion_control_gas_price_feedback_mechanism()
        {
            self.load_deferred_transactions_v2(min, max)
        } else {
            self.load_deferred_transactions_v1(min, max)
        };

        // verify that there are no duplicates - should be impossible due to
        // is_consensus_message_processed
        #[cfg(debug_assertions)]
        {
            let mut seen = HashSet::new();
            for deferred_txn_batch in &txns {
                for txn in &deferred_txn_batch.1 {
                    assert!(seen.insert(txn.transaction.0.key()));
                }
            }
        }

        output.delete_loaded_deferred_transactions(&keys);

        Ok(txns)
    }

    fn load_deferred_transactions_v2(
        &self,
        min: DeferralKey,
        max: DeferralKey,
    ) -> (
        Vec<DeferralKey>,
        Vec<(DeferralKey, Vec<DeferredTransaction>)>,
    ) {
        let (keys, txns) = {
            let mut keys = Vec::new();
            let mut txns = Vec::new();

            let deferred_transactions = self.consensus_output_cache.deferred_transactions_v2.lock();

            for (key, transactions) in deferred_transactions.range(min..max) {
                debug!(
                    "Loaded {:?} deferred txn with deferral key {:?}",
                    transactions.len(),
                    key
                );
                keys.push(*key);
                txns.push((*key, transactions.clone()));
            }

            (keys, txns)
        };

        (keys, txns)
    }

    fn load_deferred_transactions_v1(
        &self,
        min: DeferralKey,
        max: DeferralKey,
    ) -> (
        Vec<DeferralKey>,
        Vec<(DeferralKey, Vec<DeferredTransaction>)>,
    ) {
        let (keys, txns) = {
            let mut keys = Vec::new();
            let mut txns = Vec::new();

            let deferred_transactions = self.consensus_output_cache.deferred_transactions.lock();

            for (key, transactions) in deferred_transactions.range(min..max) {
                debug!(
                    "Loaded {:?} deferred txn with deferral key {:?}",
                    transactions.len(),
                    key
                );
                keys.push(*key);
                txns.push((
                    *key,
                    transactions
                        .iter()
                        .map(|tx| DeferredTransaction::new(tx.clone(), None))
                        .collect(),
                ));
            }

            (keys, txns)
        };

        (keys, txns)
    }

    pub fn get_all_deferred_transactions_for_test(
        &self,
    ) -> Vec<(DeferralKey, Vec<DeferredTransaction>)> {
        if self
            .protocol_config
            .congestion_control_gas_price_feedback_mechanism()
        {
            self.consensus_output_cache
                .deferred_transactions_v2
                .lock()
                .iter()
                .map(|(key, txs)| (*key, txs.clone()))
                .collect()
        } else {
            self.consensus_output_cache
                .deferred_transactions
                .lock()
                .iter()
                .map(|(key, txs)| {
                    let converted_txs: Vec<DeferredTransaction> = txs
                        .iter()
                        .map(|tx| DeferredTransaction::new(tx.clone(), None))
                        .collect();
                    (*key, converted_txs)
                })
                .collect()
        }
    }

    #[instrument("transactions_sequencing", level = "trace", skip_all, fields(cert_digest = ?cert.digest(), scheduling_result = tracing::field::Empty))]
    fn try_schedule(
        &self,
        cert: &VerifiedExecutableTransaction,
        commit_round: CommitRound,
        dkg_failed: bool,
        generating_randomness: bool,
        previously_deferred_tx_digests: &PreviouslyDeferredTransactions,
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
    ) -> SchedulingResult {
        // Defer transaction if it uses randomness but we aren't generating any this
        // round. Don't defer if DKG has permanently failed; in that case we
        // need to ignore.
        if !dkg_failed && !generating_randomness && cert.uses_randomness() {
            let deferred_from_round = previously_deferred_tx_digests
                .get(cert.digest())
                .map(|previous_key_suggested_gas_price_pair| {
                    previous_key_suggested_gas_price_pair
                        .0
                        .deferred_from_round()
                })
                .unwrap_or(commit_round);
            let result = SchedulingResult::Defer(
                DeferralKey::new_for_randomness(deferred_from_round),
                DeferralReason::RandomnessNotReady,
            );

            // Record the result of scheduling for tracing.
            tracing::Span::current().record("scheduling_result", tracing::field::debug(&result));
            return result;
        }

        let result = if shared_object_congestion_tracker
            .congestion_control_parameters()
            .is_congestion_control_enabled()
        {
            // Initialise the free execution slots for the objects that are not in the
            // tracker.
            let shared_input_objects = cert.shared_input_objects();
            shared_object_congestion_tracker
                .initialize_object_execution_slots(&shared_input_objects);
            // Defer transaction if it uses shared objects that are congested.
            match shared_object_congestion_tracker.try_schedule(
                cert,
                previously_deferred_tx_digests,
                commit_round,
            ) {
                SequencingResult::Defer(deferral_key, congested_objects) => {
                    SchedulingResult::Defer(
                        deferral_key,
                        DeferralReason::SharedObjectCongestion(congested_objects),
                    )
                }
                SequencingResult::Schedule(start_time) => SchedulingResult::Schedule(start_time),
            }
        } else {
            // This means shared-object congestion control is disabled.
            SchedulingResult::Schedule(0)
        };

        // Record the result of scheduling for tracing.
        tracing::Span::current().record("scheduling_result", tracing::field::debug(&result));
        result
    }

    /// Assign a sequence number for the shared objects of the input transaction
    /// based on the effects of that transaction.
    /// Used by full nodes who don't listen to consensus, and validators who
    /// catch up by state sync.
    // TODO: We should be able to pass in a vector of certs/effects and acquire them
    // all at once.
    #[instrument(level = "trace", skip_all)]
    pub fn acquire_shared_version_assignments_from_effects(
        &self,
        certificate: &VerifiedExecutableTransaction,
        effects: &TransactionEffects,
        cache_reader: &dyn ObjectCacheRead,
    ) -> IotaResult {
        let versions = SharedObjVerManager::assign_versions_from_effects(
            &[(certificate, effects)],
            self,
            cache_reader,
        );
        self.set_assigned_shared_object_versions(versions);
        Ok(())
    }

    /// When submitting a certificate caller **must** provide a ReconfigState
    /// lock guard and verify that it allows new user certificates
    pub fn insert_pending_consensus_transactions(
        &self,
        transactions: &[ConsensusTransaction],
        lock: Option<&RwLockReadGuard<ReconfigState>>,
    ) -> IotaResult {
        let key_value_pairs = transactions.iter().map(|tx| (tx.key(), tx));
        self.tables()?
            .pending_consensus_transactions
            .multi_insert(key_value_pairs)?;

        // TODO: lock once for all insert() calls.
        for transaction in transactions {
            if let ConsensusTransactionKind::CertifiedTransaction(cert) = &transaction.kind {
                let state = lock.expect("Must pass reconfiguration lock when storing certificate");
                // Caller is responsible for performing graceful check
                assert!(
                    state.should_accept_user_certs(),
                    "Reconfiguration state should allow accepting user transactions"
                );
                self.pending_consensus_certificates
                    .write()
                    .insert(*cert.digest());
            }
        }
        Ok(())
    }

    pub fn remove_pending_consensus_transactions(
        &self,
        keys: &[ConsensusTransactionKey],
    ) -> IotaResult {
        self.tables()?
            .pending_consensus_transactions
            .multi_remove(keys)?;
        // TODO: lock once for all remove() calls.
        for key in keys {
            if let ConsensusTransactionKey::Certificate(cert) = key {
                self.pending_consensus_certificates.write().remove(cert);
            }
        }
        Ok(())
    }

    pub fn pending_consensus_certificates_count(&self) -> usize {
        self.pending_consensus_certificates.read().len()
    }

    pub fn pending_consensus_certificates_empty(&self) -> bool {
        self.pending_consensus_certificates.read().is_empty()
    }

    pub fn pending_consensus_certificates(&self) -> HashSet<TransactionDigest> {
        self.pending_consensus_certificates.read().clone()
    }

    pub fn is_pending_consensus_certificate(&self, tx_digest: &TransactionDigest) -> bool {
        self.pending_consensus_certificates
            .read()
            .contains(tx_digest)
    }

    pub fn deferred_transactions_empty(&self) -> bool {
        if self
            .protocol_config
            .congestion_control_gas_price_feedback_mechanism()
        {
            self.consensus_output_cache
                .deferred_transactions_v2
                .lock()
                .is_empty()
        } else {
            self.consensus_output_cache
                .deferred_transactions
                .lock()
                .is_empty()
        }
    }

    /// Check whether any certificates were processed by consensus.
    /// This handles multiple certificates at once.
    pub fn is_any_tx_certs_consensus_message_processed<'a>(
        &self,
        certificates: impl Iterator<Item = &'a CertifiedTransaction>,
    ) -> IotaResult<bool> {
        let keys = certificates.map(|cert| {
            SequencedConsensusTransactionKey::External(ConsensusTransactionKey::Certificate(
                *cert.digest(),
            ))
        });
        Ok(self
            .check_consensus_messages_processed(keys)?
            .into_iter()
            .any(|processed| processed))
    }

    /// Check whether any certificates were processed by consensus.
    /// This handles multiple certificates at once.
    pub fn is_all_tx_certs_consensus_message_processed<'a>(
        &self,
        certificates: impl Iterator<Item = &'a VerifiedCertificate>,
    ) -> IotaResult<bool> {
        let keys = certificates.map(|cert| {
            SequencedConsensusTransactionKey::External(ConsensusTransactionKey::Certificate(
                *cert.digest(),
            ))
        });
        Ok(self
            .check_consensus_messages_processed(keys)?
            .into_iter()
            .all(|processed| processed))
    }

    pub fn is_consensus_message_processed(
        &self,
        key: &SequencedConsensusTransactionKey,
    ) -> IotaResult<bool> {
        Ok(self
            .consensus_quarantine
            .read()
            .is_consensus_message_processed(key)
            || self
                .tables()?
                .consensus_message_processed
                .contains_key(key)?)
    }

    pub fn check_consensus_messages_processed(
        &self,
        keys: impl Iterator<Item = SequencedConsensusTransactionKey>,
    ) -> IotaResult<Vec<bool>> {
        let keys = keys.collect::<Vec<_>>();

        let consensus_quarantine = self.consensus_quarantine.read();
        let tables = self.tables()?;

        Ok(do_fallback_lookup(
            &keys,
            |key| {
                if consensus_quarantine.is_consensus_message_processed(key) {
                    CacheResult::Hit(true)
                } else {
                    CacheResult::Miss
                }
            },
            |keys| {
                tables
                    .consensus_message_processed
                    .multi_contains_keys(keys)
                    .expect("db error")
            },
        ))
    }

    /// Notifies the epoch store that the specified consensus messages have been
    /// processed.
    pub async fn consensus_messages_processed_notify(
        &self,
        keys: Vec<SequencedConsensusTransactionKey>,
    ) -> Result<(), IotaError> {
        let registrations = self.consensus_notify_read.register_all(&keys);

        let unprocessed_keys_registrations = registrations
            .into_iter()
            .zip(self.check_consensus_messages_processed(keys.into_iter())?)
            .filter(|(_, processed)| !processed)
            .map(|(registration, _)| registration);

        join_all(unprocessed_keys_registrations).await;
        Ok(())
    }

    /// Get notified when transactions get executed as part of a checkpoint
    /// execution.
    pub async fn transactions_executed_in_checkpoint_notify(
        &self,
        digests: Vec<TransactionDigest>,
    ) -> Result<(), IotaError> {
        let registrations = self
            .executed_transactions_to_checkpoint_notify_read
            .register_all(&digests);

        let unprocessed_keys_registrations = registrations
            .into_iter()
            .zip(self.transactions_executed_in_checkpoint(digests.into_iter())?)
            .filter(|(_, processed)| !*processed)
            .map(|(registration, _)| registration);

        join_all(unprocessed_keys_registrations).await;
        Ok(())
    }

    /// Wait for the given transactions to be included in a checkpoint, with a
    /// timeout.
    ///
    /// Returns a vec parallel to `digests` with `Some((seq, timestamp_ms))` for
    /// each transaction that was checkpointed within the timeout, and `None`
    /// for any that were not.
    ///
    /// For transactions not yet checkpointed, the `(seq, timestamp_ms)` comes
    /// directly from the notification. For transactions already in the DB,
    /// the seq comes from the DB table and `get_timestamp` is called to
    /// resolve the timestamp for each unique checkpoint.
    pub async fn wait_for_transactions_in_checkpoint_with_timeout(
        &self,
        digests: &[TransactionDigest],
        timeout: std::time::Duration,
        mut get_timestamp: impl FnMut(CheckpointSequenceNumber) -> u64,
    ) -> IotaResult<Vec<Option<(CheckpointSequenceNumber, u64)>>> {
        // First register for notifications and read the DB in afterwards, to avoid a
        // race where a transaction gets checkpointed after we read the DB but
        // before we register for notifications.
        let registrations = self
            .executed_transactions_to_checkpoint_notify_read
            .register_all(digests);

        // Now read the DB to see if any of the transactions were already checkpointed
        // before we registered. For any that were, we can resolve the timestamp
        // immediately via the callback. For any that weren't, we will wait for the
        // notification to fire, which guarantees that the timestamp is included.
        let already_checkpointed: Vec<Option<CheckpointSequenceNumber>> = self
            .tables()?
            .executed_transactions_to_checkpoint
            .multi_get(digests)?;

        let mut results: Vec<Option<(CheckpointSequenceNumber, u64)>> = vec![None; digests.len()];
        let mut pending = Vec::new();

        for (i, (registration, existing)) in registrations
            .into_iter()
            .zip(already_checkpointed)
            .enumerate()
        {
            if let Some(seq) = existing {
                // Transaction was already checkpointed before we started waiting.
                // The notification has already fired so the registration won't
                // resolve. Resolve the timestamp via the callback.
                results[i] = Some((seq, get_timestamp(seq)));
            } else {
                pending.push((i, registration));
            }
        }

        // Await pending notifications concurrently. Collect results as they
        // arrive until the deadline.
        if !pending.is_empty() {
            let deadline = tokio::time::sleep(timeout);
            tokio::pin!(deadline);

            let mut in_flight: FuturesUnordered<_> = pending
                .into_iter()
                .map(|(i, reg)| async move { (i, reg.await) })
                .collect();

            loop {
                tokio::select! {
                    Some((i, seq_and_ts)) = in_flight.next() => {
                        results[i] = Some(seq_and_ts);
                    }
                    () = &mut deadline => {
                        break;
                    }
                    else => {
                        // All futures completed before the deadline.
                        break;
                    }
                }
            }
        }

        Ok(results)
    }

    pub fn has_sent_end_of_publish(&self, authority: &AuthorityName) -> IotaResult<bool> {
        Ok(self
            .end_of_publish
            .try_lock()
            .expect("No contention on end_of_publish lock")
            .contains_key(authority))
    }

    // Converts transaction keys to digests, waiting for digests to become available
    // for any non-digest keys.
    pub async fn notify_read_executed_digests(
        &self,
        keys: &[TransactionKey],
    ) -> IotaResult<Vec<TransactionDigest>> {
        let non_digest_keys: Vec<_> = keys
            .iter()
            .filter_map(|key| {
                if matches!(key, TransactionKey::Digest(_)) {
                    None
                } else {
                    Some(*key)
                }
            })
            .collect();

        let registrations = self
            .executed_digests_notify_read
            .register_all(&non_digest_keys);
        let executed_digests = self
            .tables()?
            .transaction_key_to_digest
            .multi_get(&non_digest_keys)?;
        let futures = executed_digests
            .into_iter()
            .zip(registrations)
            .map(|(d, r)| match d {
                // Note that Some() clause also drops registration that is already fulfilled
                Some(ready) => Either::Left(futures::future::ready(ready)),
                None => Either::Right(r),
            });
        let mut results = VecDeque::from(join_all(futures).await);

        Ok(keys
            .iter()
            .map(|key| {
                if let TransactionKey::Digest(digest) = key {
                    *digest
                } else {
                    results
                        .pop_front()
                        .expect("number of returned results should match number of non-digest keys")
                }
            })
            .collect())
    }

    /// Note: caller usually need to call consensus_message_processed_notify
    /// before this call
    pub fn user_signatures_for_checkpoint(
        &self,
        transactions: &[VerifiedTransaction],
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Vec<GenericSignature>>> {
        assert_eq!(transactions.len(), digests.len());

        let signatures: Vec<_> = {
            let mut user_sigs = self
                .consensus_output_cache
                .user_signatures_for_checkpoints
                .lock();
            digests.iter().map(|d| user_sigs.remove(d)).collect()
        };

        let mut result = Vec::with_capacity(digests.len());
        for (signatures, transaction) in signatures.into_iter().zip(transactions.iter()) {
            let signatures = if let Some(signatures) = signatures {
                signatures
            } else if matches!(
                transaction.inner().transaction_data().kind(),
                TransactionKind::RandomnessStateUpdate(_)
            ) {
                // RandomnessStateUpdate transactions don't go through consensus, but
                // have system-generated signatures that are guaranteed to be the same,
                // so we can just pull it from the transaction.
                transaction.tx_signatures().to_vec()
            } else {
                return Err(IotaError::from(
                    format!(
                        "Can not find user signature for checkpoint for transaction {:?}",
                        transaction.key()
                    )
                    .as_str(),
                ));
            };
            result.push(signatures);
        }
        Ok(result)
    }

    pub fn clear_override_protocol_upgrade_buffer_stake(&self) -> IotaResult {
        warn!(
            epoch = ?self.epoch(),
            "clearing buffer_stake_for_protocol_upgrade_bps override"
        );
        self.tables()?
            .override_protocol_upgrade_buffer_stake
            .remove(&OVERRIDE_PROTOCOL_UPGRADE_BUFFER_STAKE_INDEX)?;
        self.update_buffer_stake_metric();
        Ok(())
    }

    pub fn set_override_protocol_upgrade_buffer_stake(&self, new_stake_bps: u64) -> IotaResult {
        warn!(
            ?new_stake_bps,
            epoch = ?self.epoch(),
            "storing buffer_stake_for_protocol_upgrade_bps override"
        );
        self.tables()?
            .override_protocol_upgrade_buffer_stake
            .insert(
                &OVERRIDE_PROTOCOL_UPGRADE_BUFFER_STAKE_INDEX,
                &new_stake_bps,
            )?;
        self.update_buffer_stake_metric();
        Ok(())
    }

    fn update_buffer_stake_metric(&self) {
        self.metrics
            .effective_buffer_stake
            .set(self.get_effective_buffer_stake_bps() as i64);
    }

    pub fn get_effective_buffer_stake_bps(&self) -> u64 {
        self.tables()
            .expect("epoch initialization should have finished")
            .override_protocol_upgrade_buffer_stake
            .get(&OVERRIDE_PROTOCOL_UPGRADE_BUFFER_STAKE_INDEX)
            .expect("force_protocol_upgrade read cannot fail")
            .tap_some(|b| warn!("using overridden buffer stake value of {}", b))
            .unwrap_or_else(|| {
                self.protocol_config()
                    .buffer_stake_for_protocol_upgrade_bps()
            })
    }

    /// Record most recently advertised capabilities of all authorities
    pub fn record_capabilities_v1(&self, capabilities: &AuthorityCapabilitiesV1) -> IotaResult {
        info!("received capabilities v1 {capabilities:?}");
        let authority = &capabilities.authority;
        let tables = self.tables()?;

        // Read-compare-write pattern assumes we are only called from the consensus
        // handler task.
        if let Some(cap) = tables.authority_capabilities_v1.get(authority)? {
            if cap.generation >= capabilities.generation {
                debug!(
                    "ignoring new capabilities {:?} in favor of previous capabilities {:?}",
                    capabilities, cap
                );
                return Ok(());
            }
        }
        tables
            .authority_capabilities_v1
            .insert(authority, capabilities)?;
        Ok(())
    }

    pub fn get_capabilities_v1(&self) -> IotaResult<Vec<AuthorityCapabilitiesV1>> {
        Ok(self
            .tables()?
            .authority_capabilities_v1
            .safe_iter()
            .map(|item| item.map(|(_, v)| v))
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn record_jwk_vote(
        &self,
        output: &mut ConsensusCommitOutput,
        round: u64,
        authority: AuthorityName,
        id: &JwkId,
        jwk: &JWK,
    ) -> IotaResult {
        info!(
            "received jwk vote from {:?} for jwk ({:?}, {:?})",
            authority.concise(),
            id,
            jwk
        );

        if !self.authenticator_state_enabled() {
            info!(
                "ignoring vote because authenticator state object does exist yet
                (it will be created at the end of this epoch)"
            );
            return Ok(());
        }

        let mut jwk_aggregator = self.jwk_aggregator.lock();

        let votes = jwk_aggregator.votes_for_authority(authority);
        if votes
            >= self
                .protocol_config()
                .max_jwk_votes_per_validator_per_epoch()
        {
            warn!(
                "validator {:?} has already voted {} times this epoch, ignoring vote",
                authority, votes,
            );
            return Ok(());
        }

        output.insert_pending_jwk(authority, id.clone(), jwk.clone());

        let key = (id.clone(), jwk.clone());
        let previously_active = jwk_aggregator.has_quorum_for_key(&key);
        let insert_result = jwk_aggregator.insert(authority, key.clone());

        if !previously_active && insert_result.is_quorum_reached() {
            info!(epoch = ?self.epoch(), ?round, jwk = ?key, "jwk became active");
            output.insert_active_jwk(round, key);
        }

        Ok(())
    }

    #[instrument(level = "trace", skip_all)]
    pub(crate) fn get_new_jwks(&self, round: u64) -> IotaResult<Vec<ActiveJwk>> {
        self.consensus_quarantine.read().get_new_jwks(self, round)
    }

    pub fn jwk_active_in_current_epoch(&self, jwk_id: &JwkId, jwk: &JWK) -> bool {
        let jwk_aggregator = self.jwk_aggregator.lock();
        jwk_aggregator.has_quorum_for_key(&(jwk_id.clone(), jwk.clone()))
    }

    pub(crate) fn get_randomness_last_round_timestamp(&self) -> IotaResult<Option<TimestampMs>> {
        if let Some(ts) = self
            .consensus_quarantine
            .read()
            .get_randomness_last_round_timestamp()
        {
            Ok(Some(ts))
        } else {
            Ok(self
                .tables()?
                .randomness_last_round_timestamp
                .get(&SINGLETON_KEY)?)
        }
    }

    #[cfg(test)]
    pub fn test_insert_user_signature(
        &self,
        digest: TransactionDigest,
        signatures: Vec<GenericSignature>,
    ) {
        self.consensus_output_cache
            .user_signatures_for_checkpoints
            .lock()
            .insert(digest, signatures);
        let key = ConsensusTransactionKey::Certificate(digest);
        let key = SequencedConsensusTransactionKey::External(key);

        let mut output = ConsensusCommitOutput::default();
        output.record_consensus_message_processed(key.clone());
        output.set_default_commit_stats_for_testing();
        self.consensus_quarantine
            .write()
            .push_consensus_output(output, self)
            .expect("push_consensus_output should not fail");
        self.consensus_notify_read.notify(&key, &());
    }

    #[cfg(test)]
    pub(crate) fn push_consensus_output_for_tests(&self, output: ConsensusCommitOutput) {
        self.consensus_quarantine
            .write()
            .push_consensus_output(output, self)
            .expect("push_consensus_output should not fail");
    }

    fn process_user_signatures<'a>(
        &self,
        certificates: impl Iterator<Item = &'a VerifiedExecutableTransaction>,
    ) {
        let sigs: Vec<_> = certificates
            .map(|certificate| (*certificate.digest(), certificate.tx_signatures().to_vec()))
            .collect();

        let mut user_sigs = self
            .consensus_output_cache
            .user_signatures_for_checkpoints
            .lock();

        user_sigs.reserve(sigs.len());
        for (digest, sigs) in sigs {
            // User signatures are written in the same batch as consensus certificate
            // processed flag, which means we won't attempt to insert this twice
            // for the same tx digest
            assert!(
                user_sigs.insert(digest, sigs).is_none(),
                "duplicate user signatures for transaction digest: {digest:?}"
            );
        }
    }

    pub fn get_reconfig_state_read_lock_guard(&self) -> RwLockReadGuard<'_, ReconfigState> {
        self.reconfig_state_mem.read()
    }

    pub fn get_reconfig_state_write_lock_guard(&self) -> RwLockWriteGuard<'_, ReconfigState> {
        self.reconfig_state_mem.write()
    }

    pub fn close_user_certs(&self, mut lock_guard: RwLockWriteGuard<'_, ReconfigState>) {
        lock_guard.close_user_certs();
        self.store_reconfig_state(&lock_guard)
            .expect("Updating reconfig state cannot fail");

        // Set epoch_close_time for metric purpose.
        let mut epoch_close_time = self.epoch_close_time.write();
        if epoch_close_time.is_none() {
            // Only update it the first time epoch is closed.
            *epoch_close_time = Some(Instant::now());

            self.user_certs_closed_notify
                .notify()
                .expect("user_certs_closed_notify called twice on same epoch store");
        }
    }

    pub async fn user_certs_closed_notify(&self) {
        self.user_certs_closed_notify.wait().await
    }

    /// Notify epoch is terminated, can only be called once on epoch store
    pub async fn epoch_terminated(&self) {
        // Notify interested tasks that epoch has ended
        self.epoch_alive_notify
            .notify()
            .expect("epoch_terminated called twice on same epoch store");
        // This `write` acts as a barrier - it waits for futures executing in
        // `within_alive_epoch` to terminate before we can continue here
        debug!("Epoch terminated - waiting for pending tasks to complete");
        *self.epoch_alive.write().await = false;
        debug!("All pending epoch tasks completed");
    }

    /// Waits for the notification about epoch termination
    pub async fn wait_epoch_terminated(&self) {
        self.epoch_alive_notify.wait().await
    }

    /// This function executes given future until epoch_terminated is called
    /// If future finishes before epoch_terminated is called, future result is
    /// returned If epoch_terminated is called before future is resolved,
    /// error is returned
    ///
    /// In addition to the early termination guarantee, this function also
    /// prevents epoch_terminated() if future is being executed.
    pub async fn within_alive_epoch<F: Future + Send>(&self, f: F) -> Result<F::Output, ()> {
        // This guard is kept in the future until it resolves, preventing
        // `epoch_terminated` to acquire a write lock
        let guard = self.epoch_alive.read().await;
        if !*guard {
            return Err(());
        }
        let terminated = self.wait_epoch_terminated().boxed();
        let f = f.boxed();
        match select(terminated, f).await {
            Either::Left((_, _f)) => Err(()),
            Either::Right((result, _)) => Ok(result),
        }
    }

    #[instrument(level = "trace", skip_all)]
    pub fn verify_transaction(&self, tx: Transaction) -> IotaResult<VerifiedTransaction> {
        self.signature_verifier
            .verify_tx(tx.data())
            .map(|_| VerifiedTransaction::new_from_verified(tx))
    }

    #[instrument(level = "trace", skip_all)]
    pub fn verify_authority_capabilities(
        &self,
        authority_capabilities: SignedAuthorityCapabilitiesV1,
    ) -> IotaResult<VerifiedAuthorityCapabilitiesV1> {
        self.signature_verifier
            .verify_authority_capabilities(&authority_capabilities)
            .map(|_| VerifiedAuthorityCapabilitiesV1::new_from_verified(authority_capabilities))
    }

    /// Verifies transaction signatures and other data
    /// Important: This function can potentially be called in parallel and you
    /// can not rely on order of transactions to perform verification
    /// If this function return an error, transaction is skipped and is not
    /// passed to handle_consensus_transaction This function returns unit
    /// error and is responsible for emitting log messages for internal errors
    fn verify_consensus_transaction(
        &self,
        transaction: SequencedConsensusTransaction,
        skipped_consensus_txns: &IntCounter,
    ) -> Option<VerifiedSequencedConsensusTransaction> {
        let _scope = monitored_scope("VerifyConsensusTransaction");
        if self
            .is_consensus_message_processed(&transaction.transaction.key())
            .expect("Storage error")
        {
            trace!(
                consensus_index=?transaction.consensus_index.transaction_index,
                tracking_id=?transaction.transaction.get_tracking_id(),
                "handle_consensus_transaction UserTransaction [skip]",
            );
            skipped_consensus_txns.inc();
            return None;
        }
        // Signatures are verified as part of the consensus payload verification in
        // IotaTxValidator
        match &transaction.transaction {
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::CertifiedTransaction(_certificate),
                ..
            }) => {}
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::CheckpointSignature(data),
                ..
            }) => {
                if transaction.sender_authority() != data.summary.auth_sig().authority {
                    warn!(
                        "CheckpointSignature authority {} does not match its author from consensus {}",
                        data.summary.auth_sig().authority,
                        transaction.certificate_author_index
                    );
                    return None;
                }
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::EndOfPublish(authority),
                ..
            }) => {
                if &transaction.sender_authority() != authority {
                    warn!(
                        "EndOfPublish authority {} does not match its author from consensus {}",
                        authority, transaction.certificate_author_index
                    );
                    return None;
                }
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::MisbehaviorReport(authority, _, _),
                ..
            }) => {
                if &transaction.sender_authority() != authority {
                    warn!(
                        "MisbehaviorReport authority {} does not match its author from consensus {}",
                        authority, transaction.certificate_author_index
                    );
                    self.scorer
                        .update_invalid_reports_count(transaction.certificate_author_index);
                    return None;
                }
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind:
                    ConsensusTransactionKind::CapabilityNotificationV1(AuthorityCapabilitiesV1 {
                        authority,
                        ..
                    }),
                ..
            }) => {
                if transaction.sender_authority() != *authority {
                    warn!(
                        "CapabilityNotificationV1 authority {} does not match its author from consensus {}",
                        authority, transaction.certificate_author_index
                    );
                    return None;
                }
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::SignedCapabilityNotificationV1(_),
                ..
            }) => {
                // Signatures are verified as part of the consensus payload
                // verification in IotaTxValidator. We don't need to check the
                // sender_authority as it's correct that it's different from the
                // authority in the notification.
                // Here we only check if tracking non-committee authority capabilities is
                // enabled.
                if !self
                    .protocol_config()
                    .track_non_committee_eligible_validators()
                {
                    return None;
                }
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::NewJWKFetched(authority, id, jwk),
                ..
            }) => {
                if transaction.sender_authority() != *authority {
                    warn!(
                        "NewJWKFetched authority {} does not match its author from consensus {}",
                        authority, transaction.certificate_author_index,
                    );
                    return None;
                }
                if !check_total_jwk_size(id, jwk) {
                    warn!(
                        "{:?} sent jwk that exceeded max size",
                        transaction.sender_authority().concise()
                    );
                    return None;
                }
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::RandomnessDkgMessage(authority, _bytes),
                ..
            }) => {
                if transaction.sender_authority() != *authority {
                    warn!(
                        "RandomnessDkgMessage authority {} does not match its author from consensus {}",
                        authority, transaction.certificate_author_index
                    );
                    return None;
                }
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::RandomnessDkgConfirmation(authority, _bytes),
                ..
            }) => {
                if transaction.sender_authority() != *authority {
                    warn!(
                        "RandomnessDkgConfirmation authority {} does not match its author from consensus {}",
                        authority, transaction.certificate_author_index
                    );
                    return None;
                }
            }
            SequencedConsensusTransactionKind::System(_) => {}
        }
        Some(VerifiedSequencedConsensusTransaction(transaction))
    }

    fn db_batch(&self) -> IotaResult<DBBatch> {
        Ok(self.tables()?.last_consensus_stats.batch())
    }

    #[cfg(test)]
    pub fn db_batch_for_test(&self) -> DBBatch {
        self.db_batch()
            .expect("test should not be write past end of epoch")
    }

    #[instrument(level = "trace", skip_all)]
    pub(crate) async fn process_consensus_transactions_and_commit_boundary<
        C: CheckpointServiceNotify,
    >(
        &self,
        transactions: Vec<SequencedConsensusTransaction>,
        consensus_stats: &ExecutionIndicesWithStats,
        checkpoint_service: &Arc<C>,
        cache_reader: &dyn ObjectCacheRead,
        tx_reader: &dyn TransactionCacheRead,
        consensus_commit_info: &ConsensusCommitInfo,
        authority_metrics: &Arc<AuthorityMetrics>,
    ) -> IotaResult<Vec<VerifiedExecutableTransaction>> {
        // Split transactions into different types for processing.
        let verified_transactions: Vec<_> = transactions
            .into_iter()
            .filter_map(|transaction| {
                self.verify_consensus_transaction(
                    transaction,
                    &authority_metrics.skipped_consensus_txns,
                )
            })
            .collect();
        let mut system_transactions = Vec::with_capacity(verified_transactions.len());
        let mut current_commit_sequenced_consensus_transactions =
            Vec::with_capacity(verified_transactions.len());
        let mut current_commit_sequenced_randomness_transactions =
            Vec::with_capacity(verified_transactions.len());
        let mut end_of_publish_transactions = Vec::with_capacity(verified_transactions.len());
        for tx in verified_transactions {
            if tx.0.is_end_of_publish() {
                end_of_publish_transactions.push(tx);
            } else if tx.0.is_system() {
                system_transactions.push(tx);
            } else if tx.0.is_user_tx_with_randomness() {
                current_commit_sequenced_randomness_transactions.push(tx);
            } else {
                current_commit_sequenced_consensus_transactions.push(tx);
            }
        }

        let mut output = ConsensusCommitOutput::new(consensus_commit_info.round);

        // Load transactions deferred from previous commits.
        let deferred_txs: Vec<(DeferralKey, Vec<DeferredTransaction>)> = self
            .load_deferred_transactions_for_up_to_consensus_round(
                &mut output,
                consensus_commit_info.round,
            )?
            .into_iter()
            .collect();
        let mut previously_deferred_tx_digests: PreviouslyDeferredTransactions = deferred_txs
            .iter()
            .flat_map(|(deferral_key, txs)| {
                txs.iter()
                    .map(|tx| match tx.transaction.0.transaction.key() {
                        SequencedConsensusTransactionKey::External(
                            ConsensusTransactionKey::Certificate(digest),
                        ) => (digest, (*deferral_key, tx.suggested_gas_price)),
                        _ => panic!("deferred transaction was not a user certificate: {tx:?}"),
                    })
            })
            .collect();

        // Sequenced_transactions and sequenced_randomness_transactions store all
        // transactions that will be sent to process_consensus_transactions. We
        // put deferred transactions at the beginning of the list before
        // PostConsensusTxReorder::reorder, so that for transactions with the same gas
        // price, deferred transactions will be placed earlier in the execution
        // queue.
        let mut sequenced_transactions: Vec<VerifiedSequencedConsensusTransaction> =
            Vec::with_capacity(
                current_commit_sequenced_consensus_transactions.len()
                    + previously_deferred_tx_digests.len(),
            );
        let mut sequenced_randomness_transactions: Vec<VerifiedSequencedConsensusTransaction> =
            Vec::with_capacity(
                current_commit_sequenced_randomness_transactions.len()
                    + previously_deferred_tx_digests.len(),
            );

        let mut randomness_manager = self.randomness_manager.get().map(|rm| {
            rm.try_lock()
                .expect("should only ever be called from the commit handler thread")
        });
        let mut dkg_failed = false;
        let randomness_round = {
            let randomness_manager = randomness_manager
                .as_mut()
                .expect("randomness manager should exist if randomness is enabled");
            match randomness_manager.dkg_status() {
                DkgStatus::Pending => None,
                DkgStatus::Failed => {
                    dkg_failed = true;
                    None
                }
                DkgStatus::Successful => {
                    // Generate randomness for this commit if DKG is successful and we are still
                    // accepting certs.
                    if self
                        // It is ok to just release lock here as functions called by this one are
                        // the only place that transition into
                        // RejectAllCerts state, and this function itself is
                        // always executed from consensus task.
                        .get_reconfig_state_read_lock_guard()
                        .should_accept_tx()
                    {
                        randomness_manager
                            .reserve_next_randomness(consensus_commit_info.timestamp, &mut output)?
                    } else {
                        None
                    }
                }
            }
        };

        // We should load any previously-deferred randomness-using tx:
        // - if DKG is failed, so we can ignore them
        // - if randomness is being generated, so we can process them
        if dkg_failed || randomness_round.is_some() {
            self.load_and_process_deferred_transactions_for_randomness(
                &mut output,
                &mut previously_deferred_tx_digests,
                &mut sequenced_randomness_transactions,
            )?;
        }

        // Add ConsensusRound deferred tx back into the sequence.
        for tx in deferred_txs
            .into_iter()
            .flat_map(|(_, txs)| txs.into_iter())
        {
            if tx.transaction.0.is_user_tx_with_randomness() {
                sequenced_randomness_transactions.push(tx.transaction);
            } else {
                sequenced_transactions.push(tx.transaction);
            }
        }
        sequenced_transactions.extend(current_commit_sequenced_consensus_transactions);
        sequenced_randomness_transactions.extend(current_commit_sequenced_randomness_transactions);

        // Save roots for checkpoint generation. One set for most tx, one for randomness
        // tx.
        let mut roots: BTreeSet<_> = system_transactions
            .iter()
            .chain(sequenced_transactions.iter())
            // no need to include end_of_publish_transactions here because they would be
            // filtered out below by `executable_transaction_digest` anyway
            .filter_map(|transaction| {
                transaction
                    .0
                    .transaction
                    .executable_transaction_digest()
                    .map(TransactionKey::Digest)
            })
            .collect();
        let mut randomness_roots: BTreeSet<_> = sequenced_randomness_transactions
            .iter()
            .filter_map(|transaction| {
                transaction
                    .0
                    .transaction
                    .executable_transaction_digest()
                    .map(TransactionKey::Digest)
            })
            .collect();

        // We always order transactions using randomness last.
        PostConsensusTxReorder::reorder(
            &mut sequenced_transactions,
            self.protocol_config.consensus_transaction_ordering(),
        );
        PostConsensusTxReorder::reorder(
            &mut sequenced_randomness_transactions,
            self.protocol_config.consensus_transaction_ordering(),
        );

        let congestion_control_parameters = CongestionControlParameters::new(&self.protocol_config);

        // We track transaction shared object congestion separately for regular
        // transactions and transactions using randomness.
        let shared_object_congestion_tracker = SharedObjectCongestionTracker::new(
            self.consensus_quarantine.read().load_initial_object_debts(
                self,
                consensus_commit_info.round,
                false,
                &sequenced_transactions,
            )?,
            congestion_control_parameters.clone(),
        );
        let shared_object_using_randomness_congestion_tracker = SharedObjectCongestionTracker::new(
            self.consensus_quarantine.read().load_initial_object_debts(
                self,
                consensus_commit_info.round,
                true,
                &sequenced_randomness_transactions,
            )?,
            congestion_control_parameters,
        );

        system_transactions.extend(sequenced_transactions);
        let sequenced_non_randomness_transactions = system_transactions;

        let (
            verified_non_randomness_transactions,
            mut verified_randomness_transactions,
            notifications,
            lock,
            final_round,
            consensus_commit_prologue_root,
        ) = self
            .process_consensus_transactions(
                &mut output,
                &sequenced_non_randomness_transactions,
                &sequenced_randomness_transactions,
                &end_of_publish_transactions,
                checkpoint_service,
                cache_reader,
                consensus_commit_info,
                &mut roots,
                &mut randomness_roots,
                previously_deferred_tx_digests,
                randomness_manager.as_deref_mut(),
                dkg_failed,
                randomness_round,
                authority_metrics,
                shared_object_congestion_tracker,
                shared_object_using_randomness_congestion_tracker,
            )
            .await?;
        self.process_user_signatures(
            verified_non_randomness_transactions
                .iter()
                .chain(verified_randomness_transactions.iter()),
        );
        output.record_consensus_commit_stats(consensus_stats.clone());

        // Create pending checkpoints if we are still accepting tx.
        let should_accept_tx = if let Some(lock) = &lock {
            lock.should_accept_tx()
        } else {
            // It is ok to just release lock here as functions called by this one are the
            // only place that transition reconfig state, and this function itself is always
            // executed from consensus task. At this point if the lock was not already
            // provided above, we know we won't be transitioning state for this
            // commit.
            self.get_reconfig_state_read_lock_guard().should_accept_tx()
        };
        let make_checkpoint = should_accept_tx || final_round;
        if make_checkpoint {
            let checkpoint_height =
                self.calculate_pending_checkpoint_height(consensus_commit_info.round);

            let mut non_randomness_roots: Vec<TransactionKey> = Vec::with_capacity(roots.len() + 1);

            if let Some(consensus_commit_prologue_root) = consensus_commit_prologue_root {
                // Put consensus commit prologue root at the beginning of the checkpoint roots.
                non_randomness_roots.push(consensus_commit_prologue_root);
            }
            non_randomness_roots.extend(roots.into_iter());

            if let Some(randomness_round) = randomness_round {
                let key = TransactionKey::RandomnessRound(self.epoch(), randomness_round);

                // During crash recovery, the randomness update transaction may already have
                // been created and executed before the crash. If it is
                // available locally, we need to ensure it is executed.
                if let Some(digest) = self.tables()?.transaction_key_to_digest.get(&key)? {
                    if let Some(tx) = tx_reader.get_transaction_block(&digest) {
                        info!(
                            "Randomness update transaction {:?} already exists, scheduling for execution",
                            digest
                        );
                        let tx =
                            VerifiedExecutableTransaction::new_system((*tx).clone(), self.epoch());
                        verified_randomness_transactions.push(tx);
                    }
                }

                randomness_roots.insert(key);
            }

            // Determine whether to write pending checkpoint for user tx with randomness.
            // - If randomness is not generated for this commit, we will skip the checkpoint
            //   with the associated height. Therefore checkpoint heights may not be
            //   contiguous.
            // - Exception: if DKG fails, we always need to write out a PendingCheckpoint
            //   for randomness tx that are canceled.
            let should_write_random_checkpoint =
                randomness_round.is_some() || (dkg_failed && !randomness_roots.is_empty());

            let pending_checkpoint = PendingCheckpoint::V1(PendingCheckpointContentsV1 {
                roots: non_randomness_roots,
                details: PendingCheckpointInfo {
                    timestamp_ms: consensus_commit_info.timestamp,
                    last_of_epoch: final_round && !should_write_random_checkpoint,
                    checkpoint_height,
                },
            });
            self.write_pending_checkpoint(&mut output, &pending_checkpoint)?;

            if should_write_random_checkpoint {
                let pending_checkpoint = PendingCheckpoint::V1(PendingCheckpointContentsV1 {
                    roots: randomness_roots.into_iter().collect(),
                    details: PendingCheckpointInfo {
                        timestamp_ms: consensus_commit_info.timestamp,
                        last_of_epoch: final_round,
                        checkpoint_height: checkpoint_height + 1,
                    },
                });
                self.write_pending_checkpoint(&mut output, &pending_checkpoint)?;
            }
        }

        {
            if self
                .protocol_config
                .congestion_control_gas_price_feedback_mechanism()
            {
                let mut deferred_transactions =
                    self.consensus_output_cache.deferred_transactions_v2.lock();
                for deleted_deferred_key in output.get_deleted_deferred_txn_keys() {
                    deferred_transactions.remove(&deleted_deferred_key);
                }
            } else {
                let mut deferred_transactions =
                    self.consensus_output_cache.deferred_transactions.lock();
                for deleted_deferred_key in output.get_deleted_deferred_txn_keys() {
                    deferred_transactions.remove(&deleted_deferred_key);
                }
            }
        }

        // Write details of the this consensus commit to consensus output quarantine.
        self.consensus_quarantine
            .write()
            .push_consensus_output(output, self)?;

        // Only after batch is written, notify checkpoint service to start building any
        // new pending checkpoints.
        if make_checkpoint {
            debug!(
                ?consensus_commit_info.round,
                "Notifying checkpoint service about new pending checkpoint(s)",
            );
            checkpoint_service.notify_checkpoint()?;
        }

        // Once commit processing is recorded, kick off randomness generation.
        if let Some(randomness_round) = randomness_round {
            let epoch = self.epoch();
            randomness_manager
                .as_ref()
                .expect("randomness manager should exist if randomness round is provided")
                .generate_randomness(epoch, randomness_round);
        }

        self.process_notifications(&notifications, &end_of_publish_transactions);

        if final_round {
            info!(
                epoch=?self.epoch(),
                // Accessing lock on purpose so that the compiler ensures
                // the lock is not yet dropped.
                lock=?lock.as_ref(),
                final_round=?final_round,
                "Notified last checkpoint"
            );
            self.record_end_of_message_quorum_time_metric();
        }

        Ok([
            verified_non_randomness_transactions,
            verified_randomness_transactions,
        ]
        .concat())
    }

    fn calculate_pending_checkpoint_height(&self, consensus_round: u64) -> u64 {
        consensus_round * 2
    }

    // Adds the consensus commit prologue transaction to the beginning of input
    // `transactions` to update the system clock used in all transactions in the
    // current consensus commit. Returns the root of the consensus commit
    // prologue transaction if it was added to the input.
    fn add_consensus_commit_prologue_transaction(
        &self,
        output: &mut ConsensusCommitOutput,
        transactions: &mut VecDeque<VerifiedExecutableTransaction>,
        consensus_commit_info: &ConsensusCommitInfo,
        cancelled_txns: &BTreeMap<TransactionDigest, CancelConsensusCertificateReason>,
    ) -> IotaResult<Option<TransactionKey>> {
        {
            if consensus_commit_info.skip_consensus_commit_prologue_in_test() {
                return Ok(None);
            }
        }

        let mut version_assignment: Vec<(TransactionDigest, Vec<(ObjectID, SequenceNumber)>)> =
            Vec::new();

        let mut shared_input_next_version = HashMap::new();
        for txn in transactions.iter() {
            match cancelled_txns.get(txn.digest()) {
                Some(CancelConsensusCertificateReason::CongestionOnObjects { .. })
                | Some(CancelConsensusCertificateReason::DkgFailed) => {
                    let assigned_versions = SharedObjVerManager::assign_versions_for_certificate(
                        txn,
                        &mut shared_input_next_version,
                        cancelled_txns,
                        self.protocol_config
                            .congestion_control_gas_price_feedback_mechanism(),
                    );
                    version_assignment.push((*txn.digest(), assigned_versions));
                }
                None => {}
            }
        }

        fail_point_arg!(
            "additional_cancelled_txns_for_tests",
            |additional_cancelled_txns: Vec<(
                TransactionDigest,
                Vec<(ObjectID, SequenceNumber)>
            )>| {
                version_assignment.extend(additional_cancelled_txns);
            }
        );

        let transaction = consensus_commit_info
            .create_consensus_commit_prologue_transaction(self.epoch(), version_assignment);
        let consensus_commit_prologue_root = match self
            .process_consensus_system_transaction(&transaction)
        {
            ConsensusCertificateResult::Scheduled {
                transaction,
                start_time: _,
            } => {
                transactions.push_front(transaction.clone());
                Some(transaction.key())
            }
            ConsensusCertificateResult::IgnoredSystem => None,
            _ => unreachable!(
                "process_consensus_system_transaction returned unexpected ConsensusCertificateResult."
            ),
        };

        output.record_consensus_message_processed(SequencedConsensusTransactionKey::System(
            *transaction.digest(),
        ));
        Ok(consensus_commit_prologue_root)
    }

    // Assigns shared object versions to transactions and updates the shared object
    // version state. Shared object versions in cancelled transactions are
    // assigned to special versions that will cause the transactions to be
    // cancelled in execution engine.
    fn process_consensus_transaction_shared_object_versions(
        &self,
        cache_reader: &dyn ObjectCacheRead,
        non_randomness_transactions: &[VerifiedExecutableTransaction],
        randomness_transactions: &[VerifiedExecutableTransaction],
        randomness_round: Option<RandomnessRound>,
        cancelled_txns: &BTreeMap<TransactionDigest, CancelConsensusCertificateReason>,
        output: &mut ConsensusCommitOutput,
    ) -> IotaResult {
        // If randomness_round is set, we know that eventually there will be a
        // randomness state update transaction. We create a placeholder
        // transaction so that the SharedObjVerManager can update the version of
        // the randomness state object and use that version for randomness transactions.
        let randomness_state_update = randomness_round.map(|round| {
            VerifiedExecutableTransaction::new_system(
                VerifiedTransaction::new_randomness_state_update(
                    self.epoch(),
                    round,
                    // This is placeholder bytes, since this transaction does not exist yet.
                    vec![],
                    self.epoch_start_config()
                        .randomness_obj_initial_shared_version(),
                ),
                self.epoch(),
            )
        });
        let all_certs = non_randomness_transactions
            .iter()
            // randomness_state_update must be before randomness_transactions to make sure the
            // version of the randomness state object is updated before it is used in
            // randomness transactions.
            .chain(randomness_state_update.iter())
            .chain(randomness_transactions.iter());
        let ConsensusSharedObjVerAssignment {
            shared_input_next_versions,
            assigned_versions,
        } = SharedObjVerManager::assign_versions_from_consensus(
            self,
            cache_reader,
            all_certs,
            cancelled_txns,
        )?;

        self.consensus_output_cache
            .insert_shared_object_assignments(&assigned_versions);

        output.set_next_shared_object_versions(shared_input_next_versions);
        Ok(())
    }

    pub fn get_highest_pending_checkpoint_height(&self) -> CheckpointHeight {
        self.consensus_quarantine
            .read()
            .get_highest_pending_checkpoint_height()
            .unwrap_or_default()
    }

    // Caller is not required to set ExecutionIndices with the right semantics in
    // VerifiedSequencedConsensusTransaction.
    // Also, ConsensusStats and hash will not be updated in the db with this
    // function, unlike in process_consensus_transactions_and_commit_boundary().
    pub async fn process_consensus_transactions_for_tests<C: CheckpointServiceNotify>(
        self: &Arc<Self>,
        transactions: Vec<SequencedConsensusTransaction>,
        checkpoint_service: &Arc<C>,
        cache_reader: &dyn ObjectCacheRead,
        tx_reader: &dyn TransactionCacheRead,
        authority_metrics: &Arc<AuthorityMetrics>,
        skip_consensus_commit_prologue_in_test: bool,
    ) -> IotaResult<Vec<VerifiedExecutableTransaction>> {
        self.process_consensus_transactions_and_commit_boundary(
            transactions,
            &ExecutionIndicesWithStats::default(),
            checkpoint_service,
            cache_reader,
            tx_reader,
            &ConsensusCommitInfo::new_for_test(
                self.get_highest_pending_checkpoint_height() / 2 + 1,
                0,
                skip_consensus_commit_prologue_in_test,
            ),
            authority_metrics,
        )
        .await
    }

    pub fn assign_shared_object_versions_for_tests(
        self: &Arc<Self>,
        cache_reader: &dyn ObjectCacheRead,
        transactions: &[VerifiedExecutableTransaction],
    ) -> IotaResult {
        let mut output = ConsensusCommitOutput::new(0);
        self.process_consensus_transaction_shared_object_versions(
            cache_reader,
            transactions,
            &[],
            None,
            &BTreeMap::new(),
            &mut output,
        )?;
        let mut batch = self.db_batch()?;
        output.set_default_commit_stats_for_testing();
        output.write_to_batch(self, &mut batch)?;
        batch.write()?;
        Ok(())
    }

    fn process_notifications(
        &self,
        notifications: &[SequencedConsensusTransactionKey],
        end_of_publish: &[VerifiedSequencedConsensusTransaction],
    ) {
        for key in notifications
            .iter()
            .cloned()
            .chain(end_of_publish.iter().map(|tx| tx.0.transaction.key()))
        {
            self.consensus_notify_read.notify(&key, &());
        }
    }

    /// Depending on the type of the VerifiedSequencedConsensusTransaction
    /// wrappers,
    /// - Verify and initialize the state to execute the certificates. Return
    ///   VerifiedCertificates for each executable certificate
    /// - Or update the state for checkpoint or epoch change protocol.
    #[instrument("process_consensus_transactions", level = "trace", skip_all)]
    #[expect(clippy::type_complexity)]
    pub(crate) async fn process_consensus_transactions<C: CheckpointServiceNotify>(
        &self,
        output: &mut ConsensusCommitOutput,
        non_randomness_transactions: &[VerifiedSequencedConsensusTransaction],
        randomness_transactions: &[VerifiedSequencedConsensusTransaction],
        end_of_publish_transactions: &[VerifiedSequencedConsensusTransaction],
        checkpoint_service: &Arc<C>,
        cache_reader: &dyn ObjectCacheRead,
        consensus_commit_info: &ConsensusCommitInfo,
        non_randomness_roots: &mut BTreeSet<TransactionKey>,
        randomness_roots: &mut BTreeSet<TransactionKey>,
        previously_deferred_tx_digests: PreviouslyDeferredTransactions,
        mut randomness_manager: Option<&mut RandomnessManager>,
        dkg_failed: bool,
        randomness_round: Option<RandomnessRound>,
        authority_metrics: &Arc<AuthorityMetrics>,
        mut shared_object_congestion_tracker: SharedObjectCongestionTracker,
        mut shared_object_using_randomness_congestion_tracker: SharedObjectCongestionTracker,
    ) -> IotaResult<(
        Vec<VerifiedExecutableTransaction>, // non-randomness transactions to schedule
        Vec<VerifiedExecutableTransaction>, // randomness transactions to schedule
        Vec<SequencedConsensusTransactionKey>, // keys to notify as complete
        Option<RwLockWriteGuard<'_, ReconfigState>>,
        bool,                   // true if final round
        Option<TransactionKey>, // consensus commit prologue root
    )> {
        if randomness_round.is_some() {
            assert!(!dkg_failed); // invariant check
        }

        let mut notifications =
            Vec::with_capacity(non_randomness_transactions.len() + randomness_transactions.len());

        let mut deferred_txns: BTreeMap<DeferralKey, Vec<DeferredTransaction>> = BTreeMap::new();
        let mut cancelled_txns: BTreeMap<TransactionDigest, CancelConsensusCertificateReason> =
            BTreeMap::new();

        fail_point_arg!(
            "initial_congestion_tracker",
            |tracker: SharedObjectCongestionTracker| {
                info!(
                    "Initialize shared_object_congestion_tracker to {:?}",
                    tracker
                );
                shared_object_congestion_tracker = tracker;
            }
        );

        let mut suggested_gas_price_calculator = SuggestedGasPriceCalculator::new(
            shared_object_congestion_tracker
                .congestion_control_parameters()
                .clone(),
            self.reference_gas_price(),
        );
        let mut suggested_gas_price_calculator_for_randomness = SuggestedGasPriceCalculator::new(
            shared_object_using_randomness_congestion_tracker
                .congestion_control_parameters()
                .clone(),
            self.reference_gas_price(),
        );

        fail_point_arg!(
            "initial_suggested_gas_price_calculator",
            |calculator: SuggestedGasPriceCalculator| {
                info!(
                    "Initialize suggested_gas_price_calculator to {:?}",
                    calculator
                );
                suggested_gas_price_calculator = calculator;
            }
        );

        let mut randomness_state_updated = false;
        let mut sequenced_non_randomness = Vec::new();
        let mut sequenced_randomness = Vec::new();

        for entry in non_randomness_transactions
            .iter()
            .map(Either::Left)
            .chain(randomness_transactions.iter().map(Either::Right))
        {
            let (tx, congestion_tracker, sgp_calculator, sequenced_txns) = match entry {
                Either::Left(tx) => (
                    tx,
                    &mut shared_object_congestion_tracker,
                    &mut suggested_gas_price_calculator,
                    &mut sequenced_non_randomness,
                ),
                Either::Right(tx) => (
                    tx,
                    &mut shared_object_using_randomness_congestion_tracker,
                    if self
                        .protocol_config
                        .separate_gas_price_feedback_mechanism_for_randomness()
                    {
                        &mut suggested_gas_price_calculator_for_randomness
                    } else {
                        &mut suggested_gas_price_calculator
                    },
                    &mut sequenced_randomness,
                ),
            };
            let key = tx.0.transaction.key();
            let mut ignored = false;
            let mut filter_roots = false;
            match self
                .process_consensus_transaction(
                    output,
                    tx,
                    checkpoint_service,
                    consensus_commit_info.round,
                    &previously_deferred_tx_digests,
                    randomness_manager.as_deref_mut(),
                    dkg_failed,
                    randomness_round.is_some(),
                    congestion_tracker,
                    sgp_calculator,
                    authority_metrics,
                )
                .await?
            {
                ConsensusCertificateResult::Scheduled {
                    transaction,
                    start_time,
                } => {
                    notifications.push(key.clone());
                    sequenced_txns.push((transaction, start_time));
                }
                ConsensusCertificateResult::Deferred {
                    deferral_key,
                    suggested_gas_price,
                } => {
                    // Note: record_consensus_message_processed() must be called for this
                    // cert even though we are not processing it now!
                    deferred_txns
                        .entry(deferral_key)
                        .or_default()
                        .push(DeferredTransaction::new(tx.clone(), suggested_gas_price));
                    filter_roots = true;
                    if tx.0.transaction.is_executable_transaction() {
                        // Notify consensus adapter that the consensus handler has received the
                        // transaction.
                        notifications.push(key.clone());
                    }
                }
                ConsensusCertificateResult::Cancelled((cert, reason)) => {
                    notifications.push(key.clone());
                    assert!(cancelled_txns.insert(*cert.digest(), reason).is_none());
                    sequenced_txns.push((cert, congestion_tracker.max_occupied_slot_end_time()));
                }
                ConsensusCertificateResult::RandomnessConsensusMessage => {
                    randomness_state_updated = true;
                    notifications.push(key.clone());
                }
                ConsensusCertificateResult::ConsensusMessage => notifications.push(key.clone()),
                ConsensusCertificateResult::IgnoredSystem => {
                    filter_roots = true;
                }
                // Note: ignored external transactions must not be recorded as processed. Otherwise
                // they may not get reverted after restart during epoch change.
                ConsensusCertificateResult::Ignored => {
                    ignored = true;
                    filter_roots = true;
                }
            }
            if !ignored {
                output.record_consensus_message_processed(key.clone());
            }
            if filter_roots {
                if let Some(txn_key) =
                    tx.0.transaction
                        .executable_transaction_digest()
                        .map(TransactionKey::Digest)
                {
                    non_randomness_roots.remove(&txn_key);
                    randomness_roots.remove(&txn_key);
                }
            }
        }

        // sort the sequenced transactions based on their start_time from the
        // sequencing result and add these to the verified_certificates.
        sequenced_non_randomness.sort_by_key(|(_, start_time)| *start_time);
        let mut verified_non_randomness_certificates: VecDeque<_> = sequenced_non_randomness
            .into_iter()
            .map(|(tx, _)| tx)
            .collect();
        sequenced_randomness.sort_by_key(|(_, start_time)| *start_time);
        let verified_randomness_certificates: VecDeque<_> =
            sequenced_randomness.into_iter().map(|(tx, _)| tx).collect();
        let commit_has_deferred_txns = !deferred_txns.is_empty();
        let mut total_deferred_txns = 0;
        {
            if self
                .protocol_config
                .congestion_control_gas_price_feedback_mechanism()
            {
                let mut deferred_transactions =
                    self.consensus_output_cache.deferred_transactions_v2.lock();
                for (key, txns) in deferred_txns.into_iter() {
                    total_deferred_txns += txns.len();
                    deferred_transactions.insert(key, txns.clone());
                    output.defer_transactions(key, txns);
                }
            } else {
                let mut deferred_transactions =
                    self.consensus_output_cache.deferred_transactions.lock();

                for (key, txns) in deferred_txns.into_iter() {
                    total_deferred_txns += txns.len();
                    deferred_transactions.insert(
                        key,
                        txns.iter()
                            .map(|tx| tx.transaction.clone())
                            .collect::<Vec<_>>(),
                    );
                    output.defer_transactions(key, txns);
                }
            }
        }

        authority_metrics
            .consensus_handler_deferred_transactions
            .inc_by(total_deferred_txns as u64);
        authority_metrics
            .consensus_handler_cancelled_transactions
            .inc_by(cancelled_txns.len() as u64);
        authority_metrics
            .consensus_handler_max_object_costs
            .with_label_values(&["regular_commit"])
            .set(shared_object_congestion_tracker.max_occupied_slot_end_time() as i64);
        authority_metrics
            .consensus_handler_max_object_costs
            .with_label_values(&["randomness_commit"])
            .set(
                shared_object_using_randomness_congestion_tracker.max_occupied_slot_end_time()
                    as i64,
            );

        // Record accumulated debts from this consensus commit following sequencing.
        // This output will be written to consensus quarantine so the debts can be
        // loaded in the future consensus commit rounds where the objects are involved.
        if let Some(max_execution_duration_per_commit) = shared_object_congestion_tracker
            .congestion_control_parameters()
            .max_execution_duration_per_commit()
        {
            output.set_congestion_control_object_debts(
                shared_object_congestion_tracker
                    .accumulated_debts(max_execution_duration_per_commit),
            );
            output.set_congestion_control_randomness_object_debts(
                shared_object_using_randomness_congestion_tracker
                    .accumulated_debts(max_execution_duration_per_commit),
            );
        }

        if randomness_state_updated {
            if let Some(randomness_manager) = randomness_manager.as_mut() {
                randomness_manager
                    .advance_dkg(output, consensus_commit_info.round)
                    .await?;
            }
        }

        // Add the consensus commit prologue transaction to the beginning of
        // `verified_non_randomness_certificates`.
        let consensus_commit_prologue_root = self.add_consensus_commit_prologue_transaction(
            output,
            &mut verified_non_randomness_certificates,
            consensus_commit_info,
            &cancelled_txns,
        )?;

        let verified_non_randomness_certificates: Vec<_> =
            verified_non_randomness_certificates.into();
        let verified_randomness_certificates: Vec<_> = verified_randomness_certificates.into();

        self.process_consensus_transaction_shared_object_versions(
            cache_reader,
            &verified_non_randomness_certificates,
            &verified_randomness_certificates,
            randomness_round,
            &cancelled_txns,
            output,
        )?;

        let (lock, final_round) = self.process_end_of_publish_transactions_and_reconfig(
            output,
            end_of_publish_transactions,
            commit_has_deferred_txns,
        )?;

        Ok((
            verified_non_randomness_certificates,
            verified_randomness_certificates,
            notifications,
            lock,
            final_round,
            consensus_commit_prologue_root,
        ))
    }

    fn process_end_of_publish_transactions_and_reconfig(
        &self,
        output: &mut ConsensusCommitOutput,
        transactions: &[VerifiedSequencedConsensusTransaction],
        commit_has_deferred_txns: bool,
    ) -> IotaResult<(
        Option<RwLockWriteGuard<'_, ReconfigState>>,
        bool, // true if final round
    )> {
        let mut lock = None;

        for transaction in transactions {
            let VerifiedSequencedConsensusTransaction(SequencedConsensusTransaction {
                transaction,
                ..
            }) = transaction;

            if let SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::EndOfPublish(authority),
                ..
            }) = transaction
            {
                debug!(
                    "Received EndOfPublish for epoch {} from {:?}",
                    self.committee.epoch,
                    authority.concise()
                );

                // It is ok to just release lock here as this function is the only place that
                // transition into RejectAllCerts state And this function itself
                // is always executed from consensus task
                let collected_end_of_publish = if lock.is_none()
                    && self
                        .get_reconfig_state_read_lock_guard()
                        .should_accept_consensus_certs()
                {
                    output.insert_end_of_publish(*authority);
                    self.end_of_publish.try_lock()
                        .expect("No contention on Authority::end_of_publish as it is only accessed from consensus handler")
                        .insert_generic(*authority, ()).is_quorum_reached()
                    // end_of_publish lock is released here.
                } else {
                    // If we past the stage where we are accepting consensus certificates we also
                    // don't record end of publish messages
                    debug!(
                        "Ignoring end of publish message from validator {:?} as we already collected enough end of publish messages",
                        authority.concise()
                    );
                    false
                };

                if collected_end_of_publish {
                    assert!(lock.is_none());
                    debug!(
                        "Collected enough end_of_publish messages for epoch {} with last message from validator {:?}",
                        self.committee.epoch,
                        authority.concise(),
                    );
                    let mut l = self.get_reconfig_state_write_lock_guard();
                    l.close_all_certs();
                    output.store_reconfig_state(l.clone());
                    // Holding this lock until end of
                    // process_consensus_transactions_and_commit_boundary() where we write batch to
                    // DB
                    lock = Some(l);
                };
                // Important: we actually rely here on fact that ConsensusHandler panics if its
                // operation returns error. If some day we won't panic in ConsensusHandler on
                // error we need to figure out here how to revert in-memory
                // state of .end_of_publish and .reconfig_state when write
                // fails.
                output.record_consensus_message_processed(transaction.key());
            } else {
                panic!(
                    "process_end_of_publish_transactions_and_reconfig called with non-end-of-publish transaction"
                );
            }
        }

        // Determine if we're ready to advance reconfig state to RejectAllTx.
        let is_reject_all_certs = if let Some(lock) = &lock {
            lock.is_reject_all_certs()
        } else {
            // It is ok to just release lock here as this function is the only place that
            // transitions into RejectAllTx state, and this function itself is always
            // executed from consensus task.
            self.get_reconfig_state_read_lock_guard()
                .is_reject_all_certs()
        };

        if !is_reject_all_certs || !self.deferred_transactions_empty() || commit_has_deferred_txns {
            // Don't end epoch until all deferred transactions are processed.
            if is_reject_all_certs {
                debug!(
                    "Blocking end of epoch on deferred transactions, from previous commits?={}, from this commit?={commit_has_deferred_txns}",
                    !self.deferred_transactions_empty(),
                );
            }
            return Ok((lock, false));
        }

        // Acquire lock to advance state if we don't already have it.
        let mut lock = lock.unwrap_or_else(|| self.get_reconfig_state_write_lock_guard());
        lock.close_all_tx();
        output.store_reconfig_state(lock.clone());
        Ok((Some(lock), true))
    }

    #[instrument(level = "trace", skip_all)]
    async fn process_consensus_transaction<C: CheckpointServiceNotify>(
        &self,
        output: &mut ConsensusCommitOutput,
        transaction: &VerifiedSequencedConsensusTransaction,
        checkpoint_service: &Arc<C>,
        commit_round: CommitRound,
        previously_deferred_tx_digests: &PreviouslyDeferredTransactions,
        mut randomness_manager: Option<&mut RandomnessManager>,
        dkg_failed: bool,
        generating_randomness: bool,
        shared_object_congestion_tracker: &mut SharedObjectCongestionTracker,
        suggested_gas_price_calculator: &mut SuggestedGasPriceCalculator,
        authority_metrics: &Arc<AuthorityMetrics>,
    ) -> IotaResult<ConsensusCertificateResult> {
        let _scope = monitored_scope("HandleConsensusTransaction");
        let VerifiedSequencedConsensusTransaction(SequencedConsensusTransaction {
            certificate_author_index: _,
            certificate_author,
            consensus_index,
            transaction,
        }) = transaction;
        let tracking_id = transaction.get_tracking_id();

        match &transaction {
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::CertifiedTransaction(certificate),
                ..
            }) => {
                if certificate.epoch() != self.epoch() {
                    // Epoch has changed after this certificate was sequenced, ignore it.
                    debug!(
                        "Certificate epoch ({:?}) doesn't match the current epoch ({:?})",
                        certificate.epoch(),
                        self.epoch()
                    );
                    return Ok(ConsensusCertificateResult::Ignored);
                }
                if self.has_sent_end_of_publish(certificate_author)?
                    && !previously_deferred_tx_digests.contains_key(certificate.digest())
                {
                    // This can not happen with valid authority
                    // With some edge cases consensus might sometimes resend previously seen
                    // certificate after EndOfPublish However this certificate
                    // will be filtered out before this line by `consensus_message_processed` call
                    // in `verify_consensus_transaction` If we see some new
                    // certificate here it means authority is byzantine and sent certificate after
                    // EndOfPublish (or we have some bug in ConsensusAdapter)
                    warn!(
                        "[Byzantine authority] Authority {:?} sent a new, previously unseen
                            certificate {:?} after it sent EndOfPublish message to consensus",
                        certificate_author.concise(),
                        certificate.digest()
                    );
                    return Ok(ConsensusCertificateResult::Ignored);
                }
                // Safe because signatures are verified when consensus called into
                // IotaTxValidator::validate_batch.
                let certificate = VerifiedCertificate::new_unchecked(*certificate.clone());
                let certificate = VerifiedExecutableTransaction::new_from_certificate(certificate);

                debug!(
                    ?tracking_id,
                    tx_digest = ?certificate.digest(),
                    "handle_consensus_transaction UserTransaction",
                );

                if !self
                    .get_reconfig_state_read_lock_guard()
                    .should_accept_consensus_certs()
                    && !previously_deferred_tx_digests.contains_key(certificate.digest())
                {
                    debug!(
                        "Ignoring consensus certificate for transaction {:?} because of end of epoch",
                        certificate.digest()
                    );
                    return Ok(ConsensusCertificateResult::Ignored);
                }

                let scheduling_result = self.try_schedule(
                    &certificate,
                    commit_round,
                    dkg_failed,
                    generating_randomness,
                    previously_deferred_tx_digests,
                    shared_object_congestion_tracker,
                );

                match scheduling_result {
                    SchedulingResult::Defer(deferral_key, deferral_reason) => {
                        debug!(
                            "Deferring consensus certificate for transaction {:?} until {:?}",
                            certificate.digest(),
                            deferral_key
                        );

                        let deferral_result = match deferral_reason {
                            DeferralReason::RandomnessNotReady => {
                                // Always defer transaction due to randomness not ready.
                                ConsensusCertificateResult::Deferred {
                                    deferral_key,
                                    suggested_gas_price: None,
                                }
                            }
                            DeferralReason::SharedObjectCongestion(congested_objects) => {
                                authority_metrics
                                    .consensus_handler_congested_transactions
                                    .inc();

                                let suggested_gas_price = if self
                                    .protocol_config
                                    .congestion_control_gas_price_feedback_mechanism()
                                {
                                    let current_commit_suggested_gas_price =
                                        suggested_gas_price_calculator
                                            .calculate_suggested_gas_price(&certificate);

                                    let suggested_gas_price = previously_deferred_tx_digests
                                        .get(certificate.digest())
                                        .map_or_else(
                                            || current_commit_suggested_gas_price,
                                            |deferral_key_suggested_gas_price_pair| {
                                                deferral_key_suggested_gas_price_pair
                                                    .1
                                                    // If None, this could mean the certificate was
                                                    // deferred due to randomness unavailable in
                                                    // the previous round, but in the current
                                                    // round, it gets deferred due to congestion.
                                                    // Since this is the first round the
                                                    // certificate is deferred due to congestion,
                                                    // we return the suggested gas price from the
                                                    // current round.
                                                    .unwrap_or(current_commit_suggested_gas_price)
                                                    .min(current_commit_suggested_gas_price)
                                            },
                                        );

                                    Some(suggested_gas_price)
                                } else {
                                    None
                                };

                                if transaction_deferral_within_limit(
                                    &deferral_key,
                                    self.protocol_config()
                                        .max_deferral_rounds_for_congestion_control(),
                                ) {
                                    ConsensusCertificateResult::Deferred {
                                        deferral_key,
                                        suggested_gas_price,
                                    }
                                } else {
                                    // Cancel the transaction that has been deferred for too long.

                                    debug!(
                                        "Cancelling consensus certificate for transaction {:?} \
                                            with deferral key {deferral_key:?} due to congestion \
                                            on objects {congested_objects:?}: actual gas price: \
                                            {}, suggested gas price: {suggested_gas_price:?}",
                                        certificate.digest(),
                                        certificate.transaction_data().gas_price(),
                                    );

                                    ConsensusCertificateResult::Cancelled((
                                        certificate,
                                        CancelConsensusCertificateReason::CongestionOnObjects {
                                            congested_objects,
                                            suggested_gas_price,
                                        },
                                    ))
                                }
                            }
                        };

                        Ok(deferral_result)
                    }
                    SchedulingResult::Schedule(start_time) => {
                        if dkg_failed && certificate.uses_randomness() {
                            debug!(
                                "Canceling randomness-using certificate for transaction {:?} because DKG failed",
                                certificate.digest(),
                            );

                            return Ok(ConsensusCertificateResult::Cancelled((
                                certificate,
                                CancelConsensusCertificateReason::DkgFailed,
                            )));
                        }

                        // This certificate will be scheduled. If it contains shared object(s),
                        // we have to update the following:
                        // - shared object execution slots (for congestion tracker);
                        // - shared object congestion info (for suggested gas price calculator).
                        if certificate.contains_shared_object()
                            && shared_object_congestion_tracker
                                .congestion_control_parameters()
                                .is_congestion_control_enabled()
                        {
                            // We only need to do this if shared-object congestion control is
                            // enabled, since otherwise this bumping will panic as object
                            // execution slots are only initialized if
                            // `max_execution_duration_per_commit` is not `None`.
                            let bump_result = shared_object_congestion_tracker
                                .bump_object_execution_slots(&certificate, start_time);

                            suggested_gas_price_calculator.update_congestion_info(bump_result);
                        }

                        Ok(ConsensusCertificateResult::Scheduled {
                            transaction: certificate,
                            start_time,
                        })
                    }
                }
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::CheckpointSignature(info),
                ..
            }) => {
                // We usually call notify_checkpoint_signature in IotaTxValidator, but that step
                // can be skipped when a batch is already part of a certificate,
                // so we must also notify here.
                checkpoint_service.notify_checkpoint_signature(self, info)?;
                Ok(ConsensusCertificateResult::ConsensusMessage)
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::EndOfPublish(_),
                ..
            }) => {
                // these are partitioned earlier
                panic!("process_consensus_transaction called with end-of-publish transaction");
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::MisbehaviorReport(authority, report, _),
                ..
            }) => {
                let authority_index = self
                    .committee
                    .authority_index(authority)
                    .expect("authority in committee");
                // Check validity of the report and update scores depending on the result. We
                // already have consensus on inclusion of this report in the DAG.
                if !report.verify(self.committee.num_members()) {
                    self.scorer.update_invalid_reports_count(authority_index);
                    warn!(
                        "Received invalid misbehavior report from {:?}",
                        authority.concise()
                    );
                } else {
                    // Here we update all counts related to the information in the reports.
                    self.scorer.update_received_reports(authority_index, report);
                }
                Ok(ConsensusCertificateResult::ConsensusMessage)
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::CapabilityNotificationV1(capabilities),
                ..
            }) => {
                // Records capabilities for the authority.
                let authority = capabilities.authority;
                if self
                    .get_reconfig_state_read_lock_guard()
                    .should_accept_consensus_certs()
                {
                    debug!(
                        "Received CapabilityNotificationV1 from {:?}",
                        authority.concise()
                    );
                    self.record_capabilities_v1(capabilities)?;
                } else {
                    debug!(
                        "Ignoring CapabilityNotificationV1 from {:?} because of end of epoch",
                        authority.concise()
                    );
                }
                Ok(ConsensusCertificateResult::ConsensusMessage)
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::SignedCapabilityNotificationV1(signed_capabilities),
                ..
            }) => {
                // Records capabilities for the authority.
                // The signature is checked in a previous step, so we can safely access data
                let capabilities = signed_capabilities.data();
                let authority = capabilities.authority;
                if self
                    .get_reconfig_state_read_lock_guard()
                    .should_accept_consensus_certs()
                {
                    debug!(
                        "Received SignedCapabilityNotificationV1 from {:?}",
                        authority.concise()
                    );
                    self.record_capabilities_v1(capabilities)?;
                } else {
                    debug!(
                        "Ignoring SignedCapabilityNotificationV1 from {:?} because of end of epoch",
                        authority.concise()
                    );
                }
                Ok(ConsensusCertificateResult::ConsensusMessage)
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::NewJWKFetched(authority, jwk_id, jwk),
                ..
            }) => {
                if self
                    .get_reconfig_state_read_lock_guard()
                    .should_accept_consensus_certs()
                {
                    self.record_jwk_vote(
                        output,
                        consensus_index.last_committed_round,
                        *authority,
                        jwk_id,
                        jwk,
                    )?;
                } else {
                    debug!(
                        "Ignoring NewJWKFetched from {:?} because of end of epoch",
                        authority.concise()
                    );
                }
                Ok(ConsensusCertificateResult::ConsensusMessage)
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::RandomnessDkgMessage(authority, bytes),
                ..
            }) => {
                if self.get_reconfig_state_read_lock_guard().should_accept_tx() {
                    if let Some(randomness_manager) = randomness_manager.as_mut() {
                        debug!(
                            "Received RandomnessDkgMessage from {:?}",
                            authority.concise()
                        );
                        match bcs::from_bytes(bytes) {
                            Ok(message) => randomness_manager.add_message(authority, message)?,
                            Err(e) => {
                                warn!(
                                    "Failed to deserialize RandomnessDkgMessage from {:?}: {e:?}",
                                    authority.concise()
                                );
                            }
                        }
                    } else {
                        debug!(
                            "Ignoring RandomnessDkgMessage from {:?} because randomness is not enabled",
                            authority.concise()
                        );
                    }
                } else {
                    debug!(
                        "Ignoring RandomnessDkgMessage from {:?} because of end of epoch",
                        authority.concise()
                    );
                }
                Ok(ConsensusCertificateResult::RandomnessConsensusMessage)
            }
            SequencedConsensusTransactionKind::External(ConsensusTransaction {
                kind: ConsensusTransactionKind::RandomnessDkgConfirmation(authority, bytes),
                ..
            }) => {
                if self.get_reconfig_state_read_lock_guard().should_accept_tx() {
                    if let Some(randomness_manager) = randomness_manager.as_mut() {
                        debug!(
                            "Received RandomnessDkgConfirmation from {:?}",
                            authority.concise()
                        );
                        match bcs::from_bytes(bytes) {
                            Ok(message) => {
                                randomness_manager.add_confirmation(output, authority, message)?
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to deserialize RandomnessDkgConfirmation from {:?}: {e:?}",
                                    authority.concise(),
                                );
                            }
                        }
                    } else {
                        debug!(
                            "Ignoring RandomnessDkgMessage from {:?} because randomness is not enabled",
                            authority.concise()
                        );
                    }
                } else {
                    debug!(
                        "Ignoring RandomnessDkgMessage from {:?} because of end of epoch",
                        authority.concise()
                    );
                }
                Ok(ConsensusCertificateResult::RandomnessConsensusMessage)
            }
            SequencedConsensusTransactionKind::System(system_transaction) => {
                Ok(self.process_consensus_system_transaction(system_transaction))
            }
        }
    }

    fn process_consensus_system_transaction(
        &self,
        system_transaction: &VerifiedExecutableTransaction,
    ) -> ConsensusCertificateResult {
        if !self.get_reconfig_state_read_lock_guard().should_accept_tx() {
            debug!(
                "Ignoring system transaction {:?} because of end of epoch",
                system_transaction.digest()
            );
            return ConsensusCertificateResult::IgnoredSystem;
        }

        // If needed we can support owned object system transactions as well...
        assert!(system_transaction.contains_shared_object());
        ConsensusCertificateResult::Scheduled {
            transaction: system_transaction.clone(),
            start_time: 0,
        }
    }

    pub(crate) fn write_pending_checkpoint(
        &self,
        output: &mut ConsensusCommitOutput,
        checkpoint: &PendingCheckpoint,
    ) -> IotaResult {
        assert!(
            !self.pending_checkpoint_exists(&checkpoint.height())?,
            "Duplicate pending checkpoint notification at height {:?}",
            checkpoint.height()
        );

        debug!(
            checkpoint_commit_height = checkpoint.height(),
            "Pending checkpoint has {} roots",
            checkpoint.roots().len(),
        );
        trace!(
            checkpoint_commit_height = checkpoint.height(),
            "Transaction roots for pending checkpoint: {:?}",
            checkpoint.roots()
        );

        output.insert_pending_checkpoint(checkpoint.clone());

        Ok(())
    }

    pub fn get_pending_checkpoints(
        &self,
        last: Option<CheckpointHeight>,
    ) -> IotaResult<Vec<(CheckpointHeight, PendingCheckpoint)>> {
        Ok(self
            .consensus_quarantine
            .read()
            .get_pending_checkpoints(last))
    }

    pub fn pending_checkpoint_exists(&self, index: &CheckpointHeight) -> IotaResult<bool> {
        Ok(self
            .consensus_quarantine
            .read()
            .pending_checkpoint_exists(index))
    }

    pub fn process_constructed_checkpoint(
        &self,
        commit_height: CheckpointHeight,
        content_info: NonEmpty<(CheckpointSummary, CheckpointContents)>,
    ) {
        let mut consensus_quarantine = self.consensus_quarantine.write();
        for (position_in_commit, (summary, transactions)) in content_info.into_iter().enumerate() {
            let sequence_number = summary.sequence_number;
            let summary = BuilderCheckpointSummary {
                summary,
                checkpoint_height: Some(commit_height),
                position_in_commit,
            };

            consensus_quarantine.insert_builder_summary(sequence_number, summary, transactions);
        }

        // Because builder can run behind state sync, the data may be immediately ready
        // to be committed.
        consensus_quarantine
            .commit(self)
            .expect("commit cannot fail");
    }

    /// Register genesis checkpoint in builder DB
    pub fn put_genesis_checkpoint_in_builder(
        &self,
        summary: &CheckpointSummary,
        contents: &CheckpointContents,
    ) -> IotaResult<()> {
        let sequence = summary.sequence_number;
        for transaction in contents.iter() {
            let digest = transaction.transaction;
            debug!(
                "Manually inserting genesis transaction in checkpoint DB: {:?}",
                digest
            );
            self.tables()?
                .builder_digest_to_checkpoint
                .insert(&digest, &sequence)?;
        }
        let builder_summary = BuilderCheckpointSummary {
            summary: summary.clone(),
            checkpoint_height: None,
            position_in_commit: 0,
        };
        self.tables()?
            .builder_checkpoint_summary
            .insert(summary.sequence_number(), &builder_summary)?;
        Ok(())
    }

    pub fn last_built_checkpoint_builder_summary(
        &self,
    ) -> IotaResult<Option<BuilderCheckpointSummary>> {
        if let Some(summary) = self.consensus_quarantine.read().last_built_summary() {
            return Ok(Some(summary.clone()));
        }

        Ok(self
            .tables()?
            .builder_checkpoint_summary
            .reversed_safe_iter_with_bounds(None, None)?
            .next()
            .transpose()?
            .map(|(_, s)| s))
    }

    pub fn last_built_checkpoint_summary(
        &self,
    ) -> IotaResult<Option<(CheckpointSequenceNumber, CheckpointSummary)>> {
        if let Some(BuilderCheckpointSummary { summary, .. }) =
            self.consensus_quarantine.read().last_built_summary()
        {
            let seq = *summary.sequence_number();
            debug!(
                "returning last_built_summary from consensus quarantine: {:?}",
                seq
            );
            Ok(Some((seq, summary.clone())))
        } else {
            let seq = self
                .tables()?
                .builder_checkpoint_summary
                .reversed_safe_iter_with_bounds(None, None)?
                .next()
                .transpose()?
                .map(|(seq, s)| (seq, s.summary));
            debug!(
                "returning last_built_summary from builder_checkpoint_summary: {:?}",
                seq
            );
            Ok(seq)
        }
    }

    pub fn get_built_checkpoint_summary(
        &self,
        sequence: CheckpointSequenceNumber,
    ) -> IotaResult<Option<CheckpointSummary>> {
        if let Some(BuilderCheckpointSummary { summary, .. }) =
            self.consensus_quarantine.read().get_built_summary(sequence)
        {
            return Ok(Some(summary.clone()));
        }

        Ok(self
            .tables()?
            .builder_checkpoint_summary
            .get(&sequence)?
            .map(|s| s.summary))
    }

    pub fn builder_included_transactions_in_checkpoint<'a>(
        &self,
        digests: impl Iterator<Item = &'a TransactionDigest>,
    ) -> IotaResult<Vec<bool>> {
        let digests: Vec<_> = digests.cloned().collect();
        let tables = self.tables()?;
        Ok(do_fallback_lookup(
            &digests,
            |digest| {
                let consensus_quarantine = self.consensus_quarantine.read();
                if consensus_quarantine.included_transaction_in_checkpoint(digest) {
                    CacheResult::Hit(true)
                } else {
                    CacheResult::Miss
                }
            },
            |remaining| {
                tables
                    .builder_digest_to_checkpoint
                    .multi_contains_keys(remaining)
                    .expect("db error")
            },
        ))
    }

    pub fn get_last_checkpoint_signature_index(&self) -> IotaResult<u64> {
        Ok(self
            .tables()?
            .pending_checkpoint_signatures
            .reversed_safe_iter_with_bounds(None, None)?
            .next()
            .transpose()?
            .map(|((_, index), _)| index)
            .unwrap_or_default())
    }

    pub fn insert_checkpoint_signature(
        &self,
        checkpoint_seq: CheckpointSequenceNumber,
        index: u64,
        info: &CheckpointSignatureMessage,
    ) -> IotaResult<()> {
        Ok(self
            .tables()?
            .pending_checkpoint_signatures
            .insert(&(checkpoint_seq, index), info)?)
    }

    pub(crate) fn record_epoch_pending_certs_process_time_metric(&self) {
        if let Some(epoch_close_time) = *self.epoch_close_time.read() {
            self.metrics
                .epoch_pending_certs_processed_time_since_epoch_close_ms
                .set(epoch_close_time.elapsed().as_millis() as i64);
        }
    }

    pub fn record_end_of_message_quorum_time_metric(&self) {
        if let Some(epoch_close_time) = *self.epoch_close_time.read() {
            self.metrics
                .epoch_end_of_publish_quorum_time_since_epoch_close_ms
                .set(epoch_close_time.elapsed().as_millis() as i64);
        }
    }

    pub(crate) fn report_epoch_metrics_at_last_checkpoint(&self, stats: EpochStats) {
        if let Some(epoch_close_time) = *self.epoch_close_time.read() {
            self.metrics
                .epoch_last_checkpoint_created_time_since_epoch_close_ms
                .set(epoch_close_time.elapsed().as_millis() as i64);
        }
        info!(epoch=?self.epoch(), "Epoch statistics: checkpoint_count={:?}, transaction_count={:?}, total_gas_reward={:?}", stats.checkpoint_count, stats.transaction_count, stats.total_gas_reward);
        self.metrics
            .epoch_checkpoint_count
            .set(stats.checkpoint_count as i64);
        self.metrics
            .epoch_transaction_count
            .set(stats.transaction_count as i64);
        self.metrics
            .epoch_total_gas_reward
            .set(stats.total_gas_reward as i64);
    }

    pub fn record_epoch_reconfig_start_time_metric(&self) {
        if let Some(epoch_close_time) = *self.epoch_close_time.read() {
            self.metrics
                .epoch_reconfig_start_time_since_epoch_close_ms
                .set(epoch_close_time.elapsed().as_millis() as i64);
        }
    }

    fn record_reconfig_halt_duration_metric(&self) {
        if let Some(epoch_close_time) = *self.epoch_close_time.read() {
            self.metrics
                .epoch_validator_halt_duration_ms
                .set(epoch_close_time.elapsed().as_millis() as i64);
        }
    }

    pub(crate) fn record_epoch_first_checkpoint_creation_time_metric(&self) {
        self.metrics
            .epoch_first_checkpoint_created_time_since_epoch_begin_ms
            .set(self.epoch_open_time.elapsed().as_millis() as i64);
    }

    pub fn record_is_safe_mode_metric(&self, safe_mode: bool) {
        self.metrics.is_safe_mode.set(safe_mode as i64);
    }

    pub fn record_checkpoint_builder_is_safe_mode_metric(&self, safe_mode: bool) {
        if safe_mode {
            // allow tests to inject a panic here.
            fail_point!("record_checkpoint_builder_is_safe_mode_metric");
        }
        self.metrics
            .checkpoint_builder_advance_epoch_is_safe_mode
            .set(safe_mode as i64)
    }

    fn record_epoch_total_duration_metric(&self) {
        self.metrics.current_epoch.set(self.epoch() as i64);
        self.metrics
            .epoch_total_duration
            .set(self.epoch_open_time.elapsed().as_millis() as i64);
    }

    pub(crate) fn update_authenticator_state(&self, update: &AuthenticatorStateUpdateV1) {
        info!("Updating authenticator state: {:?}", update);
        for active_jwk in &update.new_active_jwks {
            let ActiveJwk { jwk_id, jwk, .. } = active_jwk;
            self.signature_verifier.insert_jwk(jwk_id, jwk);
        }
    }

    pub fn clear_signature_cache(&self) {
        self.signature_verifier.clear_signature_cache();
    }

    pub(crate) fn check_all_executed_transactions_in_checkpoint(&self) {
        let uncheckpointed_transactions = self
            .consensus_output_cache
            .get_uncheckpointed_transactions();

        if uncheckpointed_transactions.is_empty() {
            info!("Verified that all executed transactions are in a checkpoint");
            return;
        }

        // TODO: should this be debug_fatal? Its potentially very serious in that it
        // could indicate that we broke the checkpoint inclusion guarantee, but
        // we won't be able to do anything about it if it happens.
        fatal!(
            "The following transactions were neither reverted nor checkpointed: {:?}",
            uncheckpointed_transactions
        );
    }

    // Only for testing purposes. Loads initial object debts from the consensus
    // quarantine.
    #[cfg(test)]
    pub(crate) fn load_stored_object_debts_for_testing(
        &self,
        for_randomness: bool,
        object_ids: &[ObjectID],
    ) -> IotaResult<Vec<Option<CongestionPerObjectDebt>>> {
        self.consensus_quarantine
            .read()
            .load_stored_object_debts_for_testing(for_randomness, object_ids)
    }
}

impl ExecutionComponents {
    fn new(
        protocol_config: &ProtocolConfig,
        store: Arc<dyn BackingPackageStore + Send + Sync>,
        metrics: Arc<ResolverMetrics>,
        // Keep this as a parameter for possible future use
        _expensive_safety_check_config: &ExpensiveSafetyCheckConfig,
    ) -> Self {
        let silent = true;
        let executor = iota_execution::executor(protocol_config, silent, None)
            .expect("Creating an executor should not fail here");

        let module_cache = Arc::new(SyncModuleCache::new(ResolverWrapper::new(
            store,
            metrics.clone(),
        )));
        Self {
            executor,
            module_cache,
            metrics,
        }
    }

    pub(crate) fn metrics(&self) -> Arc<ResolverMetrics> {
        self.metrics.clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LockDetailsWrapper {
    V1(TransactionDigest),
}

impl LockDetailsWrapper {
    pub fn migrate(self) -> Self {
        // TODO: when there are multiple versions, we must iteratively migrate from
        // version N to N+1 until we arrive at the latest version
        self
    }

    // Always returns the most recent version. Older versions are migrated to the
    // latest version at read time, so there is never a need to access older
    // versions.
    pub fn inner(&self) -> &LockDetails {
        match self {
            Self::V1(v1) => v1,

            // can remove #[expect] when there are multiple versions
            #[expect(unreachable_patterns)]
            _ => panic!("lock details should have been migrated to latest version at read time"),
        }
    }
    pub fn into_inner(self) -> LockDetails {
        match self {
            Self::V1(v1) => v1,

            // can remove #[expect] when there are multiple versions
            #[expect(unreachable_patterns)]
            _ => panic!("lock details should have been migrated to latest version at read time"),
        }
    }
}

pub type LockDetails = TransactionDigest;

impl From<LockDetails> for LockDetailsWrapper {
    fn from(details: LockDetails) -> Self {
        // always use latest version.
        LockDetailsWrapper::V1(details)
    }
}
