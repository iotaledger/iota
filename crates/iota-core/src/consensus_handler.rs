// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    num::NonZeroUsize,
    sync::Arc,
};

use arc_swap::ArcSwap;
use iota_common::{fatal, random_util::randomize_cache_capacity_in_tests};
use iota_macros::{fail_point, fail_point_if};
use iota_metrics::{
    monitored_mpsc, monitored_mpsc::UnboundedReceiver, monitored_scope, spawn_monitored_task,
};
use iota_sdk_types::{
    CanceledTransaction, ConsensusCommitDigest, SenderSignedTransaction, TransactionDigest,
};
use iota_types::{
    base_types::AuthorityName,
    error::{IotaError, IotaResult},
    executable_transaction::{TrustedExecutableTransaction, VerifiedExecutableTransaction},
    messages_consensus::{ConsensusTransaction, ConsensusTransactionKey, ConsensusTransactionKind},
    move_authenticator::MoveAuthenticatorExt,
    transaction::{
        SenderSignedTransactionAPI, TransactionAPI, TransactionKindExt, VerifiedTransaction,
    },
};
use lru::LruCache;
use serde::{Deserialize, Serialize};
use starfish_config::Committee as ConsensusCommittee;
use tokio::task::{JoinHandle, JoinSet};
use tracing::{debug, info, instrument, trace_span, warn};

use crate::{
    authority::{
        AuthorityMetrics, AuthorityState, ExecutionEnv,
        authority_per_epoch_store::{
            AuthorityPerEpochStore, ConsensusStats, ConsensusStatsAPI, ExecutionIndices,
            ExecutionIndicesWithStats,
            handler_object_state::{AssignedCommit, CommitIndex, handler_rows_for_commit},
        },
        backpressure::{BackpressureManager, BackpressureSubscriber},
        shared_object_version_manager::{AssignedTxAndVersions, Schedulable},
    },
    checkpoints::{CheckpointService, CheckpointServiceNotify},
    consensus_types::{
        AuthorityIndex,
        consensus_output_api::{ConsensusOutputAPI, ConsensusOutputTransactions},
    },
    epoch::randomness::AdvanceDkgError,
    epoch_start_consensus_committee::get_consensus_committee,
    execution_cache::ObjectCacheRead,
    execution_scheduler::{ExecutionSchedulerAPI, ExecutionSchedulerWrapper},
    scoring_decision::update_low_scoring_authorities,
};

pub struct ConsensusHandlerInitializer {
    state: Arc<AuthorityState>,
    checkpoint_service: Arc<CheckpointService>,
    epoch_store: Arc<AuthorityPerEpochStore>,
    low_scoring_authorities: Arc<ArcSwap<HashMap<AuthorityName, u64>>>,
    backpressure_manager: Arc<BackpressureManager>,
}

impl ConsensusHandlerInitializer {
    pub fn new(
        state: Arc<AuthorityState>,
        checkpoint_service: Arc<CheckpointService>,
        epoch_store: Arc<AuthorityPerEpochStore>,
        low_scoring_authorities: Arc<ArcSwap<HashMap<AuthorityName, u64>>>,
        backpressure_manager: Arc<BackpressureManager>,
    ) -> Self {
        Self {
            state,
            checkpoint_service,
            epoch_store,
            low_scoring_authorities,
            backpressure_manager,
        }
    }

    pub fn new_for_testing(
        state: Arc<AuthorityState>,
        checkpoint_service: Arc<CheckpointService>,
    ) -> Self {
        let backpressure_manager = BackpressureManager::new_for_tests();
        Self {
            state: state.clone(),
            checkpoint_service,
            epoch_store: state.epoch_store_for_testing().clone(),
            low_scoring_authorities: Arc::new(Default::default()),
            backpressure_manager,
        }
    }

    pub fn new_consensus_handler(&self) -> ConsensusHandler<CheckpointService> {
        let new_epoch_start_state = self.epoch_store.epoch_start_state();
        let consensus_committee = get_consensus_committee(new_epoch_start_state);

        ConsensusHandler::new(
            self.epoch_store.clone(),
            self.state.clone(),
            self.checkpoint_service.clone(),
            self.state.execution_scheduler().clone(),
            self.state.get_object_cache_reader().clone(),
            self.low_scoring_authorities.clone(),
            consensus_committee,
            self.state.metrics.clone(),
            self.backpressure_manager.subscribe(),
        )
    }
}

/// Runtime shutdown cancelled blocking work while a consensus commit was being
/// processed.
#[derive(Debug)]
pub(crate) struct RuntimeShutdown;

pub struct ConsensusHandler<C> {
    /// A store created for each epoch. ConsensusHandler is recreated each
    /// epoch, with the corresponding store. This store is also used to get
    /// the current epoch ID.
    epoch_store: Arc<AuthorityPerEpochStore>,
    /// The authority state, used for post-consensus transaction validation.
    state: Arc<AuthorityState>,
    /// Holds the indices, hash and stats after the last consensus commit
    /// It is used for avoiding replaying already processed transactions,
    /// checking chain consistency, and accumulating per-epoch consensus output
    /// stats.
    last_consensus_stats: ExecutionIndicesWithStats,
    checkpoint_service: Arc<C>,
    /// cache reader is needed when determining the next version to assign for
    /// shared objects.
    cache_reader: Arc<dyn ObjectCacheRead>,
    /// Reputation scores used by consensus adapter that we update, forwarded
    /// from consensus
    low_scoring_authorities: Arc<ArcSwap<HashMap<AuthorityName, u64>>>,
    /// The consensus committee used to do stake computations for deciding set
    /// of low scoring authorities
    committee: ConsensusCommittee,
    // TODO: ConsensusHandler doesn't really share metrics with AuthorityState. We could define
    // a new metrics type here if we want to.
    metrics: Arc<AuthorityMetrics>,
    /// Lru cache to quickly discard transactions processed by consensus
    processed_cache: LruCache<SequencedConsensusTransactionKey, ()>,
    /// The `pcool_deterministic_validation` flag, fixed for the epoch. With it
    /// on, the per-commit duplicate check keeps one copy of a user
    /// transaction per author.
    deterministic_validation: bool,
    transaction_scheduler: AsyncTransactionScheduler,
    /// Marks commits fully executed for the P-COOL deterministic-validation
    /// bookkeeping; `None` unless this node keeps the bookkeeping. Held only
    /// so the task is aborted with the handler.
    _execution_watcher: Option<ExecutionWatcher>,

    backpressure_subscriber: BackpressureSubscriber,
}

const PROCESSED_CACHE_CAP: usize = 1024 * 1024;

impl<C> ConsensusHandler<C> {
    pub(crate) fn new(
        epoch_store: Arc<AuthorityPerEpochStore>,
        state: Arc<AuthorityState>,
        checkpoint_service: Arc<C>,
        execution_scheduler: Arc<ExecutionSchedulerWrapper>,
        cache_reader: Arc<dyn ObjectCacheRead>,
        low_scoring_authorities: Arc<ArcSwap<HashMap<AuthorityName, u64>>>,
        committee: ConsensusCommittee,
        metrics: Arc<AuthorityMetrics>,
        backpressure_subscriber: BackpressureSubscriber,
    ) -> Self {
        // Recover last_consensus_stats so it is consistent across validators.
        let mut last_consensus_stats = epoch_store
            .get_last_consensus_stats()
            .expect("Should be able to read last consensus index");
        // stats is empty at the beginning of epoch.
        if !last_consensus_stats.stats.is_initialized() {
            last_consensus_stats.stats = ConsensusStats::new(committee.size());
        }
        let transaction_scheduler =
            AsyncTransactionScheduler::start(execution_scheduler, epoch_store.clone());
        let execution_watcher = epoch_store
            .pcool_bookkeeping_enabled()
            .then(|| ExecutionWatcher::start(epoch_store.clone()));

        let deterministic_validation = epoch_store
            .protocol_config()
            .pcool_deterministic_validation();

        // Seed the gauges so series exist from epoch start, not only after the
        // first commit.
        publish_scoring_gauges(&epoch_store, &committee, &metrics);

        Self {
            epoch_store,
            state,
            last_consensus_stats,
            checkpoint_service,
            cache_reader,
            low_scoring_authorities,
            committee,
            metrics,
            processed_cache: LruCache::new(
                NonZeroUsize::new(randomize_cache_capacity_in_tests(PROCESSED_CACHE_CAP)).unwrap(),
            ),
            deterministic_validation,
            transaction_scheduler,
            _execution_watcher: execution_watcher,
            backpressure_subscriber,
        }
    }

    /// Returns the last subdag index processed by the handler.
    pub fn last_processed_subdag_index(&self) -> u64 {
        self.last_consensus_stats.index.sub_dag_index
    }
}

impl<C: CheckpointServiceNotify + Send + Sync> ConsensusHandler<C> {
    /// Called during startup to allow us to observe commits we previously
    /// processed, for crash recovery. Any state computed here must be a
    /// pure function of the commits observed, it cannot depend on any state
    /// recorded in the epoch db.
    fn handle_prior_consensus_output(&mut self, consensus_commit: impl ConsensusOutputAPI) {
        // TODO: this will be used to recover state computed from previous commits at
        // startup.
        let round = consensus_commit.leader_round();
        info!("Ignoring prior consensus commit for round {:?}", round);
    }

    /// Test-only wrapper that performs BCS deserialization inline before
    /// delegating to [`Self::handle_consensus_output`]. In production the
    /// deserialization is performed on the [`StarfishConsensusHandler`]
    /// pipeline stage that feeds the handler.
    #[cfg(test)]
    async fn handle_consensus_output_for_test(
        &mut self,
        consensus_output: impl ConsensusOutputAPI,
    ) {
        let block_transactions = consensus_output.transactions();
        self.handle_consensus_output(consensus_output, block_transactions)
            .await
            .expect("runtime should not shut down during a test commit");
    }

    /// Processes one commit, unless this node executes downloaded checkpoints
    /// for the rest of the epoch. A commit still in progress when that starts
    /// is abandoned; its transactions are then executed from those
    /// checkpoints. An abandoned commit can leave behind, for the rest of the
    /// epoch only: dropped-transaction statuses already reported to
    /// submitters, P-COOL soft locks that are released only by their timeout,
    /// and in-memory deferred-transaction, randomness and handler state.
    /// Nothing reads them again in this epoch, and the next epoch starts from
    /// a new epoch store.
    ///
    /// Returns `Err(RuntimeShutdown)` if runtime shutdown cancelled work that
    /// the commit depends on. The commit's output is then not persisted, and
    /// it is replayed after restart.
    #[instrument("handle_consensus_output", level = "trace", skip_all)]
    async fn handle_consensus_output(
        &mut self,
        consensus_output: impl ConsensusOutputAPI,
        block_transactions: ConsensusOutputTransactions,
    ) -> Result<(), RuntimeShutdown> {
        let epoch_store = self.epoch_store.clone();
        // Only deterministic validation switches a validator to executing
        // synced checkpoints within an epoch.
        if !epoch_store
            .protocol_config()
            .pcool_deterministic_validation()
        {
            return self
                .process_consensus_output(consensus_output, block_transactions)
                .await;
        }
        if epoch_store.is_executing_synced_checkpoints() {
            debug!("not processing consensus commit: executing synced checkpoints");
            return Ok(());
        }
        let _commit_guard = epoch_store.lock_consensus_commit().await;
        // The switch may have happened while waiting for the guard.
        if epoch_store.is_executing_synced_checkpoints() {
            debug!("not processing consensus commit: executing synced checkpoints");
            return Ok(());
        }
        // A node that switched to executing synced checkpoints and then
        // restarted stops after the same commit as before the restart: the
        // synced checkpoints it has executed since the switch wrote outputs
        // that validating a later commit would read.
        let commit_index = consensus_output.commit_sub_dag_index();
        let last_commit_before_switch = epoch_store
            .last_commit_before_synced_checkpoint_execution()
            .expect("the epoch store of the commit being processed must be readable");
        if last_commit_before_switch.is_some_and(|last| commit_index > last) {
            info!(
                commit_index,
                ?last_commit_before_switch,
                "not processing consensus commit: switched to executing synced checkpoints before a restart"
            );
            epoch_store.resume_executing_synced_checkpoints();
            return Ok(());
        }
        // Every wait while processing the commit races against the switch, so
        // the commit cannot run against state that executing downloaded
        // checkpoints writes, and the switch is not held up waiting for it.
        tokio::select! {
            biased;
            _ = epoch_store.wait_for_synced_checkpoint_execution() => {
                info!("abandoned consensus commit: executing synced checkpoints for the rest of the epoch");
                Ok(())
            }
            result = self.process_consensus_output(consensus_output, block_transactions) => {
                // A commit stopped by runtime shutdown is replayed after restart,
                // so it is not handled yet.
                if result.is_ok() {
                    epoch_store.record_handled_commit(commit_index);
                }
                result
            }
        }
    }

