// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use futures::{StreamExt as _, stream::FuturesUnordered};
use iota_macros::fail_point_async;
use iota_metrics::{
    monitored_future,
    monitored_mpsc::{Receiver, Sender, channel},
    monitored_scope,
};
use parking_lot::{Mutex, RwLock};
#[cfg(not(test))]
use rand::prelude::SliceRandom as _;
use rand::{SeedableRng, rng, rngs::StdRng};
use starfish_config::AuthorityIndex;
use tokio::{
    sync::mpsc::error::TrySendError,
    task::{JoinError, JoinSet},
    time::{Instant, sleep_until, timeout},
};
use tracing::{debug, info, warn};

use crate::{
    block_header::CommitmentVerifiedTransactions,
    block_verifier::BlockVerifier,
    commit_syncer::verify_transactions_commitments,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::{DagState, DataSource},
    error::{ConsensusError, ConsensusResult},
    misbehavior_store::MisbehaviorStore,
    network::{NetworkClient, SerializedTransactionsV2},
    task::spawn_blocking,
    transaction_ref::TransactionRef,
};

/// The number of concurrent live transaction fetch requests
/// Set to the maximum number of rounds per second as it can be called by newly
/// produced commits only
const LIVE_FETCH_TRANSACTIONS_CONCURRENCY: usize = 20;

/// The number of concurrent periodic transaction fetch requests
const PERIODIC_FETCH_TRANSACTIONS_CONCURRENCY: usize = 4;

/// The maximum number of concurrent request per authority for fetching
/// transactions. It is used separately for live and periodic fetches
const MAX_CONCURRENT_REQUESTS_PER_AUTHORITY: usize = 2;

/// The maximum number of assigned peers per one call of transaction fetch
/// It allows to globally limit the number of spawned tasks by
/// (LIVE_FETCH_TRANSACTIONS_CONCURRENCY +
/// PERIODIC_FETCH_TRANSACTIONS_CONCURRENCY) *
/// MAX_ASSIGNED_AUTHORITIES_PER_TRANSACTION_FETCH
const MAX_ASSIGNED_AUTHORITIES_PER_TRANSACTION_FETCH: usize = 4;

/// Timeout for the transactions synchronizer to run periodically and fetch
/// missing transactions.
const TRANSACTIONS_SYNCHRONIZER_TIMEOUT: Duration = Duration::from_millis(500);

/// Timeout that is given to fetch transactions from a given peer.
pub(crate) const FETCH_REQUEST_TIMEOUT: Duration = Duration::from_millis(2000);

/// Maximum number of authorities that can concurrently fetch transactions for a
/// given block ref.
const MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION: usize = 2;

#[derive(Debug, Clone, Copy, Ord, Eq, PartialOrd, PartialEq)]
enum SyncMethod {
    Live,
    Periodic,
}
impl SyncMethod {
    fn get_string(&self) -> String {
        match self {
            SyncMethod::Live => "live",
            SyncMethod::Periodic => "periodic",
        }
        .to_string()
    }
}

/// Outcome of a successful fetch from one authority.
struct FetchStats {
    latency: Duration,
    requested: usize,
    matched_requested: usize,
}

/// Tracks the number of concurrent transaction fetch requests to each peer.
/// Counts the number of fetch requests separately for periodic and live
/// transaction synchronizer as they serve different purposes.
struct InflightActiveRequests {
    inner: Mutex<BTreeMap<(AuthorityIndex, SyncMethod), usize>>,
}

impl InflightActiveRequests {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(BTreeMap::new()),
        })
    }
    fn unlock_active_request(&self, peer: AuthorityIndex, sync_method: SyncMethod) {
        let mut inner = self.inner.lock();
        if let Some(val) = inner.get_mut(&(peer, sync_method)) {
            *val = val.saturating_sub(1);
        }
    }
}

struct ActiveRequestGuard {
    peer: AuthorityIndex,
    sync_method: SyncMethod,
    active_requests: Arc<InflightActiveRequests>,
}

impl Drop for ActiveRequestGuard {
    fn drop(&mut self) {
        self.active_requests
            .unlock_active_request(self.peer, self.sync_method);
    }
}

struct TransactionsGuard {
    map: Arc<InflightTransactionsMap>,
    transactions_refs: BTreeSet<TransactionRef>,
    peer: AuthorityIndex,
}

impl Drop for TransactionsGuard {
    fn drop(&mut self) {
        self.map
            .unlock_transactions(&self.transactions_refs, self.peer);
    }
}

// Keeps a mapping between the missing transactions that have been instructed to
// be fetched and the authorities that are currently fetching them. For a
// transaction ref there is a maximum number of authorities that can
// concurrently fetch it. The authority ids that are currently fetching a
// transaction are set on the corresponding `BTreeSet` and basically they act
// as "locks".
struct InflightTransactionsMap {
    inner: Mutex<HashMap<TransactionRef, BTreeSet<AuthorityIndex>>>,
}

impl InflightTransactionsMap {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(HashMap::new()),
        })
    }

    /// Locks the transactions to be fetched for the assigned `peer`. We
    /// want to avoid re-fetching the missing transactions from too many
    /// authorities at the same time, thus we limit the concurrency per
    /// transaction by attempting to lock per transaction ref. In addition, we
    /// check whether a given `peer` has many concurrent requests. If so, we
    /// will not lock transactions. The method return optionally two guards.
    /// One for the fetched transactions and one for active fetch request.
    fn lock_transactions_and_active_request(
        self: &Arc<Self>,
        missing_transaction_refs: BTreeSet<TransactionRef>,
        peer: AuthorityIndex,
        max_number_transactions_per_fetch: usize,
        sync_method: SyncMethod,
        active_requests: Arc<InflightActiveRequests>,
    ) -> Option<(TransactionsGuard, ActiveRequestGuard)> {
        // Lock both maps
        let mut transaction_map = self.inner.lock();
        let mut active_requests_locked = active_requests.inner.lock();

        // Ensure we have a counter for this (peer, method)
        let req_entry = active_requests_locked
            .entry((peer, sync_method))
            .or_insert(0);

        // Enforce per-peer concurrent fetch cap
        if *req_entry >= MAX_CONCURRENT_REQUESTS_PER_AUTHORITY {
            return None;
        }

        // Now try to lock transactions
        let mut selected_transactions_to_fetch = BTreeSet::new();
        let mut selected_transaction_refs_num = 0;

        for tx_ref in missing_transaction_refs {
            let authorities = transaction_map.entry(tx_ref).or_default();

            if authorities.len() < MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION
                && authorities.insert(peer)
            {
                selected_transactions_to_fetch.insert(tx_ref);
                selected_transaction_refs_num += 1;

                if selected_transaction_refs_num >= max_number_transactions_per_fetch {
                    break;
                }
            }
        }

        // If we couldn’t lock any transactions, don’t bump the request counter
        if selected_transactions_to_fetch.is_empty() {
            return None;
        }

        // We actually got some work → count an active request
        *req_entry += 1;

        // Drop locks before returning guards
        drop(transaction_map);
        drop(active_requests_locked);

        let transactions_guard = TransactionsGuard {
            map: self.clone(),
            transactions_refs: selected_transactions_to_fetch,
            peer,
        };

        let active_request_guard = ActiveRequestGuard {
            peer,
            sync_method,
            active_requests: active_requests.clone(),
        };

        Some((transactions_guard, active_request_guard))
    }

    /// Unlocks the provided transaction references for the given `peer`. The
    /// unlocking is strict, meaning that if this method is called for a
    /// specific transaction ref and peer more times than the corresponding
    /// lock has been called, it will panic.
    fn unlock_transactions(
        self: &Arc<Self>,
        tx_refs: &BTreeSet<TransactionRef>,
        peer: AuthorityIndex,
    ) {
        // Now mark all the transactions as fetched from the map
        let mut transactions_to_fetch = self.inner.lock();
        for tx_ref in tx_refs {
            let authorities = transactions_to_fetch
                .get_mut(tx_ref)
                .expect("We should expect a non empty map with at least one peer");
            assert!(authorities.remove(&peer), "Peer index should be present!");
            // If the last one then just clean up
            if authorities.is_empty() {
                transactions_to_fetch.remove(tx_ref);
            }
        }
    }
    #[cfg(test)]
    fn num_of_locked_transactions(self: &Arc<Self>) -> usize {
        let inner = self.inner.lock();
        inner.len()
    }
}

pub(crate) struct TransactionsSynchronizerHandle {
    live_fetch_requests: Sender<BTreeMap<TransactionRef, BTreeSet<AuthorityIndex>>>,
    tasks: tokio::sync::Mutex<JoinSet<()>>,
}

impl TransactionsSynchronizerHandle {
    /// Queues a live fetch of the missing transactions from the authorities
    /// that acknowledged them. Fails with `TransactionSynchronizerSaturated`
    /// when the live queue is full; the periodic scheduler picks the
    /// transactions up later.
    pub(crate) fn fetch_transactions(
        &self,
        missing_transaction_refs: BTreeMap<TransactionRef, BTreeSet<AuthorityIndex>>,
    ) -> ConsensusResult<()> {
        self.live_fetch_requests
            .try_send(missing_transaction_refs)
            .map_err(|err| match err {
                TrySendError::Full(_) => ConsensusError::TransactionSynchronizerSaturated,
                TrySendError::Closed(_) => ConsensusError::Shutdown,
            })
    }

    pub(crate) async fn stop(&self) -> Result<(), JoinError> {
        let mut tasks = self.tasks.lock().await;
        tasks.abort_all();
        while let Some(result) = tasks.join_next().await {
            match result {
                // task finished successfully
                Ok(_) => (),
                // task was cancelled, which is expected on shutdown
                Err(e) if e.is_cancelled() => (),
                // propagate other errors (e.g. panics)
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

/// `TransactionsSynchronizer` oversees live transaction synchronization,
/// crucial for node progress. Live synchronization refers to the process of
/// retrieving missing transactions, particularly those essential for advancing
/// a node when transactions from the committed blocks is absent.
/// `TransactionsSynchronizer` aims for swift catch-up employing two mechanisms:
///
/// 1. Explicitly requesting missing transactions from authorities that have acknowledged them in
///    their blocks that were committed. A locking mechanism allows concurrent requests for missing
///    transactions from a limited number of authorities simultaneously, enhancing the chances of
///    timely retrieval.
///
/// 2. Periodically requesting missing transactions via a scheduler. This primarily serves to
///    retrieve missing transactions that were not fetched via the live synchronization. The
///    scheduler runs on a fixed period, and at half that period after it started a fetch, ensuring
///    continued transaction retrieval if gaps persist.
pub(crate) struct TransactionsSynchronizer<C: NetworkClient, D: CoreThreadDispatcher> {
    context: Arc<Context>,
    core_dispatcher: Arc<D>,
    dag_state: Arc<RwLock<DagState>>,
    active_requests: Arc<InflightActiveRequests>,
    fetch_transactions_scheduler_task: JoinSet<()>,
    network_client: Arc<C>,
    inflight_transactions_map: Arc<InflightTransactionsMap>,
    /// Applies the same transaction limit and batch verification checks to
    /// fetched payloads as the direct block-bundle route.
    block_verifier: Arc<dyn BlockVerifier>,
    /// Charges faults for fetched payloads that fail verification: the author,
    /// when the payload is provably theirs, and the peer that served it.
    misbehavior_store: Arc<MisbehaviorStore>,
}

impl<C: NetworkClient, D: CoreThreadDispatcher> TransactionsSynchronizer<C, D> {
    /// Starts the transactions synchronizer, which is responsible for fetching
    /// transactions from other authorities and managing transaction
    /// synchronization tasks.
    pub fn start(
        network_client: Arc<C>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        dag_state: Arc<RwLock<DagState>>,
        block_verifier: Arc<dyn BlockVerifier>,
    ) -> Arc<TransactionsSynchronizerHandle> {
        let inflight_transactions_map = InflightTransactionsMap::new();

        // Create a channel for live fetch requests
        let (live_fetch_sender, live_fetch_receiver) = channel(
            "consensus_transactions_synchronizer_live_fetches",
            LIVE_FETCH_TRANSACTIONS_CONCURRENCY,
        );

        let mut tasks = JoinSet::new();
        let active_requests = InflightActiveRequests::new();
        let misbehavior_store = dag_state.read().misbehavior_store().clone();
        // Spawn the live fetcher task
        let live_fetcher_async = Self::live_fetcher(
            active_requests.clone(),
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            live_fetch_receiver,
            inflight_transactions_map.clone(),
            block_verifier.clone(),
            misbehavior_store.clone(),
        );
        tasks.spawn(monitored_future!(live_fetcher_async));

        // Spawn the task for the periodic runs
        tasks.spawn(monitored_future!(async move {
            let mut s = Self {
                context,
                core_dispatcher,
                fetch_transactions_scheduler_task: JoinSet::new(),
                active_requests,
                network_client,
                inflight_transactions_map,
                dag_state,
                block_verifier,
                misbehavior_store,
            };
            s.run().await;
        }));

        Arc::new(TransactionsSynchronizerHandle {
            live_fetch_requests: live_fetch_sender,
            tasks: tokio::sync::Mutex::new(tasks),
        })
    }

    /// Runs the periodic scheduler: on every tick, while fewer than
    /// `PERIODIC_FETCH_TRANSACTIONS_CONCURRENCY` periodic fetches are
    /// running, asks the core for the missing transactions and fetches them.
    #[cfg_attr(test, tracing::instrument(skip_all, name = "", fields(authority = %self.context.own_index
    )))]
    async fn run(&mut self) {
        // We want the transactions synchronizer to run periodically to
        // fetch any missing transactions.
        let scheduler_timeout = sleep_until(Instant::now() + TRANSACTIONS_SYNCHRONIZER_TIMEOUT);

        tokio::pin!(scheduler_timeout);

        loop {
            tokio::select! {
                Some(result) = self.fetch_transactions_scheduler_task.join_next(), if !self.fetch_transactions_scheduler_task.is_empty() => {
                    match result {
                        Ok(()) => {},
                        Err(e) => {
                            if e.is_cancelled() {
                            } else if e.is_panic() {
                                std::panic::resume_unwind(e.into_panic());
                            } else {
                                panic!("fetch transactions scheduler task failed: {e}");
                            }
                        },
                    };
                },
                () = &mut scheduler_timeout => {
                    // we want to start a new task only if the number of tasks is not too large.
                    let started = if self.fetch_transactions_scheduler_task.len() < PERIODIC_FETCH_TRANSACTIONS_CONCURRENCY {
                        match self.start_fetch_missing_transactions_task().await {
                            Ok(started) => started,
                            Err(err) => {
                                debug!("Core is shutting down, transactions synchronizer is shutting down: {err:?}");
                                return;
                            }
                        }
                    } else {
                        false
                    };

                    // Retry sooner while there is something to fetch.
                    let next_tick = if started {
                        TRANSACTIONS_SYNCHRONIZER_TIMEOUT / 2
                    } else {
                        TRANSACTIONS_SYNCHRONIZER_TIMEOUT
                    };
                    scheduler_timeout.as_mut().reset(Instant::now() + next_tick);
                }
            }
        }
    }