    async fn process_consensus_output(
        &mut self,
        consensus_output: impl ConsensusOutputAPI,
        block_transactions: ConsensusOutputTransactions,
    ) -> Result<(), RuntimeShutdown> {
        // This may block until one of two conditions happens:
        // - Number of uncommitted transactions in the writeback cache goes below the backpressure
        //   threshold.
        // - The highest executed checkpoint catches up to the highest certified checkpoint.
        self.backpressure_subscriber.await_no_backpressure().await;

        let _scope = monitored_scope("HandleConsensusOutput");

        let last_committed_round = self.last_consensus_stats.index.last_committed_round;

        let round = consensus_output.leader_round();

        // TODO: Is this check necessary? For now consensus will not
        // return more than one leader per round so we are not in danger of
        // ignoring any commits.
        assert!(
            round >= last_committed_round,
            "Consensus output round {round} is less than last committed round {last_committed_round}"
        );
        if last_committed_round == round {
            // we can receive the same commit twice after restart
            // It is critical that the writes done by this function are atomic - otherwise
            // we can lose the later parts of a commit if we restart midway
            // through processing it.
            warn!(
                "Ignoring consensus output for round {} as it is already committed. NOTE: This is only expected if consensus is running.",
                round
            );
            return Ok(());
        }

        // (serialized, transaction, output_cert)
        let mut transactions = vec![];
        let leader_author = consensus_output.leader_author_index();
        let commit_sub_dag_index = consensus_output.commit_sub_dag_index();

        debug!(
            %consensus_output,
            epoch = ?self.epoch_store.epoch(),
            "Received consensus output"
        );

        let execution_index = ExecutionIndices {
            last_committed_round: round,
            sub_dag_index: commit_sub_dag_index,
            transaction_index: 0_u64,
        };
        // This function has filtered out any already processed consensus output.
        // So we can safely assume that the index is always increasing.
        assert!(self.last_consensus_stats.index < execution_index);

        // TODO: test empty commit explicitly.
        // Note that consensus commit batch may contain no transactions, but we still
        // need to record the current round and subdag index in the
        // last_consensus_stats, so that it won't be re-executed in the future.
        self.last_consensus_stats.index = execution_index;

        if self
            .epoch_store
            .protocol_config()
            .calculate_validator_scores()
        {
            self.epoch_store
                .misbehavior_monitor
                .update_from_consensus_output(consensus_output.misbehavior_counts());
        }

        update_low_scoring_authorities(
            self.low_scoring_authorities.clone(),
            self.epoch_store.committee(),
            &self.committee,
            consensus_output.reputation_score_sorted_desc(),
            &self.metrics,
            self.epoch_store
                .protocol_config()
                .consensus_bad_nodes_stake_threshold(),
        );

        self.metrics
            .consensus_committed_subdags
            .with_label_values(&[&leader_author.to_string()])
            .inc();

        self.metrics
            .consensus_handler_leader_round
            .set(round as i64);

        for (authority_index, number_of_committed_headers) in
            consensus_output.number_of_headers_in_commit_by_authority()
        {
            self.last_consensus_stats
                .stats
                .inc_num_messages(authority_index as usize, number_of_committed_headers);
        }

        {
            let span = trace_span!("process_consensus_certs");
            let _guard = span.enter();
            for (authority_index, authority_transactions) in block_transactions {
                // TODO: consider only messages within 1~3 rounds of the leader?
                for (transaction, serialized_len) in authority_transactions {
                    let kind = classify(&transaction);
                    self.metrics
                        .consensus_handler_processed
                        .with_label_values(&[kind])
                        .inc();
                    self.metrics
                        .consensus_handler_transaction_sizes
                        .with_label_values(&[kind])
                        .observe(serialized_len as f64);
                    if transaction.is_user_certificate() || transaction.kind.is_user_transaction() {
                        self.last_consensus_stats
                            .stats
                            .inc_num_user_transactions(authority_index as usize);
                    }
                    let transaction = SequencedConsensusTransactionKind::External(transaction);
                    transactions.push((transaction, authority_index));
                }
            }
        }

        for (i, authority) in self.committee.authorities() {
            let hostname = &authority.hostname;
            self.metrics
                .consensus_committed_messages
                .with_label_values(&[hostname])
                .set(self.last_consensus_stats.stats.get_num_messages(i.value()) as i64);
            self.metrics
                .consensus_committed_user_transactions
                .with_label_values(&[hostname])
                .set(
                    self.last_consensus_stats
                        .stats
                        .get_num_user_transactions(i.value()) as i64,
                );
        }

        // TODO(#13170): once `consensus_dedup_only_recorded_messages` is enabled on
        // all networks, remove the flag and the old caching it replaces below.
        let dedup_only_recorded_messages = self
            .epoch_store
            .protocol_config()
            .consensus_dedup_only_recorded_messages();
        let mut all_transactions = Vec::new();
        {
            // We need a set here as well, since the processed_cache is a LRU cache and can
            // drop entries while we're iterating over the sequenced
            // transactions.
            let mut processed_set = HashSet::new();
            let skipped_consensus_txns = &self.metrics.skipped_consensus_txns;

            for (seq, (transaction, cert_origin)) in transactions.into_iter().enumerate() {
                // In process_consensus_transactions_and_commit_boundary(), we will add a system
                // consensus commit prologue transaction, which will be the
                // first transaction in this consensus commit batch. Therefore,
                // the transaction sequence number starts from 1 here.
                let current_tx_index = ExecutionIndices {
                    last_committed_round: round,
                    sub_dag_index: commit_sub_dag_index,
                    transaction_index: (seq + 1) as u64,
                };

                self.last_consensus_stats.index = current_tx_index;

                let certificate_author = *self
                    .epoch_store
                    .committee()
                    .authority_by_index(cert_origin)
                    .unwrap();

                let sequenced_transaction = SequencedConsensusTransaction {
                    certificate_author_index: cert_origin,
                    certificate_author,
                    consensus_index: current_tx_index,
                    transaction,
                };

                // The digest does not cover the signatures, so with deterministic
                // validation one author's copy of a user transaction must not hide
                // another author's copy: validation tries a later copy when an
                // earlier one fails on its own signatures. One copy per author
                // bounds that work.
                let copy_author = (self.deterministic_validation
                    && sequenced_transaction.is_user_transaction())
                .then_some(cert_origin);
                // With the flag on, the set holds only keys of messages that passed verification
                // earlier in this commit and the cache only keys recorded as processed. A hit in
                // either means this message would be skipped after verification too, whatever its
                // payload.
                if dedup_only_recorded_messages {
                    let claimed_key = sequenced_transaction.transaction.key();
                    if processed_set.contains(&(claimed_key.clone(), copy_author))
                        || self.processed_cache.contains(&claimed_key)
                    {
                        self.metrics.skipped_consensus_txns_cache_hit.inc();
                        continue;
                    }
                }

                // Verify before adding the key to the set or the cache. The key is derived
                // from identities the payload claims, so a rejected transaction must not be
                // able to shadow a later valid one carrying the same key.
                let Some(verified_transaction) = self
                    .epoch_store
                    .verify_consensus_transaction(sequenced_transaction, skipped_consensus_txns)
                else {
                    continue;
                };

                let key = verified_transaction.0.key();
                if dedup_only_recorded_messages {
                    processed_set.insert((key, copy_author));
                } else {
                    // TODO(#13170): remove with the flag. Caches a key before the epoch store
                    // decides whether to record it.
                    let in_set = !processed_set.insert((key.clone(), copy_author));
                    let in_cache = self.processed_cache.put(key, ()).is_some();
                    if in_set || in_cache {
                        self.metrics.skipped_consensus_txns_cache_hit.inc();
                        continue;
                    }
                }

                all_transactions.push(verified_transaction);
            }
        }

        let (transactions_to_schedule, assigned_versions, processed_keys) = match self
            .epoch_store
            .process_consensus_transactions_and_commit_boundary(
                all_transactions,
                &self.last_consensus_stats,
                &self.checkpoint_service,
                self.cache_reader.as_ref(),
                &ConsensusCommitInfo::new(&consensus_output),
                &self.metrics,
                &self.state,
            )
            .await
        {
            Ok(result) => result,
            Err(AdvanceDkgError::RuntimeShutdown) => return Err(RuntimeShutdown),
            // The wait for the execution frontier was cut short by the end of
            // the epoch. Nothing in this commit is scheduled: the epoch is
            // over and reconfiguration drops this handler.
            Err(AdvanceDkgError::Iota(IotaError::EpochEnded(epoch)))
                if self.epoch_store.pcool_bookkeeping_enabled() =>
            {
                info!(
                    epoch,
                    "consensus commit abandoned: the epoch ended during validation"
                );
                return Ok(());
            }
            Err(AdvanceDkgError::Iota(e)) => {
                panic!("Unrecoverable error in consensus handler: {e:?}")
            }
        };
        for key in processed_keys {
            self.processed_cache.put(key, ());
        }

        publish_scoring_gauges(&self.epoch_store, &self.committee, &self.metrics);

        fail_point_if!("correlated-crash-after-consensus-commit-boundary", || {
            let key = [commit_sub_dag_index, self.epoch_store.epoch()];
            if iota_simulator::random::deterministic_probability_once(key, 0.01) {
                iota_simulator::task::kill_current_node(None);
            }
        });

        fail_point!("crash"); // for tests that produce random crashes

        self.transaction_scheduler
            .schedule(transactions_to_schedule, assigned_versions)
            .await;
        Ok(())
    }
}

/// Publishes the per-authority validator-scoring gauges sourced from
/// `Scoreboard` and `ReportAggregator`. No-op when validator scores are
/// disabled by protocol config.
fn publish_scoring_gauges(
    epoch_store: &AuthorityPerEpochStore,
    committee: &ConsensusCommittee,
    metrics: &AuthorityMetrics,
) {
    if !epoch_store.protocol_config().calculate_validator_scores() {
        return;
    }
    let scores = epoch_store.scoreboard.current_scores();
    let invalid_reports = epoch_store.report_aggregator.invalid_reports_counts();
    for (i, authority) in committee.authorities() {
        let labels = &[authority.hostname.as_str()];
        metrics
            .validator_scoreboard_scores
            .with_label_values(labels)
            .set(scores[i.value()] as i64);
        metrics
            .invalid_misbehavior_reports_by_authority
            .with_label_values(labels)
            .set(invalid_reports[i.value()] as i64);
    }
}

struct AsyncTransactionScheduler {
    sender: tokio::sync::mpsc::Sender<(Vec<Schedulable>, AssignedTxAndVersions)>,
}

impl AsyncTransactionScheduler {
    pub fn start(
        execution_scheduler: Arc<ExecutionSchedulerWrapper>,
        epoch_store: Arc<AuthorityPerEpochStore>,
    ) -> Self {
        let (sender, recv) = tokio::sync::mpsc::channel(16);
        spawn_monitored_task!(Self::run(recv, execution_scheduler, epoch_store));
        Self { sender }
    }

    pub async fn schedule(
        &self,
        transactions: Vec<Schedulable>,
        assigned_versions: AssignedTxAndVersions,
    ) {
        tracing::trace_span!("transaction_scheduler_enqueue");
        self.sender
            .send((transactions, assigned_versions))
            .await
            .ok();
    }

    pub async fn run(
        mut recv: tokio::sync::mpsc::Receiver<(Vec<Schedulable>, AssignedTxAndVersions)>,
        execution_scheduler: Arc<ExecutionSchedulerWrapper>,
        epoch_store: Arc<AuthorityPerEpochStore>,
    ) {
        while let Some((transactions, assigned_versions)) = recv.recv().await {
            let _guard = monitored_scope("ConsensusHandler::enqueue");
            let assigned_versions = assigned_versions.into_map();
            let txns = transactions
                .into_iter()
                .map(|txn| {
                    let key = txn.key();
                    (
                        txn,
                        ExecutionEnv::new().with_assigned_versions(
                            assigned_versions.get(&key).cloned().unwrap_or_default(),
                        ),
                    )
                })
                .collect();
            execution_scheduler.enqueue(txns, &epoch_store);
        }
    }
}

/// Name under which the watcher's effects waits appear in the notify-read
/// long-wait log.
const EXECUTION_WATCHER_NOTIFY_READ_TASK_NAME: &str =
    "ExecutionWatcher::notify_read_executed_effects";

/// Marks each consensus commit fully executed for the P-COOL
/// deterministic-validation bookkeeping once all of its roots have effects.
///
/// Receives commits from the epoch store in processing order and completes
/// them in that order, so a completed commit implies every earlier commit is
/// complete. Lives for one epoch: the wait ends when the epoch is terminated,
/// and the task is aborted when the handler drops. An error stops the node:
/// without the watcher, a commit waited on would complete only once its
/// checkpoint is certified.
pub(crate) struct ExecutionWatcher {
    handle: JoinHandle<()>,
}

impl ExecutionWatcher {
    /// Starts the watcher for `epoch_store`'s epoch.
    ///
    /// # Panics
    ///
    /// If a watcher already took the epoch store's assigned-commit receiver.
    /// Without a watcher the highest fully executed commit never advances and
    /// post-consensus validation waits on it forever, so a second start is a
    /// bug that must not run on.
    pub(crate) fn start(epoch_store: Arc<AuthorityPerEpochStore>) -> Self {
        let receiver = epoch_store.take_assigned_commits_receiver().expect(
            "the epoch store's assigned-commit receiver is taken once, by its epoch's \
             consensus handler",
        );
        let handle = spawn_monitored_task!(async move {
            match epoch_store
                .within_alive_epoch(Self::run(receiver, &epoch_store))
                .await
            {
                Ok(Ok(())) => debug!("ExecutionWatcher finished: assigned-commit channel closed"),
                Ok(Err(e)) => {
                    epoch_store.record_execution_watcher_error(&e);
                    fatal!("ExecutionWatcher stopping on error: {e}");
                }
                Err(_) => debug!(
                    epoch = epoch_store.epoch(),
                    "ExecutionWatcher stopping: epoch ended"
                ),
            }
        });
        Self { handle }
    }

    async fn run(
        mut receiver: UnboundedReceiver<AssignedCommit>,
        epoch_store: &AuthorityPerEpochStore,
    ) -> IotaResult {
        while let Some(AssignedCommit { index, roots }) = receiver.recv().await {
            let digests = epoch_store.notify_read_tx_key_to_digest(&roots).await?;
            let effects = epoch_store
                .effects_store()
                .try_notify_read_executed_effects(EXECUTION_WATCHER_NOTIFY_READ_TASK_NAME, &digests)
                .await?;
            let upserts = handler_rows_for_commit(&effects, index);
            #[cfg(test)]
            epoch_store.injected_watcher_completion_error()?;
            // A no-op once the commit's output has flushed: the flush derives
            // the durable rows from the same effects and completes the commit
            // itself.
            epoch_store.record_commit_fully_executed(index, &upserts)?;
        }
        Ok(())
    }
}