    /// Serves the live fetch requests, at most
    /// `LIVE_FETCH_TRANSACTIONS_CONCURRENCY` at a time. Returns once the
    /// handle is dropped.
    async fn live_fetcher(
        active_requests: Arc<InflightActiveRequests>,
        network_client: Arc<C>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        mut receiver: Receiver<BTreeMap<TransactionRef, BTreeSet<AuthorityIndex>>>,
        inflight_transactions_map: Arc<InflightTransactionsMap>,
        block_verifier: Arc<dyn BlockVerifier>,
        misbehavior_store: Arc<MisbehaviorStore>,
    ) {
        // Dropped with this task on shutdown, which aborts the running fetches.
        let mut tasks = JoinSet::new();

        loop {
            tokio::select! {
                request = receiver.recv(), if tasks.len() < LIVE_FETCH_TRANSACTIONS_CONCURRENCY => {
                    let Some(missing_transactions) = request else {
                        // Channel closed → shutdown
                        info!("Live fetcher task will now abort.");
                        break;
                    };
                    let context = context.clone();
                    let active_requests = active_requests.clone();
                    let inflight_transactions_map = inflight_transactions_map.clone();
                    let network_client = network_client.clone();
                    let core_dispatcher = core_dispatcher.clone();
                    let block_verifier = block_verifier.clone();
                    let misbehavior_store = misbehavior_store.clone();
                    tasks.spawn(async move {
                        Self::fetch_and_process_transactions_from_authorities(
                            context,
                            active_requests,
                            inflight_transactions_map,
                            network_client,
                            missing_transactions,
                            core_dispatcher,
                            SyncMethod::Live,
                            block_verifier,
                            misbehavior_store,
                        )
                        .await;
                    });
                },
                Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                    match result {
                        Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
                        _ => {}
                    }
                },
            }
        }
    }

    /// Asks the core for the missing transactions and spawns a periodic fetch
    /// for them. Returns whether a fetch was spawned; fails only when the
    /// core is shutting down.
    async fn start_fetch_missing_transactions_task(&mut self) -> ConsensusResult<bool> {
        // Get missing transactions from the core
        let missing_transactions = self
            .core_dispatcher
            .get_missing_transaction_data()
            .await
            .map_err(|_err| ConsensusError::Shutdown)?;

        let dag_state = self.dag_state.clone();

        // Compute the gap to unavailable transactions.
        // If no missing transactions, the gap is zero; Otherwise, it is the difference
        // between the highest accepted round and the earliest unavailable transaction
        // round.
        let accepted_round = dag_state.read().highest_accepted_round();
        let earliest_unavailable_transaction_round = missing_transactions
            .first_key_value()
            .map(|(tx_ref, _)| tx_ref.round)
            .unwrap_or(accepted_round);
        let gap_to_unavailable_transactions =
            accepted_round.saturating_sub(earliest_unavailable_transaction_round);
        self.context
            .metrics
            .node_metrics
            .gap_to_unavailable_transactions
            .set(gap_to_unavailable_transactions as i64);

        let context = self.context.clone();

        // Update metrics for missing transactions per authority before fetching,
        // also when nothing is missing so that they return to zero.
        let mut missing_transactions_per_authority = vec![0; context.committee.size()];
        for tx_ref in missing_transactions.keys() {
            missing_transactions_per_authority[tx_ref.author] += 1;
        }
        for (missing, (_, authority)) in missing_transactions_per_authority
            .into_iter()
            .zip(context.committee.authorities())
        {
            context
                .metrics
                .node_metrics
                .transactions_synchronizer_missing_transactions_by_authority
                .with_label_values(&[&authority.hostname.as_str()])
                .inc_by(missing as u64);
            context
                .metrics
                .node_metrics
                .transactions_synchronizer_current_missing_transactions_by_authority
                .with_label_values(&[&authority.hostname.as_str()])
                .set(missing as i64);
        }

        if missing_transactions.is_empty() {
            return Ok(false);
        }

        let network_client = self.network_client.clone();
        let core_dispatcher = self.core_dispatcher.clone();
        let inflight_transactions_map = self.inflight_transactions_map.clone();
        let active_requests = self.active_requests.clone();
        let block_verifier = self.block_verifier.clone();
        let misbehavior_store = self.misbehavior_store.clone();

        self.fetch_transactions_scheduler_task
            .spawn(monitored_future!(async move {
                let _scope = monitored_scope("FetchMissingTransactionsScheduler");
                fail_point_async!("consensus-delay");
                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_periodic_inflight
                    .inc();
                // Fetch and process missing transactions
                Self::fetch_and_process_transactions_from_authorities(
                    context.clone(),
                    active_requests,
                    inflight_transactions_map,
                    network_client,
                    missing_transactions,
                    core_dispatcher,
                    SyncMethod::Periodic,
                    block_verifier,
                    misbehavior_store,
                )
                .await;
                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_periodic_inflight
                    .dec();
            }));
        Ok(true)
    }

    /// Fetches missing transactions from authorities.
    async fn fetch_and_process_transactions_from_authorities(
        context: Arc<Context>,
        active_requests: Arc<InflightActiveRequests>,
        inflight_transactions_map: Arc<InflightTransactionsMap>,
        network_client: Arc<C>,
        missing_transactions: BTreeMap<TransactionRef, BTreeSet<AuthorityIndex>>,
        core_dispatcher: Arc<D>,
        sync_method: SyncMethod,
        block_verifier: Arc<dyn BlockVerifier>,
        misbehavior_store: Arc<MisbehaviorStore>,
    ) {
        // Build a mapping from authority -> set of transaction refs it has
        // acknowledged. A committed payload was acknowledged by a quorum, so
        // when no acknowledger is known, every peer is asked for it.
        let committee: BTreeSet<AuthorityIndex> = context
            .committee
            .authorities()
            .map(|(index, _)| index)
            .collect();
        let mut transaction_refs_by_authority: BTreeMap<AuthorityIndex, BTreeSet<TransactionRef>> =
            BTreeMap::new();
        for (tx_ref, authorities) in &missing_transactions {
            let authorities = if authorities.is_empty() {
                &committee
            } else {
                authorities
            };
            for authority in authorities {
                if *authority != context.own_index {
                    transaction_refs_by_authority
                        .entry(*authority)
                        .or_default()
                        .insert(*tx_ref);
                }
            }
        }

        // For each authority, try to lock up the
        // maximum possible amount of acknowledged transactions and fetch
        // those transactions. The logic is as follows:
        // * Iterate, in the order chosen below, over all authorities that have acknowledged missing
        //   transactions.
        // * Attempt to lock max_transactions_per_fetch acknowledged transactions using the
        //   inflight_transactions_map. Some transactions may already be locked by other
        //   authorities, but continue with the transactions that were successfully locked.
        // * For each authority, if transactions were successfully locked, then send a request to
        //   the network client to fetch the transactions from the authority.
        // * If the transactions were successfully fetched, then process them and send them to the
        //   core for processing.
        // Each request is performed individually to avoid blocking the
        // synchronizer for too long, as certain peers may take a while to respond.
        // The number of requests to each peer is limited by the parameters.

        // Responsive peers are tried first when ranking is enabled. Without
        // ranking the order is uniformly random, or the committee order under
        // test so that scenarios stay deterministic.
        let mut rng = StdRng::from_rng(&mut rng());
        let mut order: Vec<AuthorityIndex> =
            transaction_refs_by_authority.keys().copied().collect();
        if context.parameters.enable_peer_responsiveness_ranking {
            context.peer_responsiveness.prioritize(
                DataSource::TransactionSynchronizer,
                &mut order,
                &mut rng,
            );
        } else {
            #[cfg(not(test))]
            order.shuffle(&mut rng);
        }

        let mut request_futures = FuturesUnordered::new();

        let mut assigned_authorities_for_transaction_fetch = 0;

        for authority in order {
            let authority_transaction_refs = transaction_refs_by_authority
                .remove(&authority)
                .expect("order should be a permutation of the candidate set");
            // * If transactions are successfully locked, and we didn't make too many to this
            //   authority, then send a request to the network client to fetch the transactions from
            //   the authority. If the fetch is successful, then process the transactions and send
            //   them to the core for processing.
            if let Some((transactions_guard, active_request_guard)) = inflight_transactions_map
                .lock_transactions_and_active_request(
                    authority_transaction_refs,
                    authority,
                    context
                        .parameters
                        .max_transactions_per_transaction_sync_fetch,
                    sync_method,
                    active_requests.clone(),
                )
            {
                let context = context.clone();
                let network_client = network_client.clone();
                let core_dispatcher = core_dispatcher.clone();
                let block_verifier = block_verifier.clone();
                let misbehavior_store = misbehavior_store.clone();
                request_futures.push(async move {
                    let result = Self::fetch_and_process_transactions_from_authority(
                        authority,
                        context,
                        transactions_guard,
                        network_client,
                        core_dispatcher,
                        sync_method,
                        active_request_guard,
                        block_verifier,
                        misbehavior_store,
                    )
                    .await;
                    (authority, result)
                });

                assigned_authorities_for_transaction_fetch += 1;
                if assigned_authorities_for_transaction_fetch
                    == MAX_ASSIGNED_AUTHORITIES_PER_TRANSACTION_FETCH
                {
                    break;
                }
            }
        }

        if request_futures.is_empty() {
            return;
        }

        // Await all authority requests to complete, feeding the per-peer
        // responsiveness signal. The recorded latency spans fetch and
        // verification. A successful fetch records that latency scaled up by
        // the fraction of requested transactions it returned, so unrelated or
        // partial responses cannot improve a peer's rank. An empty response is
        // a goodput failure; an error or timeout is recorded the same way, so a
        // failing peer is demoted to a timeout-scale latency and ordered behind
        // healthy peers in subsequent selections without ever being dropped
        // from the set.
        while let Some((peer, result)) = request_futures.next().await {
            match result {
                Ok(stats) if stats.matched_requested > 0 => {
                    let shortfall_factor =
                        (stats.requested as f64 / stats.matched_requested as f64).max(1.0);
                    // Capped at the failure penalty: a partial delivery never
                    // records worse than a failed fetch.
                    context.peer_responsiveness.record_success(
                        DataSource::TransactionSynchronizer,
                        peer,
                        stats
                            .latency
                            .mul_f64(shortfall_factor)
                            .min(FETCH_REQUEST_TIMEOUT),
                    );
                }
                Ok(_) => {
                    context.peer_responsiveness.record_failure_with_timeout(
                        DataSource::TransactionSynchronizer,
                        peer,
                        FETCH_REQUEST_TIMEOUT,
                    );
                }
                Err(err) => {
                    context.peer_responsiveness.record_failure_with_timeout(
                        DataSource::TransactionSynchronizer,
                        peer,
                        FETCH_REQUEST_TIMEOUT,
                    );
                    warn!(
                        "[{}] Error when fetching and processing transactions from authority {peer}: {err}",
                        sync_method.get_string(),
                    );
                }
            }
        }
    }

    /// Fetches and processes transactions from a specific authority peer.
    async fn fetch_and_process_transactions_from_authority(
        peer: AuthorityIndex,
        context: Arc<Context>,
        transactions_guard: TransactionsGuard,
        network_client: Arc<C>,
        core_dispatcher: Arc<D>,
        sync_method: SyncMethod,
        _active_guard: ActiveRequestGuard,
        block_verifier: Arc<dyn BlockVerifier>,
        misbehavior_store: Arc<MisbehaviorStore>,
    ) -> ConsensusResult<FetchStats> {
        let peer_hostname = &context.committee.authority(peer).hostname;
        let total_requested = transactions_guard.transactions_refs.len();

        debug!(
            "[{}] Syncing {total_requested} missing committed transactions from authority {peer} {peer_hostname}",
            sync_method.get_string(),
        );

        // Span the whole fetch-and-verify so the responsiveness latency includes
        // verification time, consistent with the commit syncer.
        let started = Instant::now();
        let (fetched_serialized_transactions, transactions_guard, peer) =
            Self::fetch_transactions_request(
                network_client.clone(),
                peer,
                transactions_guard,
                FETCH_REQUEST_TIMEOUT,
                context.clone(),
                sync_method,
                misbehavior_store.clone(),
            )
            .await?;

        let total_fetched = fetched_serialized_transactions.len();

        debug!(
            "Transactions from {total_requested} blocks requested, fetched from {total_fetched} blocks"
        );

        let matched_requested = Self::process_fetched_transactions(
            fetched_serialized_transactions,
            peer,
            transactions_guard,
            core_dispatcher.clone(),
            context,
            sync_method,
            block_verifier,
            misbehavior_store,
        )
        .await?;

        Ok(FetchStats {
            latency: started.elapsed(),
            requested: total_requested,
            matched_requested,
        })
    }

    /// Fetches transactions from a peer authority for the given block
    /// references. Returns the fetched transactions, the transactions
    /// guard, and the peer index.
    async fn fetch_transactions_request(
        network_client: Arc<C>,
        peer: AuthorityIndex,
        transactions_guard: TransactionsGuard,
        request_timeout: Duration,
        context: Arc<Context>,
        sync_method: SyncMethod,
        misbehavior_store: Arc<MisbehaviorStore>,
    ) -> ConsensusResult<(Vec<Bytes>, TransactionsGuard, AuthorityIndex)> {
        // Track concurrent inflight requests
        let inflight_metric = &context
            .metrics
            .node_metrics
            .transactions_synchronizer_inflight_requests;
        inflight_metric.inc();
        let _guard = InflightGuard {
            metric: inflight_metric,
        };

        let requested_transactions_refs = transactions_guard
            .transactions_refs
            .iter()
            .cloned()
            .collect::<Vec<TransactionRef>>();

        let peer_hostname = &context.committee.authority(peer).hostname;
        let start_time = Instant::now();
        // Fetch the transactions from the peer
        let result = timeout(
            request_timeout,
            network_client.fetch_transactions(
                peer,
                requested_transactions_refs.clone(),
                request_timeout,
            ),
        )
        .await;

        fail_point_async!("consensus-delay");

        // Record fetch latency
        let fetch_duration = start_time.elapsed();
        context
            .metrics
            .node_metrics
            .transactions_synchronizer_fetch_latency
            .observe(fetch_duration.as_secs_f64());
        context
            .metrics
            .node_metrics
            .transactions_synchronizer_fetch_latency_by_peer
            .with_label_values(&[peer_hostname.as_str(), &sync_method.get_string()])
            .observe(fetch_duration.as_secs_f64());

        let resp = match result {
            Ok(Err(err)) => {
                misbehavior_store.record_fetch_fault(peer, &err);
                // Record failure
                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_failure_by_peer
                    .with_label_values(&[
                        peer_hostname.as_str(),
                        &sync_method.get_string(),
                        err.name(),
                    ])
                    .inc();

                Err(err) // network error
            }
            Err(err) => {
                // Record timeout failure
                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_failure_by_peer
                    .with_label_values(&[
                        peer_hostname.as_str(),
                        &sync_method.get_string(),
                        "timeout",
                    ])
                    .inc();
                // timeout
                Err(ConsensusError::NetworkRequestTimeout(err.to_string()))
            }
            Ok(result) => {
                // Record success
                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_success_by_peer
                    .with_label_values(&[peer_hostname.as_str(), &sync_method.get_string()])
                    .inc();

                result
            }
        };
        resp.map(|txs| (txs, transactions_guard, peer))
    }

    /// Processes the requested raw fetched transactions from peer `peer_index`.
    /// If no error is returned then the verified transactions are
    /// immediately sent to Core for processing. Returns an error if the
    /// response contains a transaction that was not requested.
    async fn process_fetched_transactions(
        serialized_transactions_vec: Vec<Bytes>,
        peer_index: AuthorityIndex,
        requested_transactions_guard: TransactionsGuard,
        core_dispatcher: Arc<D>,
        context: Arc<Context>,
        sync_method: SyncMethod,
        block_verifier: Arc<dyn BlockVerifier>,
        misbehavior_store: Arc<MisbehaviorStore>,
    ) -> ConsensusResult<usize> {
        let _s = context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Synchronizer::process_fetched_transactions"])
            .start_timer();
        // Ensure that all the returned transactions do not go over the total max
        // allowed returned transactions
        if serialized_transactions_vec.len() > requested_transactions_guard.transactions_refs.len()
        {
            misbehavior_store.record_faulty_transactions(peer_index, false, [peer_index]);
            return Err(ConsensusError::TooManyFetchedTransactionsReturned(
                peer_index,
            ));
        }
        let metrics = &context.metrics.node_metrics;
        let peer_hostname = &context.committee.authority(peer_index).hostname;

        // Deserialization, commitment checks and the transaction batch
        // verification run on the blocking pool.
        let transactions = spawn_blocking({
            let context = context.clone();
            let block_verifier = block_verifier.clone();
            let misbehavior_store = misbehavior_store.clone();
            let requested_transactions_refs =
                requested_transactions_guard.transactions_refs.clone();
            move || {
                Self::verify_fetched_transactions(
                    serialized_transactions_vec,
                    &requested_transactions_refs,
                    peer_index,
                    &context,
                    block_verifier.as_ref(),
                    &misbehavior_store,
                )
            }
        })
        .await??;

        metrics
            .transactions_synchronizer_fetched_transactions_by_peer
            .with_label_values(&[peer_hostname.as_str(), &sync_method.get_string()])
            .inc_by(transactions.len() as u64);
        for transactions in &transactions {
            let block_hostname = &context.committee.authority(transactions.author()).hostname;
            metrics
                .transactions_synchronizer_fetched_transactions_by_authority
                .with_label_values(&[block_hostname.as_str(), &sync_method.get_string()])
                .inc();
        }

        let matched_requested = transactions.len();

        // Add the transactions to the core
        core_dispatcher
            .add_transactions(transactions, DataSource::TransactionSynchronizer)
            .await
            .map_err(|_| ConsensusError::Shutdown)?;

        // now release all the locked blocks as they have been fetched, verified &
        // processed
        drop(requested_transactions_guard);

        Ok(matched_requested)
    }

    /// Deserializes the fetched payloads, checks each against the commitment
    /// in its transaction reference and runs the same validity checks as the
    /// block-bundle route. Records metrics and misbehavior for a failure.
    fn verify_fetched_transactions(
        serialized_transactions_vec: Vec<Bytes>,
        requested_transactions_refs: &BTreeSet<TransactionRef>,
        peer_index: AuthorityIndex,
        context: &Arc<Context>,
        block_verifier: &dyn BlockVerifier,
        misbehavior_store: &MisbehaviorStore,
    ) -> ConsensusResult<Vec<CommitmentVerifiedTransactions>> {
        let metrics = &context.metrics.node_metrics;
        let peer_hostname = &context.committee.authority(peer_index).hostname;

        let mut serialized_transactions_map: BTreeMap<TransactionRef, Bytes> = BTreeMap::new();
        for serialized_transaction_bytes in &serialized_transactions_vec {
            let serialized_transactions: SerializedTransactionsV2 =
                bcs::from_bytes(serialized_transaction_bytes)
                    .inspect_err(|_| {
                        misbehavior_store.record_faulty_transactions(
                            peer_index,
                            false,
                            [peer_index],
                        )
                    })
                    .map_err(ConsensusError::MalformedTransactions)?;
            let committed_transaction_ref = serialized_transactions.transaction_ref;
            // The commitment check below only proves each payload matches
            // its own claimed ref; it does not tie the ref to anything we
            // asked for. Reject a ref outside the requested set so a peer
            // cannot serve correctly-committed transactions we never
            // requested (which need not correspond to any real header).
            if !requested_transactions_refs.contains(&committed_transaction_ref) {
                misbehavior_store.record_faulty_transactions(peer_index, false, [peer_index]);
                return Err(ConsensusError::UnrequestedTransactionFetched {
                    peer: peer_index,
                    transaction_ref: serialized_transactions.transaction_ref,
                });
            }
            serialized_transactions_map.insert(
                committed_transaction_ref,
                serialized_transactions.serialized_transactions,
            );
        }

        let transactions: Vec<_> =
            match verify_transactions_commitments(context, peer_index, serialized_transactions_map)
            {
                Ok(transactions) => transactions.into_values().collect(),
                Err(err) => {
                    // The serving peer relayed a payload whose bytes don't match a
                    // committed payload; count it against that peer and charge it an
                    // unprovable fault. The mismatch can't be proven against the
                    // author, whose commitment the peer may have forged.
                    metrics
                        .invalid_transactions
                        .with_label_values(&[
                            peer_hostname.as_str(),
                            "transaction_synchronizer",
                            err.name(),
                        ])
                        .inc();
                    misbehavior_store.record_faulty_transactions(peer_index, false, [peer_index]);
                    return Err(err);
                }
            };

        // The commitment check above only proves the fetched bytes match what
        // the author committed to; it does not enforce the per-transaction
        // limits or the application-level `verify_batch` checks the direct
        // block-bundle route applies before a payload can be acknowledged.
        // Run the same checks here so a payload violating them can't be
        // acknowledged and become committable via this route either.
        for verified_transactions in &transactions {
            if let Err(err) = block_verifier.verify_transactions_validity(verified_transactions) {
                let author = verified_transactions.author();
                metrics
                    .invalid_transactions
                    .with_label_values(&[
                        peer_hostname.as_str(),
                        "transaction_synchronizer",
                        err.name(),
                    ])
                    .inc();
                // The recomputed commitment (checked above) ties this payload to
                // the author's signed transactions_commitment, so the invalid
                // payload is provably the author's. `peer_index` served a full
                // payload it could have verified before relaying, so it is also
                // charged an unprovable fault.
                misbehavior_store.record_faulty_transactions(author, true, [peer_index]);
                return Err(err);
            }
        }
        Ok(transactions)
    }
}