impl Drop for ExecutionWatcher {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Capacity of the channel from the deserialize worker to the commit handler.
/// Small and bounded: the worker prepares ~1 commit ahead (pipelining) while
/// applying backpressure when the handler is the bottleneck, rather than
/// buffering parsed commits unboundedly.
const CONSENSUS_HANDLER_DESERIALIZE_CHANNEL_CAPACITY: usize = 2;

/// Consensus handler used by Starfish.
/// During initialization, the sender is passed into Starfish which can send
/// consensus output to the channel.
///
/// The handler is split into two stages connected by a small bounded channel:
///   - **Deserialize worker** — BCS-parses each commit's transactions off the handler's critical
///     path so parsing can overlap with the handler processing the previous commit.
///   - **Commit handler** — consumes pre-parsed commits in arrival order.
///
/// Both stages run as a single worker each, with a FIFO channel between them,
/// so the original commit ordering is preserved by construction.
pub struct StarfishConsensusHandler {
    tasks: JoinSet<()>,
}

impl StarfishConsensusHandler {
    pub fn new(
        last_processed_commit_at_startup: starfish_core::CommitIndex,
        mut consensus_handler: ConsensusHandler<CheckpointService>,
        mut receiver: monitored_mpsc::UnboundedReceiver<starfish_core::CommittedSubDag>,
        commit_consumer_monitor: Arc<starfish_core::CommitConsumerMonitor>,
    ) -> Self {
        let mut tasks = JoinSet::new();

        // Stage 1 — deserialize worker: BCS-parses each commit's transactions
        // off the handler's critical path, so parsing overlaps the handler
        // processing the previous commit. The channel is bounded (small) so
        // the worker prepares ~1 commit ahead but applies backpressure
        // (rather than buffering parsed commits unboundedly) when the handler
        // is the bottleneck. Single-threaded; ordering is preserved (one
        // worker, FIFO channel, one handler).
        let (parsed_sender, mut parsed_receiver) = monitored_mpsc::channel(
            "consensus_deserialized_commits",
            CONSENSUS_HANDLER_DESERIALIZE_CHANNEL_CAPACITY,
        );
        tasks.spawn(async move {
            while let Some(consensus_output) = receiver.recv().await {
                let block_transactions: ConsensusOutputTransactions = {
                    let _scope = monitored_scope("StarfishConsensusHandler::deserialize_worker");
                    consensus_output.transactions()
                };
                // The send is intentionally outside the scope above: on the
                // bounded channel it blocks when the handler is the
                // bottleneck, and that idle-wait would otherwise inflate the
                // deserialize_worker scope (making it read as CPU work, not
                // waiting).
                if parsed_sender
                    .send((consensus_output, block_transactions))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        });

        // Stage 2 — commit handler: processes pre-parsed commits in order.
        tasks.spawn(async move {
            // TODO: pause when execution is overloaded, so consensus can detect the
            // backpressure.
            while let Some((consensus_output, block_transactions)) = parsed_receiver.recv().await {
                let commit_index = consensus_output.commit_ref.index;
                if commit_index <= last_processed_commit_at_startup {
                    consensus_handler.handle_prior_consensus_output(consensus_output);
                } else if let Err(e) = consensus_handler
                    .handle_consensus_output(consensus_output, block_transactions)
                    .await
                {
                    info!(
                        "Consensus commit {commit_index} was not handled ({e:?}); it will be replayed after restart"
                    );
                    break;
                }
                commit_consumer_monitor.set_highest_handled_commit(commit_index);
            }
        });
        Self { tasks }
    }

    pub async fn abort(&mut self) {
        self.tasks.abort_all();
        while self.tasks.join_next().await.is_some() {}
    }
}

impl Drop for StarfishConsensusHandler {
    fn drop(&mut self) {
        self.tasks.abort_all();
    }
}

/// Whether the transaction or one of its Move authenticators names a shared
/// object, the same answer as `contains_shared_object`. It does not merge the
/// declarations, since `classify` runs on committed transactions before
/// `validity_check` has rejected those whose declarations disagree.
fn names_shared_object(transaction: &SenderSignedTransaction) -> bool {
    transaction.transaction().kind().contains_shared_object()
        || transaction
            .move_authenticators()
            .iter()
            .any(|authenticator| !authenticator.shared_objects().is_empty())
}

pub(crate) fn classify(transaction: &ConsensusTransaction) -> &'static str {
    match &transaction.kind {
        ConsensusTransactionKind::CertifiedTransaction(certificate) => {
            if certificate.contains_shared_object() {
                "shared_certificate"
            } else {
                "owned_certificate"
            }
        }
        ConsensusTransactionKind::UserTransactionV1(transaction) => {
            if names_shared_object(transaction.data()) {
                "shared_user_transaction"
            } else {
                "owned_user_transaction"
            }
        }
        ConsensusTransactionKind::CheckpointSignature(_) => "checkpoint_signature",
        ConsensusTransactionKind::EndOfPublish(_) => "end_of_publish",
        ConsensusTransactionKind::CapabilityNotificationV1(_) => "capability_notification_v1",
        ConsensusTransactionKind::MisbehaviorReport(_) => "misbehavior_report",
        ConsensusTransactionKind::SignedCapabilityNotificationV1(_) => {
            "signed_capability_notification_v1"
        }
        #[allow(deprecated)]
        ConsensusTransactionKind::NewJWKFetchedDeprecated => "new_jwk_fetched_deprecated",
        ConsensusTransactionKind::RandomnessDkgMessage(_, _) => "randomness_dkg_message",
        ConsensusTransactionKind::RandomnessDkgConfirmation(_, _) => "randomness_dkg_confirmation",
        ConsensusTransactionKind::OverloadNotificationV1(_, _, _) => "overload_notification_v1",
        ConsensusTransactionKind::TransactionDenyRuleProposal(_) => {
            "transaction_deny_rule_proposal"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencedConsensusTransaction {
    pub certificate_author_index: AuthorityIndex,
    pub certificate_author: AuthorityName,
    pub consensus_index: ExecutionIndices,
    pub transaction: SequencedConsensusTransactionKind,
}

#[derive(Debug, Clone)]
pub enum SequencedConsensusTransactionKind {
    External(ConsensusTransaction),
    System(VerifiedExecutableTransaction),
}

impl Serialize for SequencedConsensusTransactionKind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let serializable = SerializableSequencedConsensusTransactionKind::from(self);
        serializable.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SequencedConsensusTransactionKind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let serializable =
            SerializableSequencedConsensusTransactionKind::deserialize(deserializer)?;
        Ok(serializable.into())
    }
}

// We can't serialize SequencedConsensusTransactionKind directly because it
// contains a VerifiedExecutableTransaction, which is not serializable (by
// design). This wrapper allows us to convert to a serializable format easily.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum SerializableSequencedConsensusTransactionKind {
    External(Box<ConsensusTransaction>),
    System(Box<TrustedExecutableTransaction>),
}

impl From<&SequencedConsensusTransactionKind> for SerializableSequencedConsensusTransactionKind {
    fn from(kind: &SequencedConsensusTransactionKind) -> Self {
        match kind {
            SequencedConsensusTransactionKind::External(ext) => {
                SerializableSequencedConsensusTransactionKind::External(Box::new(ext.clone()))
            }
            SequencedConsensusTransactionKind::System(txn) => {
                SerializableSequencedConsensusTransactionKind::System(Box::new(
                    txn.clone().serializable(),
                ))
            }
        }
    }
}

impl From<SerializableSequencedConsensusTransactionKind> for SequencedConsensusTransactionKind {
    fn from(kind: SerializableSequencedConsensusTransactionKind) -> Self {
        match kind {
            SerializableSequencedConsensusTransactionKind::External(ext) => {
                SequencedConsensusTransactionKind::External(*ext)
            }
            SerializableSequencedConsensusTransactionKind::System(txn) => {
                SequencedConsensusTransactionKind::System((*txn).into())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Hash, PartialEq, Eq, Debug, Ord, PartialOrd)]
pub enum SequencedConsensusTransactionKey {
    External(ConsensusTransactionKey),
    System(TransactionDigest),
}

impl SequencedConsensusTransactionKind {
    pub fn key(&self) -> SequencedConsensusTransactionKey {
        match self {
            SequencedConsensusTransactionKind::External(ext) => {
                SequencedConsensusTransactionKey::External(ext.key())
            }
            SequencedConsensusTransactionKind::System(txn) => {
                SequencedConsensusTransactionKey::System(*txn.digest())
            }
        }
    }

    pub fn get_tracking_id(&self) -> u64 {
        match self {
            SequencedConsensusTransactionKind::External(ext) => ext.get_tracking_id(),
            SequencedConsensusTransactionKind::System(_txn) => 0,
        }
    }

    pub fn is_executable_transaction(&self) -> bool {
        match self {
            SequencedConsensusTransactionKind::External(ext) => {
                ext.is_user_certificate() || ext.kind.is_user_transaction()
            }
            SequencedConsensusTransactionKind::System(_) => true,
        }
    }

    /// Returns the digest for transactions that get executed -- user-originated
    /// (`CertifiedTransaction` and `UserTransactionV1`) and system transactions
    /// -- and `None` for internal consensus messages (checkpoint signatures,
    /// capability notifications, randomness DKG, etc.). Used to build
    /// checkpoint roots and to schedule transactions for execution.
    pub fn executable_transaction_digest(&self) -> Option<TransactionDigest> {
        match self {
            SequencedConsensusTransactionKind::External(ext) => ext.kind.transaction_digest(),
            SequencedConsensusTransactionKind::System(txn) => Some(*txn.digest()),
        }
    }

    /// Returns the digest only for user-originated transaction kinds
    /// (`CertifiedTransaction` and `UserTransactionV1`). Used by post-consensus
    /// load shedding to guarantee that no internal consensus messages
    /// (checkpoint signatures, capability notifications, randomness DKG, etc.)
    /// and system transactions are eligible to be dropped.
    pub fn user_transaction_digest(&self) -> Option<TransactionDigest> {
        match self {
            SequencedConsensusTransactionKind::External(ext) => ext.kind.transaction_digest(),
            SequencedConsensusTransactionKind::System(_) => None,
        }
    }

    pub fn is_end_of_publish(&self) -> bool {
        match self {
            // Delegate to the `ConsensusTransaction` helper so the kind check
            // stays in one place.
            SequencedConsensusTransactionKind::External(ext) => ext.is_end_of_publish(),
            SequencedConsensusTransactionKind::System(_) => false,
        }
    }
}

impl SequencedConsensusTransaction {
    pub fn sender_authority(&self) -> AuthorityName {
        self.certificate_author
    }

    pub fn key(&self) -> SequencedConsensusTransactionKey {
        self.transaction.key()
    }

    pub fn is_end_of_publish(&self) -> bool {
        // Delegate to the `ConsensusTransaction` helper so the kind check
        // stays in one place.
        match &self.transaction {
            SequencedConsensusTransactionKind::External(ext) => ext.is_end_of_publish(),
            SequencedConsensusTransactionKind::System(_) => false,
        }
    }

    pub fn is_system(&self) -> bool {
        matches!(
            self.transaction,
            SequencedConsensusTransactionKind::System(_)
        )
    }

    /// Returns `true` if this is a user-originated transaction
    /// (`CertifiedTransaction` or `UserTransactionV1`) that uses randomness.
    /// Non-user kinds (internal consensus messages, system transactions) are
    /// always `false`, since only user transactions can request randomness.
    pub fn is_user_tx_with_randomness(&self) -> bool {
        match &self.transaction {
            SequencedConsensusTransactionKind::External(ext) => {
                ext.kind.as_sender_signed_transaction()
            }
            SequencedConsensusTransactionKind::System(_) => None,
        }
        .is_some_and(|data| data.uses_randomness())
    }

    /// Returns the transaction data if this is a user-originated
    /// (`CertifiedTransaction` or `UserTransactionV1`) or system transaction
    /// that touches at least one shared object, and `None` otherwise (internal
    /// consensus messages, or a transaction with only owned objects).
    pub fn as_shared_object_txn(&self) -> Option<&SenderSignedTransaction> {
        match &self.transaction {
            SequencedConsensusTransactionKind::External(ext) => ext
                .kind
                .as_sender_signed_transaction()
                .filter(|data| data.contains_shared_object()),
            SequencedConsensusTransactionKind::System(txn) => {
                txn.contains_shared_object().then(|| txn.data())
            }
        }
    }

    /// Returns `true` only for a raw, uncertified user transaction
    /// (`UserTransactionV1`). Certified user transactions and system
    /// transactions are not included.
    pub fn is_user_transaction(&self) -> bool {
        match &self.transaction {
            // Classification of user-transaction kinds lives on the enum helper,
            // so a new kind only has to be handled in one place there.
            SequencedConsensusTransactionKind::External(ext) => ext.kind.is_user_transaction(),
            SequencedConsensusTransactionKind::System(_) => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedSequencedConsensusTransaction(pub SequencedConsensusTransaction);

#[cfg(test)]
impl VerifiedSequencedConsensusTransaction {
    pub fn new_test(transaction: ConsensusTransaction) -> Self {
        Self(SequencedConsensusTransaction::new_test(transaction))
    }
}

impl SequencedConsensusTransaction {
    pub fn new_test(transaction: ConsensusTransaction) -> Self {
        Self {
            certificate_author_index: 0,
            certificate_author: AuthorityName::ZERO,
            consensus_index: Default::default(),
            transaction: SequencedConsensusTransactionKind::External(transaction),
        }
    }
}

/// Represents the information from the current consensus commit.
pub struct ConsensusCommitInfo {
    pub index: CommitIndex,
    pub round: u64,
    pub timestamp: u64,
    pub consensus_commit_digest: ConsensusCommitDigest,

    skip_consensus_commit_prologue_in_test: bool,
}

impl ConsensusCommitInfo {
    fn new(consensus_output: &impl ConsensusOutputAPI) -> Self {
        Self {
            index: consensus_output.commit_sub_dag_index(),
            round: consensus_output.leader_round(),
            timestamp: consensus_output.commit_timestamp_ms(),
            consensus_commit_digest: consensus_output.consensus_digest(),

            skip_consensus_commit_prologue_in_test: false,
        }
    }

    pub fn new_for_test(
        commit_index: CommitIndex,
        commit_round: u64,
        commit_timestamp: u64,
        skip_consensus_commit_prologue_in_test: bool,
    ) -> Self {
        Self {
            index: commit_index,
            round: commit_round,
            timestamp: commit_timestamp,
            consensus_commit_digest: ConsensusCommitDigest::default(),
            skip_consensus_commit_prologue_in_test,
        }
    }

    pub fn skip_consensus_commit_prologue_in_test(&self) -> bool {
        self.skip_consensus_commit_prologue_in_test
    }

    fn consensus_commit_prologue_v1_transaction(
        &self,
        epoch: u64,
        cancelled_transactions: Vec<CanceledTransaction>,
    ) -> VerifiedExecutableTransaction {
        let transaction = VerifiedTransaction::new_consensus_commit_prologue_v1(
            epoch,
            self.round,
            self.timestamp,
            self.consensus_commit_digest,
            cancelled_transactions,
        );
        VerifiedExecutableTransaction::new_system(transaction, epoch)
    }

    pub fn create_consensus_commit_prologue_transaction(
        &self,
        epoch: u64,
        cancelled_transactions: Vec<CanceledTransaction>,
    ) -> VerifiedExecutableTransaction {
        self.consensus_commit_prologue_v1_transaction(epoch, cancelled_transactions)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use arc_swap::ArcSwap;
    use futures::pin_mut;
    use iota_protocol_config::{Chain, ConsensusTransactionOrdering, ProtocolConfig};
    use iota_sdk_types::{Address, ObjectId, SenderSignedTransaction, Transaction};
    use iota_swarm_config::network_config::NetworkConfig;
    use iota_types::{
        base_types::{AuthorityName, random_object_ref},
        committee::Committee,
        crypto::{
            AccountPrivateKey, KeypairTraits, deterministic_random_account_private_key,
            get_key_pair,
        },
        effects::TransactionEffectsAPI,
        error::{IotaError, UserInputError},
        messages_consensus::{
            AuthorityCapabilitiesV1, ConsensusTransaction, ConsensusTransactionKind,
        },
        object::Object,
        supported_protocol_versions::{
            SupportedProtocolVersions, SupportedProtocolVersionsWithHashes,
        },
        transaction::{
            CertifiedTransaction, TEST_ONLY_GAS_UNIT_FOR_TRANSFER, TransactionAPI, TransactionKey,
        },
        utils::{
            make_move_authenticator_tx_with_inconsistent_shared_input, to_sender_signed_transaction,
        },
    };
    use prometheus_filtered::Registry;
    use starfish_core::{
        BlockHeaderAPI, CommitDigest, CommitRef, CommitmentVerifiedTransactions, CommittedSubDag,
        TestBlockHeader, VerifiedBlockHeader,
    };

    use super::*;
    use crate::{
        authority::{
            AuthorityMetrics, authority_per_epoch_store::ConsensusStatsAPI,
            backpressure::BackpressureManager, test_authority_builder::TestAuthorityBuilder,
        },
        checkpoints::CheckpointServiceNoop,
        consensus_adapter::consensus_tests::{test_certificates, test_gas_objects},
        post_consensus_tx_reorder::PostConsensusTxReorder,
        post_consensus_validation::PostConsensusVerdict,
    };

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    pub async fn test_consensus_handler() {
        telemetry_subscribers::init_for_testing();

        // GIVEN
        let mut objects = test_gas_objects();
        let shared_object = Object::shared_for_testing();
        objects.push(shared_object.clone());

        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .with_objects(objects.clone())
                .build();

        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;

        let epoch_store = state.epoch_store_for_testing().clone();
        let new_epoch_start_state = epoch_store.epoch_start_state();
        let consensus_committee = get_consensus_committee(new_epoch_start_state);

        let metrics = Arc::new(AuthorityMetrics::new(&Registry::new()));

        let backpressure_manager = BackpressureManager::new_for_tests();

        let mut consensus_handler = ConsensusHandler::new(
            epoch_store,
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            consensus_committee.clone(),
            metrics,
            backpressure_manager.subscribe(),
        );

        // AND
        // Create test transactions
        let transactions = test_certificates(&state, shared_object).await;
        let mut headers = Vec::new();
        let mut subdag_transactions = Vec::new();

        for (i, transaction) in transactions.iter().enumerate() {
            let transaction_bytes: Vec<u8> = bcs::to_bytes(
                &ConsensusTransaction::new_certificate_message(&state.name, transaction.clone()),
            )
            .unwrap();

            // AND create a block header + separate transactions batch for each
            // transaction. In Starfish, transactions live on the subdag
            // alongside (not inside) the block headers.
            let header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(100 + i as u32, (i % consensus_committee.size()) as u8)
                    .build(),
            );
            let tx_batch = CommitmentVerifiedTransactions::new_for_test(
                &header,
                vec![starfish_core::Transaction::new(transaction_bytes)],
            );
            headers.push(header);
            subdag_transactions.push(tx_batch);
        }

        // AND create the consensus output
        let leader_header = headers[0].clone();
        let committed_header_refs: Vec<_> = headers.iter().map(|h| h.reference()).collect();
        let committed_sub_dag = CommittedSubDag::new(
            leader_header.reference(),
            headers.clone(),
            committed_header_refs,
            subdag_transactions,
            leader_header.timestamp_ms(),
            CommitRef::new(10, CommitDigest::MIN),
            vec![],
            vec![],
        );

        // Test that the consensus handler respects backpressure.
        backpressure_manager.set_backpressure(true);
        // Default watermarks are 0,0 which will suppress the backpressure.
        backpressure_manager.update_highest_certified_checkpoint(1);

        // AND processing the consensus output once
        {
            let waiter =
                consensus_handler.handle_consensus_output_for_test(committed_sub_dag.clone());
            pin_mut!(waiter);

            // waiter should not complete within 5 seconds
            tokio::time::timeout(Duration::from_secs(5), &mut waiter)
                .await
                .unwrap_err();

            // lift backpressure
            backpressure_manager.set_backpressure(false);

            // waiter completes now.
            tokio::time::timeout(Duration::from_secs(100), waiter)
                .await
                .unwrap();
        }

        // AND capturing the consensus stats
        let num_blocks = headers.len();
        let num_transactions = transactions.len();
        let last_consensus_stats_1 = consensus_handler.last_consensus_stats.clone();
        assert_eq!(
            last_consensus_stats_1.index.transaction_index,
            num_transactions as u64
        );
        assert_eq!(last_consensus_stats_1.index.sub_dag_index, 10_u64);
        assert_eq!(last_consensus_stats_1.index.last_committed_round, 100_u64);
        assert_eq!(last_consensus_stats_1.hash, 0);
        assert_eq!(
            last_consensus_stats_1.stats.get_num_messages(0),
            num_blocks as u64
        );
        assert_eq!(
            last_consensus_stats_1.stats.get_num_user_transactions(0),
            num_transactions as u64
        );

        // AND the certificates execute. Bounded: a lost or mismatched assignment
        // leaves the transaction waiting forever.
        let digests: Vec<_> = transactions.iter().map(|tx| *tx.digest()).collect();
        tokio::time::timeout(
            Duration::from_secs(60),
            state
                .get_transaction_cache_reader()
                .notify_read_executed_effects_for_testing("", &digests),
        )
        .await
        .expect("consensus-scheduled certificates did not execute");

        // WHEN processing the same output multiple times
        // THEN the consensus stats do not update
        for _ in 0..2 {
            consensus_handler
                .handle_consensus_output_for_test(committed_sub_dag.clone())
                .await;
            let last_consensus_stats_2 = consensus_handler.last_consensus_stats.clone();
            assert_eq!(last_consensus_stats_1, last_consensus_stats_2);
        }
    }

    /// Once this node executes synced checkpoints for the rest of the epoch, a
    /// commit still waiting is abandoned so the switch can complete, and later
    /// commits are not processed.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_consensus_handler_stops_when_executing_synced_checkpoints() {
        telemetry_subscribers::init_for_testing();
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir().build();
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;
        let epoch_store = state.epoch_store_for_testing().clone();
        let consensus_committee = get_consensus_committee(epoch_store.epoch_start_state());
        let backpressure_manager = BackpressureManager::new_for_tests();
        let mut consensus_handler = ConsensusHandler::new(
            epoch_store.clone(),
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            consensus_committee,
            Arc::new(AuthorityMetrics::new(&Registry::new())),
            backpressure_manager.subscribe(),
        );
        let commit = |round: u32, index: u32| {
            let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 0).build());
            CommittedSubDag::new(
                header.reference(),
                vec![header.clone()],
                vec![header.reference()],
                vec![],
                header.timestamp_ms(),
                CommitRef::new(index, CommitDigest::MIN),
                vec![],
                vec![],
            )
        };
        let initial_stats = consensus_handler.last_consensus_stats.clone();

        // The first commit waits on backpressure.
        backpressure_manager.set_backpressure(true);
        backpressure_manager.update_highest_certified_checkpoint(1);
        {
            let waiter = consensus_handler.handle_consensus_output_for_test(commit(100, 10));
            pin_mut!(waiter);
            tokio::time::timeout(Duration::from_secs(5), &mut waiter)
                .await
                .unwrap_err();

            let start = epoch_store.start_executing_synced_checkpoints();
            pin_mut!(start);
            tokio::time::timeout(
                Duration::from_secs(5),
                futures::future::join(waiter.as_mut(), start.as_mut()),
            )
            .await
            .expect("the waiting commit should be abandoned and the switch complete");
        }
        assert_eq!(consensus_handler.last_consensus_stats, initial_stats);

        // Later commits are not processed.
        backpressure_manager.set_backpressure(false);
        consensus_handler
            .handle_consensus_output_for_test(commit(101, 11))
            .await;
        assert_eq!(consensus_handler.last_consensus_stats, initial_stats);
    }

    /// With deterministic validation on, a commit carrying a user transaction
    /// after the switch to executing synced checkpoints is not processed: the
    /// transaction is not validated, assigned to the commit, put in a pending
    /// checkpoint or executed. The commit before the switch shows each of
    /// these for a processed commit.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_consensus_handler_stops_when_executing_synced_checkpoints_with_validation() {
        telemetry_subscribers::init_for_testing();
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let objects: Vec<Object> = (0..4)
            .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
            .collect();
        let network = iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
            .with_objects(objects.clone())
            .build();
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network, 0)
            .build()
            .await;
        let epoch_store = state.epoch_store_for_testing().clone();
        assert!(epoch_store.pcool_bookkeeping_enabled());
        let rgp = epoch_store.reference_gas_price();
        let transfer = |object: &Object, gas: &Object| {
            let transaction = to_sender_signed_transaction(
                Transaction::new_transfer(
                    Address::random(),
                    state.get_object(&object.id()).unwrap().object_ref(),
                    sender,
                    state.get_object(&gas.id()).unwrap().object_ref(),
                    rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
                    rgp,
                ),
                &key,
            );
            let digest = *transaction.digest();
            let consensus_tx = ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransactionV1(Box::new(transaction)),
                tracking_id: Default::default(),
            };
            (digest, consensus_tx)
        };
        let in_pending_checkpoint = |digest: TransactionDigest| {
            epoch_store
                .get_pending_checkpoints(None)
                .unwrap()
                .iter()
                .flat_map(|(_, checkpoint)| checkpoint.roots())
                .any(|root| *root == TransactionKey::Digest(digest))
        };
        let executed = |digest: TransactionDigest| {
            let reader = state.get_transaction_cache_reader().clone();
            tokio::time::timeout(Duration::from_secs(60), async move {
                reader
                    .notify_read_executed_effects_for_testing("", &[digest])
                    .await
            })
        };
        let mut consensus_handler = ConsensusHandler::new(
            epoch_store.clone(),
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            get_consensus_committee(epoch_store.epoch_start_state()),
            state.metrics.clone(),
            BackpressureManager::new_for_tests().subscribe(),
        );