struct InflightGuard<'a> {
    metric: &'a prometheus_filtered::IntGauge,
}

impl<'a> Drop for InflightGuard<'a> {
    fn drop(&mut self) {
        self.metric.dec();
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, slice, sync::Arc, time::Duration};

    use async_trait::async_trait;
    use bytes::Bytes;
    use tokio::time::sleep;

    use super::*;
    use crate::{
        Round, TestBlockHeader, Transaction,
        authority_service::serialize_transactions_entry,
        block_header::{
            BlockRef, CommitmentVerifiedTransactions, TransactionsCommitment, VerifiedBlockHeader,
        },
        block_verifier::{NoopBlockVerifier, SignedBlockVerifier, test::TxnSizeVerifier},
        commit::CommitRange,
        context::Context,
        core_thread::tests::MockCoreThreadDispatcher,
        dag_state::{DagState, DataSource},
        encoder::create_encoder,
        network::{BlockBundleStream, FetchedCommitsAndTransactions, NetworkClient},
        storage::mem_store::MemStore,
        transaction_ref::TransactionRef,
    };

    /// How long `wait_until` waits before failing the test.
    const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

    /// A started synchronizer with its mocks.
    struct Fixture {
        context: Arc<Context>,
        core_dispatcher: Arc<MockCoreThreadDispatcher>,
        network_client: Arc<MockNetworkClient>,
        dag_state: Arc<RwLock<DagState>>,
        handle: Arc<TransactionsSynchronizerHandle>,
    }