        let (before, before_tx) = transfer(&objects[0], &objects[1]);
        consensus_handler
            .handle_consensus_output_for_test(commit_with_transactions(
                1,
                100,
                vec![(0, before_tx)],
            ))
            .await;
        assert_eq!(
            epoch_store.post_consensus_verdict(1, before).unwrap(),
            Some(PostConsensusVerdict::Kept)
        );
        assert!(in_pending_checkpoint(before));
        executed(before)
            .await
            .expect("the transaction of the commit before the switch must execute");
        let stats_before_switch = consensus_handler.last_consensus_stats.clone();
        assert_eq!(stats_before_switch.index.sub_dag_index, 1);

        epoch_store.start_executing_synced_checkpoints().await;
        let (after, after_tx) = transfer(&objects[2], &objects[3]);
        consensus_handler
            .handle_consensus_output_for_test(commit_with_transactions(2, 101, vec![(0, after_tx)]))
            .await;

        assert_eq!(consensus_handler.last_consensus_stats, stats_before_switch);
        assert_eq!(epoch_store.last_validated_commit().unwrap(), Some(1));
        assert_eq!(epoch_store.post_consensus_verdict(2, after).unwrap(), None);
        assert_eq!(
            epoch_store
                .handler_object_state_for_testing()
                .commit_index_of(&TransactionKey::Digest(after)),
            None
        );
        assert!(!in_pending_checkpoint(after));
        assert!(
            executed(after).await.is_err(),
            "a transaction of a commit after the switch must not be scheduled"
        );
    }

    /// A node that switched to executing synced checkpoints and restarts in
    /// the same epoch processes the commits it processed before the switch
    /// again, and stops at the first one after them: synced checkpoints it
    /// executed since the switch wrote outputs that validating a later commit
    /// would read.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_consensus_handler_restarted_after_the_switch_stops_after_the_same_commit() {
        telemetry_subscribers::init_for_testing();
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir().build();
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;
        let epoch_store = state.epoch_store_for_testing().clone();
        let consensus_committee = get_consensus_committee(epoch_store.epoch_start_state());
        let backpressure_manager = BackpressureManager::new_for_tests();
        let new_handler = |epoch_store: Arc<AuthorityPerEpochStore>| {
            ConsensusHandler::new(
                epoch_store,
                state.clone(),
                Arc::new(CheckpointServiceNoop {}),
                state.execution_scheduler().clone(),
                state.get_object_cache_reader().clone(),
                Arc::new(ArcSwap::default()),
                consensus_committee.clone(),
                Arc::new(AuthorityMetrics::new(&Registry::new())),
                backpressure_manager.subscribe(),
            )
        };
        let commit = |round: u32, index: u32| {
            let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 0).build());
            CommittedSubDag::new(
                header.reference(),
                vec![header.clone()],
                vec![header.reference()],
                vec![],
                header.timestamp_ms(),
                CommitRef::new(index, CommitDigest::MIN),
                vec![],
                vec![],
            )
        };

        let mut consensus_handler = new_handler(epoch_store.clone());
        for (round, index) in [(100, 1), (101, 2)] {
            consensus_handler
                .handle_consensus_output_for_test(commit(round, index))
                .await;
        }
        assert_eq!(consensus_handler.last_processed_subdag_index(), 2);
        epoch_store.start_executing_synced_checkpoints().await;
        drop(consensus_handler);

        let reopened =
            crate::authority::authority_per_epoch_store::authority_per_epoch_store_tests::reopen(
                &state,
                &epoch_store,
            );
        // The commit boundary insists on a randomness manager; one whose DKG
        // never completes keeps randomness out of the commits.
        let randomness_manager = crate::epoch::randomness::RandomnessManager::try_new(
            Arc::downgrade(&reopened),
            Box::new(crate::mock_consensus::MockConsensusClient::new(
                Arc::downgrade(&state),
                crate::mock_consensus::ConsensusMode::Noop,
            )),
            iota_network::randomness::Handle::new_stub(),
            network_config.validator_configs()[0].authority_key_pair(),
        )
        .await
        .unwrap();
        reopened
            .set_randomness_manager(randomness_manager)
            .await
            .unwrap();
        let mut consensus_handler = new_handler(reopened.clone());
        for (round, index) in [(100, 1), (101, 2), (102, 3)] {
            consensus_handler
                .handle_consensus_output_for_test(commit(round, index))
                .await;
        }
        assert_eq!(
            consensus_handler.last_processed_subdag_index(),
            2,
            "the restarted handler must not process a commit after the last one it processed \
             before the switch"
        );
        assert!(reopened.is_executing_synced_checkpoints());
    }

    /// A commit mixing a shared-object certificate with an owned-object-only
    /// one. Version assignment emits no entry for the owned certificate, so it
    /// must fall back to an empty `ExecutionEnv` — the `unwrap_or_default()` in
    /// `AsyncTransactionScheduler::run`. Both must execute: a panic there would
    /// lose every owned-only transaction of a commit.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_consensus_handler_mixed_owned_and_shared_commit() {
        // GIVEN
        let (sender, keypair) = deterministic_random_account_private_key();
        let mut objects = test_gas_objects();
        let shared_object = Object::shared_for_testing();
        let owned_object = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let owned_gas_object = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        objects.push(shared_object.clone());
        objects.push(owned_object.clone());
        objects.push(owned_gas_object.clone());

        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .with_objects(objects.clone())
                .build();

        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;

        let epoch_store = state.epoch_store_for_testing().clone();
        let new_epoch_start_state = epoch_store.epoch_start_state();
        let consensus_committee = get_consensus_committee(new_epoch_start_state);

        let metrics = Arc::new(AuthorityMetrics::new(&Registry::new()));
        let backpressure_manager = BackpressureManager::new_for_tests();

        let mut consensus_handler = ConsensusHandler::new(
            epoch_store.clone(),
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            consensus_committee.clone(),
            metrics,
            backpressure_manager.subscribe(),
        );

        // AND one shared-object certificate...
        let shared_certificate = test_certificates(&state, shared_object.clone())
            .await
            .swap_remove(0);

        // ...and one owned-object-only certificate (a transfer).
        let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
        let rgp = epoch_store.reference_gas_price();
        let owned_ref = state.get_object(&owned_object.id()).unwrap().object_ref();
        let gas_ref = state
            .get_object(&owned_gas_object.id())
            .unwrap()
            .object_ref();
        let transfer_data = Transaction::new_transfer(
            recipient,
            owned_ref,
            sender,
            gas_ref,
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        );
        let transfer_transaction = epoch_store
            .verify_transaction(to_sender_signed_transaction(transfer_data, &keypair))
            .unwrap();
        let response = state
            .handle_transaction(&epoch_store, transfer_transaction.clone())
            .await
            .unwrap();
        let vote = response.status.into_signed_for_testing();
        let owned_certificate = CertifiedTransaction::new(
            transfer_transaction.into_message(),
            vec![vote],
            &state.clone_committee_for_testing(),
        )
        .unwrap();

        // The owned-only certificate goes first so that a positional pairing
        // would shift the shared certificate onto the empty tail of the
        // assignment list, failing the bounded wait below. With it last, such a
        // pairing would coincidentally hand every transaction the same env a
        // lookup by key does, and the test would pass either way.
        let certificates = [owned_certificate.clone(), shared_certificate.clone()];
        let mut headers = Vec::new();
        let mut subdag_transactions = Vec::new();
        for (i, certificate) in certificates.iter().enumerate() {
            let transaction_bytes = bcs::to_bytes(&ConsensusTransaction::new_certificate_message(
                &state.name,
                certificate.clone(),
            ))
            .unwrap();
            let header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(100 + i as u32, (i % consensus_committee.size()) as u8)
                    .build(),
            );
            let tx_batch = CommitmentVerifiedTransactions::new_for_test(
                &header,
                vec![starfish_core::Transaction::new(transaction_bytes)],
            );
            headers.push(header);
            subdag_transactions.push(tx_batch);
        }
        let leader_header = headers[0].clone();
        let committed_header_refs: Vec<_> = headers.iter().map(|h| h.reference()).collect();
        let committed_sub_dag = CommittedSubDag::new(
            leader_header.reference(),
            headers.clone(),
            committed_header_refs,
            subdag_transactions,
            leader_header.timestamp_ms(),
            CommitRef::new(10, CommitDigest::MIN),
            vec![],
            vec![],
        );

        // WHEN processing the consensus output
        consensus_handler
            .handle_consensus_output_for_test(committed_sub_dag)
            .await;

        // THEN both certificates execute. Bounded, as above.
        let digests = [*shared_certificate.digest(), *owned_certificate.digest()];
        let effects = tokio::time::timeout(
            Duration::from_secs(60),
            state
                .get_transaction_cache_reader()
                .notify_read_executed_effects_for_testing("", &digests),
        )
        .await
        .expect("both certificates of the mixed commit must execute");

        // AND the shared certificate executed on the consensus-assigned shared
        // object version, while the owned certificate touched no shared input.
        let shared_effects = &effects[0];
        let owned_effects = &effects[1];
        assert_eq!(
            shared_effects
                .input_shared_objects()
                .into_iter()
                .map(|iso| {
                    let oref = iso.object_reference();
                    (oref.object_id, oref.version)
                })
                .collect::<Vec<_>>(),
            vec![(shared_object.id(), shared_object.version())]
        );
        // Pins that the transaction is owned-only — effects cannot show which env
        // it received.
        assert!(
            owned_effects.input_shared_objects().is_empty(),
            "the transaction paired with the empty env must be owned-only"
        );
    }

    /// With deterministic validation on, a dropped user transaction does not
    /// enter the processed-transaction cache, so a retry is validated at every
    /// commit it appears in. Once kept, the transaction is cached and a later
    /// copy is skipped before validation.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn retries_of_a_dropped_transaction_reach_validation_until_it_is_kept() {
        use iota_types::transaction::TransactionKey;

        use crate::{
            authority::authority_per_epoch_store::handler_object_state::{
                HandlerProcessedObject, HandlerProcessedObjectKind,
            },
            post_consensus_validation::PostConsensusVerdict,
        };

        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let object = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let gas = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let network = iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
            .with_objects(vec![object.clone(), gas.clone()])
            .build();
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network, 0)
            .build()
            .await;
        let epoch = state.epoch_store_for_testing().clone();
        let rgp = epoch.reference_gas_price();
        let object = state.get_object(&object.id()).unwrap();
        let gas = state.get_object(&gas.id()).unwrap();
        let transaction = to_sender_signed_transaction(
            Transaction::new_transfer(
                Address::random(),
                object.object_ref(),
                sender,
                gas.object_ref(),
                rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
                rgp,
            ),
            &key,
        );
        let digest = *transaction.digest();
        let consensus_tx = ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(Box::new(transaction)),
            tracking_id: Default::default(),
        };
        // An input produced at commit 1: with the horizon two commits back,
        // validation at commits 1 and 2 drops the transaction and validation
        // at commit 3 keeps it.
        epoch
            .flush_commit_rows_for_testing(
                0,
                vec![(
                    iota_types::storage::ObjectKey::from(object.object_ref()),
                    HandlerProcessedObject {
                        produced_at: 1,
                        digest: object.digest(),
                        kind: HandlerProcessedObjectKind::Live,
                        initial_shared_version: None,
                        replaced_version: None,
                        deleted_by: None,
                    },
                )],
            )
            .unwrap();
        let backpressure = BackpressureManager::new_for_tests();
        let mut handler = ConsensusHandler::new(
            epoch.clone(),
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            get_consensus_committee(epoch.epoch_start_state()),
            state.metrics.clone(),
            backpressure.subscribe(),
        );
        let bytes = bcs::to_bytes(&consensus_tx).unwrap();
        let cache_key = SequencedConsensusTransactionKey::External(consensus_tx.key());
        for index in 1..=4 {
            let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(index, 0).build());
            let batch = CommitmentVerifiedTransactions::new_for_test(
                &header,
                vec![starfish_core::Transaction::new(bytes.clone())],
            );
            let subdag = CommittedSubDag::new(
                header.reference(),
                vec![header.clone()],
                vec![header.reference()],
                vec![batch],
                header.timestamp_ms(),
                CommitRef::new(index, CommitDigest::MIN),
                vec![],
                vec![],
            );
            handler.handle_consensus_output_for_test(subdag).await;
            let verdict = epoch.post_consensus_verdict(index as u64, digest).unwrap();
            match index {
                1 | 2 => assert!(matches!(verdict, Some(PostConsensusVerdict::Missing(_)))),
                3 => assert_eq!(verdict, Some(PostConsensusVerdict::Kept)),
                4 => assert_eq!(verdict, None),
                _ => unreachable!(),
            }
            assert_eq!(handler.processed_cache.contains(&cache_key), index >= 3);
        }
        let roots = epoch.get_pending_checkpoints(None).unwrap();
        assert_eq!(
            roots
                .iter()
                .flat_map(|(_, checkpoint)| checkpoint.roots())
                .filter(|root| **root == TransactionKey::Digest(digest))
                .count(),
            1
        );
    }

    /// A transaction dropped because an input is not visible at the commit's
    /// horizon yet can succeed when sequenced again. The submitter waiting at
    /// the time of the drop learns about it, but the drop is not kept as the
    /// digest's status: a later status request or resubmission waits for the
    /// retry's outcome instead of getting the old error back at once.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn drop_for_an_input_not_yet_at_the_horizon_does_not_answer_later_status_requests() {
        use crate::authority::authority_per_epoch_store::handler_object_state::{
            HandlerProcessedObject, HandlerProcessedObjectKind,
        };

        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let object = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let gas = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let network = iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
            .with_objects(vec![object.clone(), gas.clone()])
            .build();
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network, 0)
            .build()
            .await;
        let epoch = state.epoch_store_for_testing().clone();
        let rgp = epoch.reference_gas_price();
        let object = state.get_object(&object.id()).unwrap();
        let gas = state.get_object(&gas.id()).unwrap();
        let transaction = to_sender_signed_transaction(
            Transaction::new_transfer(
                Address::random(),
                object.object_ref(),
                sender,
                gas.object_ref(),
                rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
                rgp,
            ),
            &key,
        );
        let digest = *transaction.digest();
        let consensus_tx = ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(Box::new(transaction)),
            tracking_id: Default::default(),
        };
        // An input produced at commit 1: with the horizon two commits back,
        // validation at commit 1 answers that the input is not visible yet.
        epoch
            .flush_commit_rows_for_testing(
                0,
                vec![(
                    iota_types::storage::ObjectKey::from(object.object_ref()),
                    HandlerProcessedObject {
                        produced_at: 1,
                        digest: object.digest(),
                        kind: HandlerProcessedObjectKind::Live,
                        initial_shared_version: None,
                        replaced_version: None,
                        deleted_by: None,
                    },
                )],
            )
            .unwrap();
        let backpressure = BackpressureManager::new_for_tests();
        let mut handler = ConsensusHandler::new(
            epoch.clone(),
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            get_consensus_committee(epoch.epoch_start_state()),
            state.metrics.clone(),
            backpressure.subscribe(),
        );
        // Polled once so it is registered before the commit is processed.
        let waiting_submitter = epoch.notify_read_dropped_digests(digest);
        pin_mut!(waiting_submitter);
        assert!(futures::poll!(&mut waiting_submitter).is_pending());

        let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(1, 0).build());
        let batch = CommitmentVerifiedTransactions::new_for_test(
            &header,
            vec![starfish_core::Transaction::new(
                bcs::to_bytes(&consensus_tx).unwrap(),
            )],
        );
        let subdag = CommittedSubDag::new(
            header.reference(),
            vec![header.clone()],
            vec![header.reference()],
            vec![batch],
            header.timestamp_ms(),
            CommitRef::new(1, CommitDigest::MIN),
            vec![],
            vec![],
        );
        handler.handle_consensus_output_for_test(subdag).await;

        let notified = tokio::time::timeout(Duration::from_secs(5), waiting_submitter)
            .await
            .expect("the submitter waiting at the drop must be told about it");
        assert!(
            matches!(notified, IotaError::UserInput { .. }),
            "{notified:?}"
        );
        assert!(
            tokio::time::timeout(
                Duration::from_secs(5),
                epoch.notify_read_dropped_digests(digest)
            )
            .await
            .is_err(),
            "a status request after the drop must wait for the retry's outcome"
        );
    }

    /// The digest does not cover the signatures, so a faulty validator can
    /// put in its block a copy of an honest transaction whose Move
    /// authenticator fails validation at the commit. When the copy is dropped
    /// before the honest transaction is kept, a status request for the digest
    /// after the keep waits for the kept transaction's outcome instead of
    /// getting the copy's drop.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn drop_of_a_failing_copy_does_not_answer_for_the_kept_transaction() {
        use iota_sdk_types::TransactionExpiration;

        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let object = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let gas = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let network = iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
            .with_objects(vec![object.clone(), gas.clone()])
            .build();
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network, 0)
            .build()
            .await;
        let epoch = state.epoch_store_for_testing().clone();
        let rgp = epoch.reference_gas_price();
        let object = state.get_object(&object.id()).unwrap();
        let gas = state.get_object(&gas.id()).unwrap();
        let mut data = Transaction::new_transfer(
            Address::random(),
            object.object_ref(),
            sender,
            gas.object_ref(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        );
        *data.expiration_mut_for_testing() = TransactionExpiration::Epoch(epoch.epoch());
        let honest = to_sender_signed_transaction(data.clone(), &key);
        let failing_copy = copy_with_failing_authenticator(data);
        let digest = *honest.digest();
        assert_eq!(*failing_copy.digest(), digest);

        let mut handler = ConsensusHandler::new(
            epoch.clone(),
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            get_consensus_committee(epoch.epoch_start_state()),
            state.metrics.clone(),
            BackpressureManager::new_for_tests().subscribe(),
        );
        for (index, transaction) in (1..).zip([failing_copy, honest]) {
            handler
                .handle_consensus_output_for_test(commit_with_transactions(
                    index,
                    index * 100,
                    vec![(0, ConsensusTransaction::new_user_transaction(transaction))],
                ))
                .await;
        }

        assert!(
            matches!(
                epoch.post_consensus_verdict(1, digest).unwrap(),
                Some(PostConsensusVerdict::DroppedCopy(_))
            ),
            "the failing copy must be dropped"
        );
        assert_eq!(
            epoch.post_consensus_verdict(2, digest).unwrap(),
            Some(PostConsensusVerdict::Kept)
        );
        assert!(
            tokio::time::timeout(
                Duration::from_secs(5),
                epoch.notify_read_dropped_digests(digest)
            )
            .await
            .is_err(),
            "a status request must wait for the kept transaction's outcome"
        );
    }

    /// Two copies of an honest transaction in one commit, a faulty one first
    /// in consensus order: the faulty validator's copy has a Move
    /// authenticator that fails validation at the commit. The honest copy
    /// from another author must still be validated and kept at that commit.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn failing_copy_first_in_a_commit_does_not_hide_the_honest_copy() {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let objects: Vec<_> = (0..2)
            .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
            .collect();
        let ids: Vec<_> = objects.iter().map(|object| object.id()).collect();
        let (state, epoch, mut handler) = deterministic_validation_handler(objects).await;
        let honest = transfer_in_current_epoch(&state, &epoch, sender, &key, ids[0], ids[1]);
        let failing_copy = copy_with_failing_authenticator(honest.data().transaction().clone());
        let digest = *honest.digest();
        assert_eq!(*failing_copy.digest(), digest);

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                1,
                100,
                vec![
                    (1, ConsensusTransaction::new_user_transaction(failing_copy)),
                    (0, ConsensusTransaction::new_user_transaction(honest)),
                ],
            ))
            .await;

        assert_eq!(
            epoch.post_consensus_verdict(1, digest).unwrap(),
            Some(PostConsensusVerdict::Kept),
            "the honest copy must be kept at the commit"
        );
        assert!(
            epoch
                .is_consensus_message_processed(&user_transaction_key(digest))
                .unwrap(),
            "the honest copy must be scheduled"
        );
    }

    /// A submitter already waiting for the status of an honest transaction
    /// when a faulty validator's copy of it is dropped on its authenticator
    /// is not told about the drop: the honest copy can still be kept, so the
    /// submitter keeps waiting for its outcome. The consensus adapter that
    /// submitted the transaction stops waiting for it at the drop, which
    /// frees its in-flight slot. The same holds when the commit is replayed
    /// over its saved verdicts, as after a restart.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn waiting_submitter_is_not_told_about_the_drop_of_a_failing_copy() {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let saved_error: IotaError =
            UserInputError::Unsupported("saved drop of a copy".to_string()).into();
        for saved in [None, Some(saved_error)] {
            assert_only_the_adapter_is_told_about_the_drop_of_a_failing_copy(saved).await;
        }
    }

    /// Submits an honest transaction through a consensus adapter, then
    /// handles a commit with a failing copy of it and a commit with the honest
    /// transaction. With `saved`, the failing copy's drop is saved with that
    /// error for the first commit before it is handled, so the commit repeats
    /// the saved verdict instead of validating the copy.
    async fn assert_only_the_adapter_is_told_about_the_drop_of_a_failing_copy(
        saved: Option<IotaError>,
    ) {
        use crate::{
            consensus_adapter::{
                ConnectionMonitorStatusForTests, ConsensusAdapter, ConsensusAdapterMetrics,
            },
            mock_consensus::{ConsensusMode, MockConsensusClient},
        };

        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let objects: Vec<_> = (0..2)
            .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
            .collect();
        let ids: Vec<_> = objects.iter().map(|object| object.id()).collect();
        let (state, epoch, mut handler) = deterministic_validation_handler(objects).await;
        let honest = transfer_in_current_epoch(&state, &epoch, sender, &key, ids[0], ids[1]);
        let failing_copy = copy_with_failing_authenticator(honest.data().transaction().clone());
        let digest = *honest.digest();
        if let Some(error) = &saved {
            epoch
                .persist_post_consensus_verdicts(
                    1,
                    &[(digest, PostConsensusVerdict::DroppedCopy(error.clone()))],
                )
                .unwrap();
        }

        // A consensus client that sequences nothing, so only the drop can end
        // the adapter's wait.
        let adapter = Arc::new(ConsensusAdapter::new(
            Arc::new(MockConsensusClient::new(
                Arc::downgrade(&state),
                ConsensusMode::Noop,
            )),
            state.checkpoint_store.clone(),
            state.name,
            Arc::new(ConnectionMonitorStatusForTests {}),
            100_000,
            100_000,
            None,
            None,
            ConsensusAdapterMetrics::new_test(),
            50,
        ));
        let adapter_wait = adapter
            .submit(
                ConsensusTransaction::new_user_transaction(honest.clone()),
                None,
                &epoch,
            )
            .unwrap();
        // Lets the adapter's task register its wait before the commit.
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(!adapter_wait.is_finished());
        assert_eq!(adapter.num_inflight_transactions(), 1);

        // Polled once so it is registered before the commit is processed.
        let waiting_submitter = epoch.notify_read_dropped_digests(digest);
        pin_mut!(waiting_submitter);
        assert!(futures::poll!(&mut waiting_submitter).is_pending());

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                1,
                100,
                vec![(1, ConsensusTransaction::new_user_transaction(failing_copy))],
            ))
            .await;

        let verdict = epoch.post_consensus_verdict(1, digest).unwrap();
        match &saved {
            Some(error) => assert_eq!(
                verdict,
                Some(PostConsensusVerdict::DroppedCopy(error.clone())),
                "the commit must repeat the saved verdict"
            ),
            None => assert!(
                matches!(verdict, Some(PostConsensusVerdict::DroppedCopy(_))),
                "{verdict:?}"
            ),
        }
        tokio::time::timeout(Duration::from_secs(5), adapter_wait)
            .await
            .expect("the adapter must stop waiting at the drop of the copy")
            .unwrap();
        assert_eq!(adapter.num_inflight_transactions(), 0);
        assert!(
            tokio::time::timeout(Duration::from_secs(5), &mut waiting_submitter)
                .await
                .is_err(),
            "the submitter must not get the failing copy's drop"
        );
        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                2,
                200,
                vec![(0, ConsensusTransaction::new_user_transaction(honest))],
            ))
            .await;
        assert_eq!(
            epoch.post_consensus_verdict(2, digest).unwrap(),
            Some(PostConsensusVerdict::Kept)
        );
    }

    /// `data` with a Move authenticator for its sender in place of its
    /// signature. Block verification checks such an authenticator only for
    /// its address, and declaring a mutable shared input fails its validity
    /// check at the commit.
    fn copy_with_failing_authenticator(
        data: Transaction,
    ) -> iota_types::transaction::TransactionEnvelope {
        use iota_sdk_types::{
            MoveAuthenticator, MoveAuthenticatorV1, SharedObjectReference, UserSignature, Version,
        };
        use iota_types::transaction::{CallArg, TransactionEnvelope};

        let sender = data.sender();
        TransactionEnvelope::new(SenderSignedTransaction::new(
            data,
            vec![UserSignature::MoveAuthenticator(MoveAuthenticator::from(
                MoveAuthenticatorV1::new(
                    vec![CallArg::Shared(SharedObjectReference::new(
                        ObjectId::random(),
                        Version::from(1),
                        true,
                    ))],
                    vec![],
                    SharedObjectReference::new(sender.into(), Version::from(1), false),
                ),
            ))],
        ))
    }

    /// A handler with deterministic validation on, over an authority of a
    /// four-member committee whose genesis gives `sender` the `objects`.
    async fn deterministic_validation_handler(
        objects: Vec<Object>,
    ) -> (
        Arc<AuthorityState>,
        Arc<AuthorityPerEpochStore>,
        ConsensusHandler<CheckpointServiceNoop>,
    ) {
        let network = iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
            .committee_size(NonZeroUsize::new(4).unwrap())
            .with_objects(objects)
            .build();
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network, 0)
            .build()
            .await;
        let epoch = state.epoch_store_for_testing().clone();
        let handler = ConsensusHandler::new(
            epoch.clone(),
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            get_consensus_committee(epoch.epoch_start_state()),
            state.metrics.clone(),
            BackpressureManager::new_for_tests().subscribe(),
        );
        (state, epoch, handler)
    }

    /// A key-signed transfer of `object` by `sender`, paid with `gas`, valid
    /// in `epoch`'s epoch only.
    fn transfer_in_current_epoch(
        state: &AuthorityState,
        epoch: &AuthorityPerEpochStore,
        sender: Address,
        key: &AccountPrivateKey,
        object: ObjectId,
        gas: ObjectId,
    ) -> iota_types::transaction::TransactionEnvelope {
        let rgp = epoch.reference_gas_price();
        let mut data = Transaction::new_transfer(
            Address::random(),
            state.get_object(&object).unwrap().object_ref(),
            sender,
            state.get_object(&gas).unwrap().object_ref(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        );
        *data.expiration_mut_for_testing() =
            iota_sdk_types::TransactionExpiration::Epoch(epoch.epoch());
        to_sender_signed_transaction(data, key)
    }

    fn user_transaction_key(digest: TransactionDigest) -> SequencedConsensusTransactionKey {
        SequencedConsensusTransactionKey::External(ConsensusTransactionKey::UserTransaction(digest))
    }

    /// A new user transaction from an authority that has sent EndOfPublish
    /// is never executed. It must not lock its owned inputs at the commit,
    /// which nothing would release before the epoch ends, and it must not
    /// hide another authority's copy of the same transaction in the commit.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn user_transactions_from_an_author_that_sent_end_of_publish_take_no_locks() {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let objects: Vec<_> = (0..5)
            .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
            .collect();
        let ids: Vec<_> = objects.iter().map(|object| object.id()).collect();
        let (state, epoch, mut handler) = deterministic_validation_handler(objects).await;
        let author = *epoch.committee().authority_by_index(1).unwrap();
        let transfer =
            |object, gas| transfer_in_current_epoch(&state, &epoch, sender, &key, object, gas);
        let made_up = transfer(ids[0], ids[1]);
        let copied = transfer(ids[2], ids[3]);
        let later = transfer(ids[0], ids[4]);

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                1,
                100,
                vec![(1, ConsensusTransaction::new_end_of_publish(author))],
            ))
            .await;
        assert!(epoch.has_sent_end_of_publish(&author).unwrap());
        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                2,
                200,
                vec![
                    (
                        1,
                        ConsensusTransaction::new_user_transaction(made_up.clone()),
                    ),
                    (
                        1,
                        ConsensusTransaction::new_user_transaction(copied.clone()),
                    ),
                    (
                        0,
                        ConsensusTransaction::new_user_transaction(copied.clone()),
                    ),
                ],
            ))
            .await;
        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                3,
                300,
                vec![(0, ConsensusTransaction::new_user_transaction(later.clone()))],
            ))
            .await;

        assert_eq!(
            epoch.post_consensus_verdict(3, *later.digest()).unwrap(),
            Some(PostConsensusVerdict::Kept),
            "a later transaction on the same object must not conflict"
        );
        assert!(
            epoch
                .is_consensus_message_processed(&user_transaction_key(*copied.digest()))
                .unwrap(),
            "the other authority's copy must be scheduled"
        );
        assert_eq!(
            epoch.post_consensus_verdict(2, *made_up.digest()).unwrap(),
            None,
            "the made-up transaction must not reach validation"
        );
    }

    /// Once the epoch stops accepting consensus transactions, a new user
    /// transaction is never executed, so it must not reach validation and
    /// lock its owned inputs against a deferred transaction.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn user_transactions_after_end_of_publish_quorum_take_no_locks() {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let objects: Vec<_> = (0..2)
            .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
            .collect();
        let ids: Vec<_> = objects.iter().map(|object| object.id()).collect();
        let (state, epoch, mut handler) = deterministic_validation_handler(objects).await;
        let transaction = transfer_in_current_epoch(&state, &epoch, sender, &key, ids[0], ids[1]);
        // The state an EndOfPublish quorum leaves while deferred transactions
        // keep the epoch from rejecting all transactions.
        epoch
            .get_reconfig_state_write_lock_guard()
            .close_all_certs();

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                1,
                100,
                vec![(
                    0,
                    ConsensusTransaction::new_user_transaction(transaction.clone()),
                )],
            ))
            .await;

        assert_eq!(
            epoch
                .post_consensus_verdict(1, *transaction.digest())
                .unwrap(),
            None
        );
    }

    /// Why a user transaction is ignored at the end of the epoch.
    #[derive(Clone, Copy, Debug)]
    enum IgnoredAtEpochEnd {
        /// This validator sent EndOfPublish before its own copy was sequenced.
        OwnCopyAfterOwnEndOfPublish,
        /// Another validator sent EndOfPublish before its copy was sequenced.
        OtherCopyAfterOtherEndOfPublish,
        /// The epoch no longer accepts consensus transactions.
        CertsClosed,
    }

    /// A user transaction ignored at the end of the epoch is not recorded as
    /// processed, so its consensus adapter would otherwise hold its submit
    /// slot until the epoch ends. The adapter stops waiting when its own
    /// copy is ignored after its EndOfPublish, or any copy is ignored once
    /// the epoch accepts no consensus transactions; another validator's copy
    /// ignored after that validator's EndOfPublish leaves it waiting. A client
    /// waiting for the transaction's status is never told.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn adapter_stops_waiting_for_a_transaction_ignored_at_epoch_end() {
        for deterministic_validation in [false, true] {
            for case in [
                IgnoredAtEpochEnd::OwnCopyAfterOwnEndOfPublish,
                IgnoredAtEpochEnd::OtherCopyAfterOtherEndOfPublish,
                IgnoredAtEpochEnd::CertsClosed,
            ] {
                assert_adapter_wait_for_transaction_ignored_at_epoch_end(
                    deterministic_validation,
                    case,
                )
                .await;
            }
        }
    }

    async fn assert_adapter_wait_for_transaction_ignored_at_epoch_end(
        deterministic_validation: bool,
        case: IgnoredAtEpochEnd,
    ) {
        use crate::{
            consensus_adapter::{
                ConnectionMonitorStatusForTests, ConsensusAdapter, ConsensusAdapterMetrics,
            },
            mock_consensus::{ConsensusMode, MockConsensusClient},
        };

        let _guard = ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
            if deterministic_validation {
                config.enable_pcool_deterministic_validation_for_testing();
            } else {
                config.set_enable_pcool_flow_for_testing(true);
            }
            config
        });
        let (sender, key): (_, AccountPrivateKey) = get_key_pair();
        let objects: Vec<_> = (0..2)
            .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
            .collect();
        let ids: Vec<_> = objects.iter().map(|object| object.id()).collect();
        let (state, epoch, mut handler) = deterministic_validation_handler(objects).await;
        let transaction = transfer_in_current_epoch(&state, &epoch, sender, &key, ids[0], ids[1]);
        let digest = *transaction.digest();
        let own_index = u8::try_from(epoch.committee().authority_index(&state.name).unwrap())
            .expect("a four-member committee index fits in a byte");
        let other_index = (own_index + 1) % 4;
        let other = *epoch
            .committee()
            .authority_by_index(u32::from(other_index))
            .unwrap();

        // A consensus client that sequences nothing, so only the handler can
        // end the adapter's wait.
        let adapter = Arc::new(ConsensusAdapter::new(
            Arc::new(MockConsensusClient::new(
                Arc::downgrade(&state),
                ConsensusMode::Noop,
            )),
            state.checkpoint_store.clone(),
            state.name,
            Arc::new(ConnectionMonitorStatusForTests {}),
            100_000,
            100_000,
            None,
            None,
            ConsensusAdapterMetrics::new_test(),
            50,
        ));
        let adapter_wait = adapter
            .submit(
                ConsensusTransaction::new_user_transaction(transaction.clone()),
                None,
                &epoch,
            )
            .unwrap();
        // Lets the adapter's task register its wait before the commits.
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(!adapter_wait.is_finished());
        // Polled once so it is registered before the commits are processed.
        let waiting_submitter = epoch.notify_read_dropped_digests(digest);
        pin_mut!(waiting_submitter);
        assert!(futures::poll!(&mut waiting_submitter).is_pending());

        let copy_author = match case {
            IgnoredAtEpochEnd::OwnCopyAfterOwnEndOfPublish => {
                handler
                    .handle_consensus_output_for_test(commit_with_transactions(
                        1,
                        100,
                        vec![(
                            own_index,
                            ConsensusTransaction::new_end_of_publish(state.name),
                        )],
                    ))
                    .await;
                own_index
            }
            IgnoredAtEpochEnd::OtherCopyAfterOtherEndOfPublish => {
                handler
                    .handle_consensus_output_for_test(commit_with_transactions(
                        1,
                        100,
                        vec![(other_index, ConsensusTransaction::new_end_of_publish(other))],
                    ))
                    .await;
                other_index
            }
            IgnoredAtEpochEnd::CertsClosed => {
                epoch
                    .get_reconfig_state_write_lock_guard()
                    .close_all_certs();
                other_index
            }
        };
        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                2,
                200,
                vec![(
                    copy_author,
                    ConsensusTransaction::new_user_transaction(transaction.clone()),
                )],
            ))
            .await;

        assert!(
            !epoch
                .is_consensus_message_processed(&user_transaction_key(digest))
                .unwrap(),
            "{case:?}, deterministic validation {deterministic_validation}: the ignored \
             transaction must not be recorded as processed"
        );
        let released = tokio::time::timeout(Duration::from_secs(5), adapter_wait)
            .await
            .is_ok();
        let expect_released = !matches!(case, IgnoredAtEpochEnd::OtherCopyAfterOtherEndOfPublish);
        assert_eq!(
            released, expect_released,
            "{case:?}, deterministic validation {deterministic_validation}: adapter released"
        );
        if expect_released {
            assert_eq!(adapter.num_inflight_transactions(), 0);
        }
        assert!(
            tokio::time::timeout(Duration::from_secs(5), &mut waiting_submitter)
                .await
                .is_err(),
            "{case:?}, deterministic validation {deterministic_validation}: a client waiting \
             for the status must not be told"
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_consensus_handler_user_transaction_v1() {
        // GIVEN
        // Enable the P-COOL flow so UserTransactionV1 transactions are accepted
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config
        });

        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let num_txns: usize = 3;

        // Create owned objects and gas objects for the UserTransactionV1 transactions
        let owned_objects: Vec<Object> = (0..num_txns)
            .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
            .collect();
        let gas_objects: Vec<Object> = (0..num_txns)
            .map(|_| Object::with_id_owner_for_testing(ObjectId::random(), sender))
            .collect();

        let mut objects = owned_objects.clone();
        objects.extend(gas_objects.clone());

        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .with_objects(objects.clone())
                .build();

        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;

        let epoch_store = state.epoch_store_for_testing().clone();
        let new_epoch_start_state = epoch_store.epoch_start_state();
        let consensus_committee = get_consensus_committee(new_epoch_start_state);
        let rgp = epoch_store.reference_gas_price();

        let metrics = Arc::new(AuthorityMetrics::new(&Registry::new()));
        let backpressure_manager = BackpressureManager::new_for_tests();

        let mut consensus_handler = ConsensusHandler::new(
            epoch_store.clone(),
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            consensus_committee.clone(),
            metrics,
            backpressure_manager.subscribe(),
        );

        // AND build one block per UserTransactionV1 transaction
        let recipient = Address::random();
        let mut headers = Vec::new();
        let mut subdag_transactions = Vec::new();

        for (i, (owned_obj, gas_obj)) in owned_objects.iter().zip(gas_objects.iter()).enumerate() {
            let owned_ref = state.get_object(&owned_obj.id()).unwrap().object_ref();
            let gas_ref = state.get_object(&gas_obj.id()).unwrap().object_ref();

            let tx = Transaction::new_transfer(
                recipient,
                owned_ref,
                sender,
                gas_ref,
                rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
                rgp,
            );
            let tx = to_sender_signed_transaction(tx, &sender_key);
            let verified_tx = epoch_store.verify_transaction(tx).unwrap();

            let consensus_tx = ConsensusTransaction {
                kind: ConsensusTransactionKind::UserTransactionV1(Box::new(verified_tx.into())),
                tracking_id: Default::default(),
            };

            let transaction_bytes = bcs::to_bytes(&consensus_tx).unwrap();
            let header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(100 + i as u32, (i % consensus_committee.size()) as u8)
                    .build(),
            );
            let tx_batch = CommitmentVerifiedTransactions::new_for_test(
                &header,
                vec![starfish_core::Transaction::new(transaction_bytes)],
            );
            headers.push(header);
            subdag_transactions.push(tx_batch);
        }

        // AND create the consensus output
        let leader_header = headers[0].clone();
        let committed_header_refs: Vec<_> = headers.iter().map(|h| h.reference()).collect();
        let committed_sub_dag = CommittedSubDag::new(
            leader_header.reference(),
            headers.clone(),
            committed_header_refs,
            subdag_transactions,
            leader_header.timestamp_ms(),
            CommitRef::new(10, CommitDigest::MIN),
            vec![],
            vec![],
        );

        // WHEN processing the consensus output
        consensus_handler
            .handle_consensus_output_for_test(committed_sub_dag.clone())
            .await;

        // THEN the stats reflect the UserTransactionV1 transactions
        let num_blocks = headers.len();
        let last_consensus_stats = consensus_handler.last_consensus_stats.clone();
        assert_eq!(
            last_consensus_stats.index.transaction_index,
            num_txns as u64
        );
        assert_eq!(last_consensus_stats.index.sub_dag_index, 10_u64);
        assert_eq!(last_consensus_stats.index.last_committed_round, 100_u64);
        assert_eq!(
            last_consensus_stats.stats.get_num_messages(0),
            num_blocks as u64
        );
        assert_eq!(
            last_consensus_stats.stats.get_num_user_transactions(0),
            num_txns as u64
        );

        // AND processing the same output multiple times does not update the stats
        for _ in 0..2 {
            consensus_handler
                .handle_consensus_output_for_test(committed_sub_dag.clone())
                .await;
            let last_consensus_stats_2 = consensus_handler.last_consensus_stats.clone();
            assert_eq!(last_consensus_stats, last_consensus_stats_2);
        }
    }

    /// Block verification checks only signatures, so a committed
    /// `UserTransactionV1` can carry a Move authenticator that names one of
    /// the transaction's shared objects with another initial shared version.
    /// The handler reads the transaction's shared inputs before validating it,
    /// and must still process the commit, with post-consensus validation
    /// dropping the transaction. Callers set the protocol config; with
    /// `deterministic_validation`, the drop may come from the authenticator,
    /// which another copy of the transaction need not share, so it is saved
    /// as such and not recorded as the digest's status.
    async fn assert_user_transaction_with_inconsistent_shared_input_is_dropped(
        deterministic_validation: bool,
    ) {
        // GIVEN
        let (state, mut consensus_handler, _) = handler_with_impersonated_authority().await;
        let epoch_store = state.epoch_store_for_testing().clone();

        // AND a transaction whose authenticator disagrees with it on a shared
        // object
        let (transaction, shared) = make_move_authenticator_tx_with_inconsistent_shared_input(
            Address::random(),
            epoch_store.epoch(),
        );
        let digest = *transaction.digest();
        // The first commit, so deterministic validation has no earlier commit
        // to wait for.
        let commit = commit_with_transactions(
            1,
            100,
            vec![(0, ConsensusTransaction::new_user_transaction(transaction))],
        );

        // WHEN processing the commit
        consensus_handler
            .handle_consensus_output_for_test(commit)
            .await;

        // THEN post-consensus validation drops the transaction
        let expected = IotaError::UserInput {
            error: UserInputError::InconsistentInput {
                object_id: shared.object_id,
            },
        };
        if deterministic_validation {
            assert_eq!(
                epoch_store.post_consensus_verdict(1, digest).unwrap(),
                Some(PostConsensusVerdict::DroppedCopy(expected))
            );
            return;
        }
        let error = tokio::time::timeout(
            Duration::from_secs(60),
            epoch_store.notify_read_dropped_digests(digest),
        )
        .await
        .expect("the transaction must be dropped by post-consensus validation");
        assert_eq!(error, expected);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_consensus_handler_drops_inconsistent_shared_input() {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config
        });
        assert_user_transaction_with_inconsistent_shared_input_is_dropped(false).await;
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_consensus_handler_drops_inconsistent_shared_input_with_validation() {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.enable_pcool_deterministic_validation_for_testing();
            config
        });
        assert_user_transaction_with_inconsistent_shared_input_is_dropped(true).await;
    }

    /// Builds a commit in which every entry is a separate block authored by the
    /// given committee index and carrying exactly one consensus transaction.
    /// Blocks keep the given order, so callers control which entry the
    /// handler sees first.
    fn commit_with_transactions(
        commit_index: u32,
        round: u32,
        entries: Vec<(u8, ConsensusTransaction)>,
    ) -> CommittedSubDag {
        let mut headers = Vec::new();
        let mut subdag_transactions = Vec::new();
        for (author, transaction) in entries {
            let header =
                VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build());
            let tx_batch = CommitmentVerifiedTransactions::new_for_test(
                &header,
                vec![starfish_core::Transaction::new(
                    bcs::to_bytes(&transaction).unwrap(),
                )],
            );
            headers.push(header);
            subdag_transactions.push(tx_batch);
        }
        let leader_header = headers[0].clone();
        let committed_header_refs: Vec<_> = headers.iter().map(|h| h.reference()).collect();
        CommittedSubDag::new(
            leader_header.reference(),
            headers,
            committed_header_refs,
            subdag_transactions,
            leader_header.timestamp_ms(),
            CommitRef::new(commit_index, CommitDigest::MIN),
            vec![],
            vec![],
        )
    }

    /// A handler over a fresh authority plus the name of committee member 1,
    /// used as the authority whose messages another block author impersonates.
    async fn handler_with_impersonated_authority() -> (
        Arc<AuthorityState>,
        ConsensusHandler<CheckpointServiceNoop>,
        AuthorityName,
    ) {
        let (state, handler, impersonated, _) =
            handler_with_impersonated_authority_and_objects(vec![]).await;
        (state, handler, impersonated)
    }

    /// Same as [`handler_with_impersonated_authority`], with the given objects
    /// in genesis. Also returns the network config, which holds the committee's
    /// keys.
    async fn handler_with_impersonated_authority_and_objects(
        objects: Vec<Object>,
    ) -> (
        Arc<AuthorityState>,
        ConsensusHandler<CheckpointServiceNoop>,
        AuthorityName,
        NetworkConfig,
    ) {
        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .committee_size(NonZeroUsize::new(4).unwrap())
                .with_objects(objects)
                .build();
        let state = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;
        let epoch_store = state.epoch_store_for_testing().clone();
        let consensus_committee = get_consensus_committee(epoch_store.epoch_start_state());
        let impersonated = *epoch_store.committee().authority_by_index(1).unwrap();
        let handler = ConsensusHandler::new(
            epoch_store,
            state.clone(),
            Arc::new(CheckpointServiceNoop {}),
            state.execution_scheduler().clone(),
            state.get_object_cache_reader().clone(),
            Arc::new(ArcSwap::default()),
            consensus_committee,
            Arc::new(AuthorityMetrics::new(&Registry::new())),
            BackpressureManager::new_for_tests().subscribe(),
        );
        (state, handler, impersonated, network_config)
    }

    /// An `EndOfPublish` naming authority B but committed in a block authored
    /// by A is rejected. Its key must not shadow B's own `EndOfPublish` in a
    /// later commit, otherwise a handler that ran continuously and one that
    /// restarted in between would count different stake.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_rejected_end_of_publish_does_not_shadow_later_valid_one() {
        telemetry_subscribers::init_for_testing();
        let (state, mut handler, b) = handler_with_impersonated_authority().await;
        let epoch_store = state.epoch_store_for_testing();

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                10,
                100,
                vec![(0, ConsensusTransaction::new_end_of_publish(b))],
            ))
            .await;
        assert!(!epoch_store.has_sent_end_of_publish(&b).unwrap());
        assert!(
            !epoch_store
                .is_consensus_message_processed(&SequencedConsensusTransactionKey::External(
                    ConsensusTransactionKey::EndOfPublish(b),
                ))
                .unwrap()
        );

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                11,
                101,
                vec![(1, ConsensusTransaction::new_end_of_publish(b))],
            ))
            .await;
        assert!(epoch_store.has_sent_end_of_publish(&b).unwrap());
    }

    /// Same as above, but the forged and the authentic `EndOfPublish` land in
    /// one commit, with the forged one ordered first.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_rejected_end_of_publish_does_not_shadow_valid_one_in_same_commit() {
        telemetry_subscribers::init_for_testing();
        let (state, mut handler, b) = handler_with_impersonated_authority().await;
        let epoch_store = state.epoch_store_for_testing();

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                10,
                100,
                vec![
                    (0, ConsensusTransaction::new_end_of_publish(b)),
                    (1, ConsensusTransaction::new_end_of_publish(b)),
                ],
            ))
            .await;
        assert!(epoch_store.has_sent_end_of_publish(&b).unwrap());
    }

    /// The DKG message key is one per authority per epoch and has no nonce, so
    /// a forged key is the cheapest way to suppress an authority's only DKG
    /// message. The payload bytes are irrelevant here: the handler records the
    /// key as processed whenever the message reaches the randomness manager.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_rejected_dkg_message_does_not_shadow_later_valid_one() {
        telemetry_subscribers::init_for_testing();
        let (state, mut handler, b) = handler_with_impersonated_authority().await;
        let epoch_store = state.epoch_store_for_testing();
        let dkg_message = |author: u8| {
            (
                author,
                ConsensusTransaction {
                    tracking_id: [0; 8],
                    kind: ConsensusTransactionKind::RandomnessDkgMessage(b, vec![0xff]),
                },
            )
        };
        let key = SequencedConsensusTransactionKey::External(
            ConsensusTransactionKey::RandomnessDkgMessage(b),
        );

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                10,
                100,
                vec![dkg_message(0)],
            ))
            .await;
        assert!(!epoch_store.is_consensus_message_processed(&key).unwrap());

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                11,
                101,
                vec![dkg_message(1)],
            ))
            .await;
        assert!(epoch_store.is_consensus_message_processed(&key).unwrap());
    }

    /// Commits `transaction` from a block of committee member 1 after member 1
    /// sent `EndOfPublish`, so it is dropped without being recorded as
    /// processed, then commits member 1's copy again followed by member 2's
    /// copy in one commit. Member 2's copy must be processed: neither the
    /// dropped copy in the earlier commit nor the one ahead of it in the same
    /// commit may hide it, and a handler that ran continuously and one that
    /// restarted in between must reach the same outcome.
    async fn assert_copy_of_transaction_dropped_after_end_of_publish_is_processed(
        state: &AuthorityState,
        handler: &mut ConsensusHandler<CheckpointServiceNoop>,
        eop_author: AuthorityName,
        transaction: ConsensusTransaction,
    ) {
        let epoch_store = state.epoch_store_for_testing();
        let key = SequencedConsensusTransactionKey::External(transaction.key());

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                10,
                100,
                vec![(1, ConsensusTransaction::new_end_of_publish(eop_author))],
            ))
            .await;
        assert!(epoch_store.has_sent_end_of_publish(&eop_author).unwrap());

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                11,
                101,
                vec![(1, transaction.clone())],
            ))
            .await;
        assert!(!epoch_store.is_consensus_message_processed(&key).unwrap());

        handler
            .handle_consensus_output_for_test(commit_with_transactions(
                12,
                102,
                vec![(1, transaction.clone()), (2, transaction)],
            ))
            .await;
        assert!(epoch_store.is_consensus_message_processed(&key).unwrap());
    }

    /// The [`handler_with_impersonated_authority_and_objects`] fixture plus a
    /// transfer of an owned object that the fixture's genesis holds.
    async fn handler_with_transfer() -> (
        Arc<AuthorityState>,
        ConsensusHandler<CheckpointServiceNoop>,
        AuthorityName,
        NetworkConfig,
        VerifiedTransaction,
    ) {
        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let owned_object = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let (state, handler, impersonated, network_config) =
            handler_with_impersonated_authority_and_objects(vec![
                owned_object.clone(),
                gas_object.clone(),
            ])
            .await;
        let epoch_store = state.epoch_store_for_testing();
        let rgp = epoch_store.reference_gas_price();
        let transfer = Transaction::new_transfer(
            Address::random(),
            state.get_object(&owned_object.id()).unwrap().object_ref(),
            sender,
            state.get_object(&gas_object.id()).unwrap().object_ref(),
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        );
        let transfer = epoch_store
            .verify_transaction(to_sender_signed_transaction(transfer, &sender_key))
            .unwrap();
        (state, handler, impersonated, network_config, transfer)
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_user_transaction_dropped_after_end_of_publish_does_not_shadow_later_copy() {
        telemetry_subscribers::init_for_testing();
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config.set_consensus_dedup_only_recorded_messages_for_testing(true);
            config
        });
        let (state, mut handler, eop_author, _, transfer) = handler_with_transfer().await;

        assert_copy_of_transaction_dropped_after_end_of_publish_is_processed(
            &state,
            &mut handler,
            eop_author,
            ConsensusTransaction::new_user_transaction(transfer.into()),
        )
        .await;
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_certificate_dropped_after_end_of_publish_does_not_shadow_later_copy() {
        telemetry_subscribers::init_for_testing();
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(false);
            config.set_consensus_dedup_only_recorded_messages_for_testing(true);
            config
        });
        let (state, mut handler, eop_author, network_config, transfer) =
            handler_with_transfer().await;
        let keypairs: Vec<_> = network_config
            .validator_configs()
            .iter()
            .map(|config| config.authority_key_pair().copy())
            .collect();
        let certificate = CertifiedTransaction::new_from_keypairs_for_testing(
            transfer.into_message(),
            &keypairs,
            state.epoch_store_for_testing().committee(),
        );

        assert_copy_of_transaction_dropped_after_end_of_publish_is_processed(
            &state,
            &mut handler,
            eop_author,
            ConsensusTransaction::new_certificate_message(&state.name, certificate),
        )
        .await;
    }

    /// A message recorded as processed enters the processed-transaction cache,
    /// so a later copy is skipped there. With the cache empty, as after a
    /// restart or an eviction, the copy is skipped by the processed-message
    /// table instead, with the same outcome.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_recorded_message_is_skipped_by_cache_or_by_table() {
        telemetry_subscribers::init_for_testing();
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_consensus_dedup_only_recorded_messages_for_testing(true);
            config
        });
        let (state, mut handler, b) = handler_with_impersonated_authority().await;
        let end_of_publish = |index| {
            commit_with_transactions(
                index,
                90 + index,
                vec![(1, ConsensusTransaction::new_end_of_publish(b))],
            )
        };
        let cache_hits = handler.metrics.skipped_consensus_txns_cache_hit.clone();
        let table_skips = handler.metrics.skipped_consensus_txns.clone();

        handler
            .handle_consensus_output_for_test(end_of_publish(10))
            .await;
        assert!(
            state
                .epoch_store_for_testing()
                .has_sent_end_of_publish(&b)
                .unwrap()
        );
        // The commit prologue key recorded by the same commit is not cached.
        assert_eq!(handler.processed_cache.len(), 1);

        handler
            .handle_consensus_output_for_test(end_of_publish(11))
            .await;
        assert_eq!((cache_hits.get(), table_skips.get()), (1, 0));

        handler.processed_cache.clear();
        handler
            .handle_consensus_output_for_test(end_of_publish(12))
            .await;
        assert_eq!((cache_hits.get(), table_skips.get()), (1, 1));
    }

    /// Post-consensus load shedding must be deterministic across honest
    /// validators: for the same sequence of consensus commits, every validator
    /// must schedule (and eventually checkpoint) the same set of user
    /// transactions.
    ///
    /// A transaction dropped by load shedding is not recorded in the durable
    /// `consensus_message_processed` set, so its retry can be accepted later.
    /// Without `consensus_dedup_only_recorded_messages`, its key still entered
    /// the consensus handler's in-memory `processed_cache` LRU: when the
    /// transaction was retried after shedding cleared, a validator whose
    /// handler kept running skipped it via the LRU, while a validator that
    /// restarted in between (fresh LRU, durable set unset) processed and
    /// scheduled it — a checkpoint fork.
    ///
    /// Scenario, driven by identical commits on both validators:
    /// - Commit A: every committee member broadcasts a 100% overload notification, plus user
    ///   transaction T. The quorum percentage is 100, so T is dropped on both validators.
    /// - Commit B: every committee member broadcasts 0%, plus T again (the retry that
    ///   `ValidatorOverloadedRetryAfter` asks the client for). Validator A processes commit B with
    ///   the same handler; validator B's handler is recreated first, modeling a restart after
    ///   commit A was flushed (the epoch store's consensus-derived state is the same either way —
    ///   only the handler's LRU is lost).
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn test_load_shedding_dropped_transaction_retry_is_processed_after_restart() {
        let _guard = ProtocolConfig::apply_overrides_for_testing(|_, mut config| {
            config.set_enable_pcool_flow_for_testing(true);
            config.set_consensus_dedup_only_recorded_messages_for_testing(true);
            config
        });

        let (sender, sender_key): (_, AccountPrivateKey) = get_key_pair();
        let owned_object = Object::with_id_owner_for_testing(ObjectId::random(), sender);
        let gas_object = Object::with_id_owner_for_testing(ObjectId::random(), sender);

        let network_config =
            iota_swarm_config::network_config_builder::ConfigBuilder::new_with_temp_dir()
                .committee_size(NonZeroUsize::new(4).unwrap())
                .with_objects(vec![owned_object.clone(), gas_object.clone()])
                .build();

        // Two honest validators of the same committee, fed identical commits.
        let state_a = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 0)
            .build()
            .await;
        let state_b = TestAuthorityBuilder::new()
            .with_network_config(&network_config, 1)
            .build()
            .await;

        let epoch_store_a = state_a.epoch_store_for_testing().clone();
        let epoch_store_b = state_b.epoch_store_for_testing().clone();
        let committee = epoch_store_a.committee().clone();
        let consensus_committee = get_consensus_committee(epoch_store_a.epoch_start_state());

        let new_handler = |state: &Arc<AuthorityState>, metrics: &Arc<AuthorityMetrics>| {
            ConsensusHandler::new(
                state.epoch_store_for_testing().clone(),
                state.clone(),
                Arc::new(CheckpointServiceNoop {}),
                state.execution_scheduler().clone(),
                state.get_object_cache_reader().clone(),
                Arc::new(ArcSwap::default()),
                consensus_committee.clone(),
                metrics.clone(),
                BackpressureManager::new_for_tests().subscribe(),
            )
        };

        let metrics_a = Arc::new(AuthorityMetrics::new(&Registry::new()));
        let metrics_b = Arc::new(AuthorityMetrics::new(&Registry::new()));
        let mut handler_a = new_handler(&state_a, &metrics_a);
        let mut handler_b = new_handler(&state_b, &metrics_b);
        // Stand-in for validator B's post-restart handler. `ConsensusHandler::new`
        // asserts it only runs at startup (empty quarantine), so it cannot be
        // created after commit A; creating it here gives the same state the
        // restart produces on the path under test — an empty `processed_cache`
        // over the same epoch store.
        let mut handler_b_restarted = new_handler(&state_b, &metrics_b);

        // User transaction T, identical bytes in both commits (a client retry
        // resubmits the same signed transaction, so the same digest).
        let rgp = epoch_store_a.reference_gas_price();
        let owned_ref = state_a.get_object(&owned_object.id()).unwrap().object_ref();
        let gas_ref = state_a.get_object(&gas_object.id()).unwrap().object_ref();
        let (recipient, _): (Address, AccountPrivateKey) = get_key_pair();
        let tx_data = Transaction::new_transfer(
            recipient,
            owned_ref,
            sender,
            gas_ref,
            rgp * TEST_ONLY_GAS_UNIT_FOR_TRANSFER,
            rgp,
        );
        let tx = to_sender_signed_transaction(tx_data, &sender_key);
        let verified_tx = epoch_store_a.verify_transaction(tx).unwrap();
        let tx_digest = *verified_tx.digest();
        let user_tx_bytes = bcs::to_bytes(&ConsensusTransaction {
            kind: ConsensusTransactionKind::UserTransactionV1(Box::new(verified_tx.into())),
            tracking_id: Default::default(),
        })
        .unwrap();
        let tx_key = SequencedConsensusTransactionKey::External(
            ConsensusTransactionKey::UserTransaction(tx_digest),
        );

        // A block authored by committee member `author` carrying its overload
        // notification. The generation only has to be unique per (authority,
        // commit); wall-clock-based generations could collide across the two
        // commits built back-to-back here.
        let notification_block = |author: u32, round: u32, generation: u64, percentage: u8| {
            let authority = *committee.authority_by_index(author).unwrap();
            let bytes = bcs::to_bytes(&ConsensusTransaction {
                kind: ConsensusTransactionKind::OverloadNotificationV1(
                    authority, generation, percentage,
                ),
                tracking_id: Default::default(),
            })
            .unwrap();
            let header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(round, author as u8).build(),
            );
            let batch = CommitmentVerifiedTransactions::new_for_test(
                &header,
                vec![starfish_core::Transaction::new(bytes)],
            );
            (header, batch)
        };
        let user_tx_block = |author: u32, round: u32| {
            let header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(round, author as u8).build(),
            );
            let batch = CommitmentVerifiedTransactions::new_for_test(
                &header,
                vec![starfish_core::Transaction::new(user_tx_bytes.clone())],
            );
            (header, batch)
        };
        let make_commit = |blocks: Vec<(VerifiedBlockHeader, CommitmentVerifiedTransactions)>,
                           sub_dag_index: u64| {
            let (headers, batches): (Vec<_>, Vec<_>) = blocks.into_iter().unzip();
            let leader = headers[0].clone();
            let refs: Vec<_> = headers.iter().map(|h| h.reference()).collect();
            CommittedSubDag::new(
                leader.reference(),
                headers,
                refs,
                batches,
                leader.timestamp_ms(),
                CommitRef::new(sub_dag_index as u32, CommitDigest::MIN),
                vec![],
                vec![],
            )
        };

        // Commit A: 100% notifications from all 4 members, then T. Without the
        // flag, T is the last key inserted into the handler LRU, so it survives
        // even the smallest randomized test capacity (2).
        let mut blocks_a: Vec<_> = (0..4u32)
            .map(|i| notification_block(i, 100 + i, 1, 100))
            .collect();
        blocks_a.push(user_tx_block(0, 110));
        let commit_a = make_commit(blocks_a, 10);

        // Commit B: T first (its LRU lookup happens before any other key is
        // inserted), then 0% notifications from all 4 members.
        let mut blocks_b = vec![user_tx_block(1, 200)];
        blocks_b.extend((0..4u32).map(|i| notification_block(i, 201 + i, 2, 0)));
        let commit_b = make_commit(blocks_b, 11);

        // Both validators process commit A and drop T.
        handler_a
            .handle_consensus_output_for_test(commit_a.clone())
            .await;
        handler_b.handle_consensus_output_for_test(commit_a).await;
        for (epoch_store, metrics) in [(&epoch_store_a, &metrics_a), (&epoch_store_b, &metrics_b)] {
            assert_eq!(
                epoch_store.get_quorum_load_shedding_percentage().unwrap(),
                100
            );
            assert_eq!(
                metrics
                    .consensus_handler_load_shedding_dropped_transactions
                    .get(),
                1
            );
            assert!(!epoch_store.is_consensus_message_processed(&tx_key).unwrap());
        }

        // Validator B restarts before the retry arrives: the handler (and its
        // LRU) is gone, the epoch store's consensus-derived state remains.
        drop(handler_b);

        // Both validators process commit B, which clears shedding and retries T.
        handler_a
            .handle_consensus_output_for_test(commit_b.clone())
            .await;
        handler_b_restarted
            .handle_consensus_output_for_test(commit_b)
            .await;

        // Both validators derived the same shedding state from the commits...
        assert_eq!(
            epoch_store_a.get_quorum_load_shedding_percentage().unwrap(),
            0
        );
        assert_eq!(
            epoch_store_b.get_quorum_load_shedding_percentage().unwrap(),
            0
        );

        // ...so they must agree on whether the retry of T was processed.
        let processed_a = epoch_store_a
            .is_consensus_message_processed(&tx_key)
            .unwrap();
        let processed_b = epoch_store_b
            .is_consensus_message_processed(&tx_key)
            .unwrap();
        assert_eq!(
            processed_a, processed_b,
            "honest validators diverged on a load-shed-then-retried transaction \
             (checkpoint fork): still-running validator processed={processed_a}, \
             restarted validator processed={processed_b}",
        );
        // And with shedding at 0% the retry must actually have been accepted.
        assert!(
            processed_b,
            "retry of a load-shed transaction was not processed after shedding cleared"
        );
    }

    #[test]
    fn test_order_by_gas_price() {
        let chain = Chain::Unknown;
        let mut v = vec![
            cap_txn(10, chain),
            user_txn(42),
            user_txn(100),
            cap_txn(1, chain),
        ];
        PostConsensusTxReorder::reorder(&mut v, ConsensusTransactionOrdering::ByGasPrice);
        assert_eq!(
            extract(v),
            vec![
                "cap(10)".to_string(),
                "cap(1)".to_string(),
                "user(100)".to_string(),
                "user(42)".to_string(),
            ]
        );

        let mut v = vec![
            user_txn(1200),
            cap_txn(10, chain),
            user_txn(12),
            user_txn(1000),
            user_txn(42),
            user_txn(100),
            cap_txn(1, chain),
            user_txn(1000),
        ];
        PostConsensusTxReorder::reorder(&mut v, ConsensusTransactionOrdering::ByGasPrice);
        assert_eq!(
            extract(v),
            vec![
                "cap(10)".to_string(),
                "cap(1)".to_string(),
                "user(1200)".to_string(),
                "user(1000)".to_string(),
                "user(1000)".to_string(),
                "user(100)".to_string(),
                "user(42)".to_string(),
                "user(12)".to_string(),
            ]
        );

        // If there are no user transactions, the order should be preserved.
        let mut v = vec![
            cap_txn(10, chain),
            eop_txn(12),
            eop_txn(10),
            cap_txn(1, chain),
            eop_txn(11),
        ];
        PostConsensusTxReorder::reorder(&mut v, ConsensusTransactionOrdering::ByGasPrice);
        assert_eq!(
            extract(v),
            vec![
                "cap(10)".to_string(),
                "eop(12)".to_string(),
                "eop(10)".to_string(),
                "cap(1)".to_string(),
                "eop(11)".to_string(),
            ]
        );
    }

    fn extract(v: Vec<VerifiedSequencedConsensusTransaction>) -> Vec<String> {
        v.into_iter().map(extract_one).collect()
    }

    fn extract_one(t: VerifiedSequencedConsensusTransaction) -> String {
        match t.0.transaction {
            SequencedConsensusTransactionKind::External(ext) => match ext.kind {
                ConsensusTransactionKind::EndOfPublish(authority) => {
                    format!("eop({})", authority.0[0])
                }
                ConsensusTransactionKind::CapabilityNotificationV1(cap) => {
                    format!("cap({})", cap.generation)
                }
                ConsensusTransactionKind::CertifiedTransaction(txn) => {
                    format!("user({})", txn.transaction().gas_price())
                }
                _ => unreachable!(),
            },
            SequencedConsensusTransactionKind::System(_) => unreachable!(),
        }
    }

    fn eop_txn(a: u8) -> VerifiedSequencedConsensusTransaction {
        let mut authority = AuthorityName::default();
        authority.0[0] = a;
        txn(ConsensusTransactionKind::EndOfPublish(authority))
    }

    fn cap_txn(generation: u64, chain: Chain) -> VerifiedSequencedConsensusTransaction {
        txn(ConsensusTransactionKind::CapabilityNotificationV1(
            // we don't use the "new" constructor because we need to set the generation
            AuthorityCapabilitiesV1 {
                authority: Default::default(),
                generation,
                supported_protocol_versions:
                    SupportedProtocolVersionsWithHashes::from_supported_versions(
                        SupportedProtocolVersions::SYSTEM_DEFAULT,
                        chain,
                    ),
                available_system_packages: vec![],
            },
        ))
    }

    fn user_txn(gas_price: u64) -> VerifiedSequencedConsensusTransaction {
        let (committee, keypairs) = Committee::new_simple_test_committee();
        let tx = SenderSignedTransaction::new(
            Transaction::new_transfer(
                Address::ZERO,
                random_object_ref(),
                Address::ZERO,
                random_object_ref(),
                1000 * gas_price,
                gas_price,
            ),
            vec![],
        );
        txn(ConsensusTransactionKind::CertifiedTransaction(Box::new(
            CertifiedTransaction::new_from_keypairs_for_testing(tx, &keypairs, &committee),
        )))
    }

    fn txn(kind: ConsensusTransactionKind) -> VerifiedSequencedConsensusTransaction {
        VerifiedSequencedConsensusTransaction::new_test(ConsensusTransaction {
            kind,
            tracking_id: Default::default(),
        })
    }
}