    impl Fixture {
        fn new(committee_size: usize) -> Self {
            let (context, _) = Context::new_for_test(committee_size);
            Self::start(Arc::new(context), Arc::new(NoopBlockVerifier))
        }

        fn start(context: Arc<Context>, block_verifier: Arc<dyn BlockVerifier>) -> Self {
            telemetry_subscribers::init_for_testing();
            let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
            let network_client = Arc::new(MockNetworkClient::default());
            let dag_state = Arc::new(RwLock::new(DagState::new(
                context.clone(),
                Arc::new(MemStore::new()),
            )));
            let handle = TransactionsSynchronizer::start(
                network_client.clone(),
                context.clone(),
                core_dispatcher.clone(),
                dag_state.clone(),
                block_verifier,
            );
            Self {
                context,
                core_dispatcher,
                network_client,
                dag_state,
                handle,
            }
        }

        /// Payloads of one 32-byte transaction per `(round, author)`.
        fn payloads(&self, blocks: &[(Round, u8)]) -> Vec<CommitmentVerifiedTransactions> {
            self.payloads_with(
                blocks
                    .iter()
                    .map(|&(round, author)| (round, author, vec![Transaction::new(vec![0; 32])])),
            )
        }

        fn payloads_with(
            &self,
            blocks: impl IntoIterator<Item = (Round, u8, Vec<Transaction>)>,
        ) -> Vec<CommitmentVerifiedTransactions> {
            let mut encoder = create_encoder(&self.context);
            blocks
                .into_iter()
                .map(|(round, author, transactions)| {
                    let header = VerifiedBlockHeader::new_for_test(
                        TestBlockHeader::with_transactions(
                            round,
                            author,
                            transactions.clone(),
                            &self.context,
                            &mut encoder,
                        )
                        .build(),
                    );
                    CommitmentVerifiedTransactions::new_for_test(&header, transactions)
                })
                .collect()
        }

        fn misbehavior_counts(&self, authority: u8) -> (u64, u64) {
            let counts = self.dag_state.read().misbehavior_store().snapshot_totals();
            let counts = counts[AuthorityIndex::new_for_test(authority).value()].as_v2();
            (
                counts.faulty_blocks_provable,
                counts.faulty_blocks_unprovable,
            )
        }

        /// The fetch latency recorded for `peer`, if any.
        fn latency_ms(&self, peer: u8) -> Option<f64> {
            self.context.peer_responsiveness.effective_latency_ms(
                DataSource::TransactionSynchronizer,
                AuthorityIndex::new_for_test(peer),
            )
        }

        /// Waits until as many payloads as `payloads` reach the core, then
        /// checks they are exactly `payloads`.
        async fn assert_fetched(&self, payloads: &[CommitmentVerifiedTransactions]) {
            wait_until(|| self.core_dispatcher.added_transactions().len() >= payloads.len()).await;
            let added: BTreeSet<_> = self
                .core_dispatcher
                .added_transactions()
                .iter()
                .map(|t| t.transaction_ref())
                .collect();
            let expected: BTreeSet<_> = payloads.iter().map(|p| p.transaction_ref()).collect();
            assert_eq!(added, expected);
        }

        async fn stop(self) {
            self.handle.stop().await.unwrap();
        }
    }

    /// Polls `condition` every 10 ms until it holds; panics after
    /// `WAIT_TIMEOUT`.
    async fn wait_until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + WAIT_TIMEOUT;
        while !condition() {
            assert!(
                Instant::now() < deadline,
                "condition not met within {WAIT_TIMEOUT:?}"
            );
            sleep(Duration::from_millis(10)).await;
        }
    }

    /// The missing map naming `peers` as the acknowledgers of every payload.
    fn acknowledged_by(
        payloads: &[CommitmentVerifiedTransactions],
        peers: &[u8],
    ) -> BTreeMap<TransactionRef, BTreeSet<AuthorityIndex>> {
        let peers: BTreeSet<_> = peers
            .iter()
            .map(|&peer| AuthorityIndex::new_for_test(peer))
            .collect();
        payloads
            .iter()
            .map(|payload| (payload.transaction_ref(), peers.clone()))
            .collect()
    }

    /// Peer 1 behaves as `behavior` and peer 2 serves; every payload is still
    /// fetched. Returns the fixture for further assertions.
    async fn fetches_despite_bad_peer(behavior: PeerBehavior, ranking: bool) -> Fixture {
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.enable_peer_responsiveness_ranking = ranking;
        let fixture = Fixture::start(Arc::new(context), Arc::new(NoopBlockVerifier));
        let payloads = fixture.payloads(&[(1, 0), (2, 1), (3, 2)]);
        fixture.network_client.set_behavior(1, behavior);
        fixture.network_client.serve(2, &payloads);

        fixture
            .handle
            .fetch_transactions(acknowledged_by(&payloads, &[1, 2]))
            .unwrap();

        fixture.assert_fetched(&payloads).await;
        fixture
    }

    #[tokio::test]
    async fn successful_live_syncing() {
        let fixture = Fixture::new(4);
        let payloads = fixture.payloads(&[(1, 1), (2, 1), (3, 2)]);
        fixture.network_client.serve(1, &payloads);

        fixture
            .handle
            .fetch_transactions(acknowledged_by(&payloads, &[1, 2]))
            .unwrap();

        fixture.assert_fetched(&payloads).await;
        fixture.stop().await;
    }

    /// A fetched payload must pass the same per-transaction limit and
    /// `verify_batch` checks the direct route enforces before it can reach
    /// Core. Otherwise it could be acknowledged and become committable while
    /// diverging from nodes that received the same payload directly.
    #[tokio::test]
    async fn live_syncing_rejects_transactions_failing_validity_check() {
        // GIVEN a block verifier that rejects transactions shorter than 4
        // bytes.
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(TxnSizeVerifier {}),
        ));
        let fixture = Fixture::start(context, block_verifier);
        let author = 1;
        let payloads = fixture.payloads_with([(1, author, vec![Transaction::new(vec![0u8; 2])])]);
        fixture.network_client.serve(author, &payloads);

        // WHEN
        fixture
            .handle
            .fetch_transactions(acknowledged_by(&payloads, &[author]))
            .unwrap();
        wait_until(|| fixture.misbehavior_counts(author).0 >= 1).await;

        // THEN the author is charged for the provably invalid payload, not
        // separately as the peer, and the payload never reaches Core, so it
        // can never be acknowledged.
        assert_eq!(fixture.misbehavior_counts(author), (1, 0));
        assert!(fixture.core_dispatcher.added_transactions().is_empty());
        fixture.stop().await;
    }

    #[tokio::test]
    async fn live_syncing_charges_serving_peer_for_too_many_transactions() {
        // GIVEN a synchronizer requesting a single transaction from peer 1.
        let fixture = Fixture::new(4);
        let payloads = fixture.payloads(&[(1, 2)]);
        let peer = 1;
        fixture
            .network_client
            .set_behavior(peer, PeerBehavior::Fixed(vec![serialize(&payloads[0]); 2]));

        // WHEN the peer returns more transactions than requested.
        fixture
            .handle
            .fetch_transactions(acknowledged_by(&payloads, &[peer]))
            .unwrap();
        wait_until(|| fixture.misbehavior_counts(peer).1 >= 1).await;

        // THEN the peer is charged one unprovable fault and nothing reaches
        // the core.
        assert_eq!(fixture.misbehavior_counts(peer), (0, 1));
        assert!(fixture.core_dispatcher.added_transactions().is_empty());
        fixture.stop().await;
    }

    #[tokio::test]
    async fn live_syncing_with_saturated_tasks() {
        // GIVEN one timing-out peer per request, so every live fetch holds
        // its slot for the whole request timeout.
        let requests = LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 3;
        let fixture = Fixture::new(requests + 1);
        let blocks: Vec<_> = (1..=requests as Round).map(|round| (round, 1)).collect();
        let payloads = fixture.payloads(&blocks);
        let missing_per_request: Vec<_> = (1..)
            .zip(&payloads)
            .map(|(peer, payload)| {
                fixture
                    .network_client
                    .set_behavior(peer, PeerBehavior::Timeout);
                acknowledged_by(slice::from_ref(payload), &[peer])
            })
            .collect();

        // WHEN more requests arrive than the live fetcher can hold.
        let mut results = Vec::new();
        for missing in missing_per_request {
            results.push(fixture.handle.fetch_transactions(missing));
            // Let the live fetcher take the request before the next one lands.
            tokio::task::yield_now().await;
        }

        // THEN LIVE_FETCH_TRANSACTIONS_CONCURRENCY requests are being served,
        // as many wait in the queue, and the rest are rejected as saturated.
        let accepted = results.iter().filter(|r| r.is_ok()).count();
        let saturated = results
            .iter()
            .filter(|r| matches!(r, Err(ConsensusError::TransactionSynchronizerSaturated)))
            .count();
        assert_eq!(accepted, LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 2);
        assert_eq!(saturated, LIVE_FETCH_TRANSACTIONS_CONCURRENCY);
        fixture.stop().await;
    }

    /// Peers are asked concurrently: a peer that does not answer must not
    /// hold back the delivery from one that does, and is recorded as failed
    /// once its request times out.
    #[tokio::test]
    async fn live_syncing_with_timeout_peer() {
        let started = Instant::now();
        let fixture = fetches_despite_bad_peer(PeerBehavior::Timeout, false).await;
        assert!(started.elapsed() < FETCH_REQUEST_TIMEOUT);

        let timed_out_peer = 1;
        wait_until(|| fixture.latency_ms(timed_out_peer).is_some()).await;
        let timeouts = fixture
            .context
            .metrics
            .node_metrics
            .transactions_synchronizer_failure_by_peer
            .with_label_values(&[
                fixture
                    .context
                    .committee
                    .authority(AuthorityIndex::new_for_test(timed_out_peer))
                    .hostname
                    .as_str(),
                &SyncMethod::Live.get_string(),
                "timeout",
            ]);
        assert_eq!(timeouts.get(), 1);
        let latency = fixture.latency_ms(timed_out_peer).unwrap();
        assert!(latency >= FETCH_REQUEST_TIMEOUT.as_millis() as f64);
        fixture.stop().await;
    }

    #[tokio::test]
    async fn live_syncing_with_empty_peer() {
        let fixture = fetches_despite_bad_peer(PeerBehavior::Empty, false).await;
        fixture.stop().await;
    }

    #[tokio::test]
    async fn live_syncing_with_corrupted_peer() {
        let fixture = fetches_despite_bad_peer(PeerBehavior::Corrupted, false).await;

        // The corrupted peer is charged an unprovable fault for serving
        // undeserializable bytes.
        wait_until(|| fixture.misbehavior_counts(1).1 >= 1).await;
        assert_eq!(fixture.misbehavior_counts(1), (0, 1));
        fixture.stop().await;
    }

    /// Both peers get a latency record, and the error peer ranks slower than
    /// the peer that delivered.
    #[tokio::test]
    async fn responsiveness_feedback_records_success_and_failure() {
        let error = ConsensusError::NetworkRequest("boom".to_string());
        let fixture = fetches_despite_bad_peer(PeerBehavior::Error(error), true).await;

        wait_until(|| fixture.latency_ms(1).is_some() && fixture.latency_ms(2).is_some()).await;
        let (failure, success) = (
            fixture.latency_ms(1).unwrap(),
            fixture.latency_ms(2).unwrap(),
        );
        assert!(
            failure > success,
            "error peer ({failure}) must rank slower than success peer ({success})",
        );
        fixture.stop().await;
    }

    /// The commitment check only proves a payload matches its own claimed
    /// ref. A peer serving a self-consistent payload for a ref that was never
    /// requested has its whole response rejected, including any requested
    /// payload in it, and is charged for it; the author it named is not.
    #[tokio::test]
    async fn live_syncing_rejects_unrequested_transactions() {
        let fixture = Fixture::new(4);
        let payloads = fixture.payloads(&[(1, 0), (2, 1), (3, 2)]);
        let unrequested = fixture.payloads(&[(9, 3)]);
        let peer = 1;
        // Two entries for three requested refs, so the length check passes.
        fixture.network_client.set_behavior(
            peer,
            PeerBehavior::Fixed(vec![serialize(&payloads[0]), serialize(&unrequested[0])]),
        );

        // WHEN the peer returns the unrequested payload with a requested one.
        fixture
            .handle
            .fetch_transactions(acknowledged_by(&payloads, &[peer]))
            .unwrap();
        wait_until(|| fixture.misbehavior_counts(peer).1 >= 1).await;

        // THEN the peer is charged one unprovable fault, nothing reaches the
        // core and the named author is not blamed.
        assert_eq!(fixture.misbehavior_counts(peer), (0, 1));
        assert!(fixture.core_dispatcher.added_transactions().is_empty());
        assert_eq!(fixture.misbehavior_counts(3), (0, 0));
        fixture.stop().await;
    }

    #[tokio::test]
    async fn live_syncing_with_all_peers_failing() {
        let fixture = Fixture::new(4);
        let payloads = fixture.payloads(&[(1, 0), (2, 1), (3, 2)]);
        fixture
            .network_client
            .set_behavior(1, PeerBehavior::Timeout);
        fixture.network_client.set_behavior(
            2,
            PeerBehavior::Error(ConsensusError::NetworkRequest("Test error".to_string())),
        );

        fixture
            .handle
            .fetch_transactions(acknowledged_by(&payloads, &[1, 2]))
            .unwrap();
        // Peer 1 never answers, so once peer 2's failure is recorded nothing
        // can reach the core.
        wait_until(|| fixture.latency_ms(2).is_some()).await;

        assert!(fixture.core_dispatcher.added_transactions().is_empty());
        fixture.stop().await;
    }

    /// A live fetch must not wait for the periodic core query, which can
    /// take as long as the core thread's queue.
    #[tokio::test]
    async fn live_fetch_proceeds_while_periodic_core_query_is_blocked() {
        let fixture = Fixture::new(4);
        fixture.core_dispatcher.block_missing_transactions_queries();
        timeout(
            WAIT_TIMEOUT,
            fixture.core_dispatcher.missing_transactions_query_entered(),
        )
        .await
        .expect("the periodic scheduler queries the core");
        let payloads = fixture.payloads(&[(1, 1)]);
        fixture.network_client.serve(2, &payloads);

        fixture
            .handle
            .fetch_transactions(acknowledged_by(&payloads, &[2]))
            .unwrap();

        fixture.assert_fetched(&payloads).await;
        fixture.stop().await;
    }

    /// The per-authority missing gauge is reset once the core reports
    /// nothing missing, not left at its last non-zero value.
    #[tokio::test]
    async fn periodic_fetch_clears_the_missing_transactions_gauge() {
        let fixture = Fixture::new(4);
        let author = 1;
        let payloads = fixture.payloads(&[(1, author), (2, author)]);
        fixture
            .core_dispatcher
            .stub_missing_transactions(acknowledged_by(&payloads, &[2]));
        let gauge = fixture
            .context
            .metrics
            .node_metrics
            .transactions_synchronizer_current_missing_transactions_by_authority
            .with_label_values(&[fixture
                .context
                .committee
                .authority(AuthorityIndex::new_for_test(author))
                .hostname
                .as_str()]);

        // While peer 2 serves nothing, every tick reports both payloads
        // missing.
        wait_until(|| gauge.get() == payloads.len() as i64).await;

        // Once peer 2 serves them, they are delivered and the next tick finds
        // nothing missing.
        fixture.network_client.serve(2, &payloads);
        fixture.assert_fetched(&payloads).await;
        wait_until(|| gauge.get() == 0).await;
        fixture.stop().await;
    }

    #[tokio::test]
    async fn inflight_transactions_map_with_active_requests() {
        telemetry_subscribers::init_for_testing();

        // GIVEN
        let map = InflightTransactionsMap::new();
        let active_requests = InflightActiveRequests::new();
        let sync_method = SyncMethod::Periodic;

        let context = Context::new_for_test(10).0;
        let missing_transactions_refs = [
            (1, AuthorityIndex::new_for_test(0)),
            (10, AuthorityIndex::new_for_test(0)),
            (12, AuthorityIndex::new_for_test(3)),
            (15, AuthorityIndex::new_for_test(2)),
        ]
        .into_iter()
        .map(|(round, author)| TransactionRef {
            round,
            author,
            transactions_commitment: TransactionsCommitment::MIN,
        })
        .collect::<BTreeSet<_>>();
        // We keep both guards so that drops happen at the end
        let mut all_guards: Vec<(TransactionsGuard, ActiveRequestGuard)> = Vec::new();

        // Try to acquire the transaction locks for authorities
        // 0..MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION
        for i in 0..=MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION {
            let authority = AuthorityIndex::new_for_test(i as u8);

            let guard = map.lock_transactions_and_active_request(
                missing_transactions_refs.clone(),
                authority,
                context
                    .parameters
                    .max_transactions_per_transaction_sync_fetch,
                sync_method,
                active_requests.clone(),
            );

            if i == MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION {
                // Trying to acquire for authority MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION will
                // fail - as we have maxed out the number of allowed peers for
                // each transaction
                assert!(guard.is_none());
                break;
            }
            let (tx_guard, ar_guard) = guard.expect("Guard should be created");
            assert_eq!(tx_guard.transactions_refs.len(), 4);

            all_guards.push((tx_guard, ar_guard));

            // trying to acquire any of them again for the *same* authority should not
            // succeed

            let guard = map.lock_transactions_and_active_request(
                missing_transactions_refs.clone(),
                authority,
                context
                    .parameters
                    .max_transactions_per_transaction_sync_fetch,
                sync_method,
                active_requests.clone(),
            );
            assert!(guard.is_none());
        }

        // Explicitly drop the guard of authority 1 (the first we stored) and try for
        // authority 3 again - it will now succeed because one slot per-block
        // got freed
        drop(all_guards.remove(0));

        let guard = map.lock_transactions_and_active_request(
            missing_transactions_refs.clone(),
            AuthorityIndex::new_for_test(MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION as u8),
            context
                .parameters
                .max_transactions_per_transaction_sync_fetch,
            sync_method,
            active_requests,
        );
        let (tx_guard, active_request_guard) =
            guard.expect("Guard should be successfully acquired");
        assert_eq!(tx_guard.transactions_refs, missing_transactions_refs);

        // Dropping all guards should unlock all block refs
        drop(tx_guard);
        drop(active_request_guard);
        drop(all_guards);

        assert_eq!(map.num_of_locked_transactions(), 0);
    }

    /// A committed payload with no known acknowledger is fetched from any peer
    /// that holds it.
    #[tokio::test]
    async fn live_syncing_without_acknowledgers_asks_every_peer() {
        // Three validators leave two peers, and at most two peers are asked
        // per transaction, so the serving peer is asked in any order.
        let fixture = Fixture::new(3);
        let payloads = fixture.payloads(&[(1, 1)]);
        fixture.network_client.serve(2, &payloads);

        fixture
            .handle
            .fetch_transactions(acknowledged_by(&payloads, &[]))
            .unwrap();

        fixture.assert_fetched(&payloads).await;
        fixture.stop().await;
    }

    /// How a peer answers a fetch instead of serving its stubbed payloads.
    #[derive(Clone)]
    enum PeerBehavior {
        /// Never answers; the caller's request timeout fires.
        Timeout,
        Error(ConsensusError),
        Empty,
        /// Undeserializable bytes, one per requested ref.
        Corrupted,
        /// A fixed response regardless of the request.
        Fixed(Vec<Bytes>),
    }

    #[derive(Default)]
    struct MockNetworkClient {
        served: Mutex<HashMap<(AuthorityIndex, TransactionRef), Bytes>>,
        behaviors: Mutex<HashMap<AuthorityIndex, PeerBehavior>>,
    }

    impl MockNetworkClient {
        fn serve(&self, peer: u8, payloads: &[CommitmentVerifiedTransactions]) {
            let peer = AuthorityIndex::new_for_test(peer);
            let mut served = self.served.lock();
            for payload in payloads {
                served.insert((peer, payload.transaction_ref()), serialize(payload));
            }
        }

        fn set_behavior(&self, peer: u8, behavior: PeerBehavior) {
            self.behaviors
                .lock()
                .insert(AuthorityIndex::new_for_test(peer), behavior);
        }
    }

    /// The fetch-response entry the authority service would serve.
    fn serialize(transactions: &CommitmentVerifiedTransactions) -> Bytes {
        serialize_transactions_entry(
            transactions.transaction_ref(),
            transactions.serialized().clone(),
        )
        .unwrap()
    }

    #[async_trait]
    impl NetworkClient for MockNetworkClient {
        async fn subscribe_block_bundles(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockBundleStream> {
            unimplemented!("subscribe_block_bundles not implemented in mock")
        }

        async fn fetch_transactions(
            &self,
            peer: AuthorityIndex,
            transaction_refs: Vec<TransactionRef>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            let behavior = self.behaviors.lock().get(&peer).cloned();
            match behavior {
                Some(PeerBehavior::Timeout) => return std::future::pending().await,
                Some(PeerBehavior::Error(error)) => return Err(error),
                Some(PeerBehavior::Empty) => return Ok(Vec::new()),
                Some(PeerBehavior::Corrupted) => {
                    return Ok(vec![Bytes::from(vec![0, 1, 2, 3]); transaction_refs.len()]);
                }
                Some(PeerBehavior::Fixed(response)) => return Ok(response),
                None => {}
            }

            let served = self.served.lock();
            Ok(transaction_refs
                .iter()
                .filter_map(|transaction_ref| served.get(&(peer, *transaction_ref)).cloned())
                .collect())
        }

        async fn fetch_block_headers(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<BlockRef>,
            _highest_accepted_rounds: Vec<Round>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("fetch_block_headers not implemented in mock")
        }

        async fn fetch_commits(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("fetch_commits not implemented in mock")
        }

        async fn fetch_latest_block_headers(
            &self,
            _peer: AuthorityIndex,
            _authorities: Vec<AuthorityIndex>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("fetch_latest_block_headers not implemented in mock")
        }

        async fn fetch_commits_and_transactions(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<FetchedCommitsAndTransactions> {
            unimplemented!("fetch_commits_and_transactions not implemented in mock")
        }
    }
}
