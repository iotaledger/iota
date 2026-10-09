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
    GaugeGuard, monitored_future,
    monitored_mpsc::{Receiver, Sender, channel},
    monitored_scope,
};
use parking_lot::{Mutex, RwLock};
use rand::{SeedableRng, rng, rngs::StdRng, seq::SliceRandom};
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
    commit_syncer::{shortfall_factor, verify_transactions_commitments},
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::{DagState, DataSource},
    error::{ConsensusError, ConsensusResult},
    misbehavior_store::MisbehaviorStore,
    network::{NetworkClient, SerializedTransactionsV2},
    task::spawn_blocking,
    transaction_ref::{GenericTransactionRef, TransactionRef},
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
    fn as_str(self) -> &'static str {
        match self {
            SyncMethod::Live => "live",
            SyncMethod::Periodic => "periodic",
        }
    }
}

/// Outcome of a successful fetch from one authority.
struct FetchStats {
    latency: Duration,
    requested: usize,
    matched_requested: usize,
}

/// Bounds the concurrent fetches: at most
/// `MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION` peers fetch the same transaction
/// at a time, and each peer serves at most
/// `MAX_CONCURRENT_REQUESTS_PER_AUTHORITY` requests per sync method.
#[derive(Default)]
struct InflightTransactionsMap {
    inner: Mutex<InflightState>,
}

#[derive(Default)]
struct InflightState {
    /// The peers currently fetching each transaction.
    fetching_peers: HashMap<TransactionRef, BTreeSet<AuthorityIndex>>,
    /// The number of requests in flight per peer and sync method.
    active_requests: BTreeMap<(AuthorityIndex, SyncMethod), usize>,
}

impl InflightTransactionsMap {
    /// Locks up to `max_transactions` of `missing_transaction_refs` for
    /// `peer`, skipping those enough other peers are already fetching. Returns
    /// `None` when `peer` has too many requests in flight or nothing could be
    /// locked.
    fn lock_transactions(
        self: &Arc<Self>,
        missing_transaction_refs: BTreeSet<TransactionRef>,
        peer: AuthorityIndex,
        max_transactions: usize,
        sync_method: SyncMethod,
    ) -> Option<TransactionsGuard> {
        let mut guard = self.inner.lock();
        let state = &mut *guard;
        let active_requests = state
            .active_requests
            .entry((peer, sync_method))
            .or_insert(0);
        if *active_requests >= MAX_CONCURRENT_REQUESTS_PER_AUTHORITY {
            return None;
        }

        let mut transactions_refs = BTreeSet::new();
        for tx_ref in missing_transaction_refs {
            let peers = state.fetching_peers.entry(tx_ref).or_default();
            if peers.len() < MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION && peers.insert(peer) {
                transactions_refs.insert(tx_ref);
                if transactions_refs.len() >= max_transactions {
                    break;
                }
            }
        }
        if transactions_refs.is_empty() {
            return None;
        }
        *active_requests += 1;

        Some(TransactionsGuard {
            map: self.clone(),
            transactions_refs,
            peer,
            sync_method,
        })
    }

    #[cfg(test)]
    fn num_of_locked_transactions(&self) -> usize {
        self.inner.lock().fetching_peers.len()
    }
}

/// Holds the transactions locked for one request to `peer`; releases them and
/// the peer's request slot on drop.
struct TransactionsGuard {
    map: Arc<InflightTransactionsMap>,
    transactions_refs: BTreeSet<TransactionRef>,
    peer: AuthorityIndex,
    sync_method: SyncMethod,
}

impl Drop for TransactionsGuard {
    fn drop(&mut self) {
        let mut state = self.map.inner.lock();
        for tx_ref in &self.transactions_refs {
            let peers = state
                .fetching_peers
                .get_mut(tx_ref)
                .expect("a locked transaction is tracked");
            assert!(
                peers.remove(&self.peer),
                "a locked transaction is tracked for its peer"
            );
            if peers.is_empty() {
                state.fetching_peers.remove(tx_ref);
            }
        }
        *state
            .active_requests
            .get_mut(&(self.peer, self.sync_method))
            .expect("a locked request is counted for its peer") -= 1;
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
        missing_transaction_refs: BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
    ) -> ConsensusResult<()> {
        let missing_transaction_refs = transaction_refs(missing_transaction_refs)?;
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

/// The `BlockRef` arm exists only for `CommitV1`, which no longer reaches the
/// transaction-sync paths, so the synchronizer works with `TransactionRef`.
fn transaction_refs(
    missing_transaction_refs: BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
) -> ConsensusResult<BTreeMap<TransactionRef, BTreeSet<AuthorityIndex>>> {
    missing_transaction_refs
        .into_iter()
        .map(|(tx_ref, authorities)| Ok((tx_ref.expect_transaction_ref()?, authorities)))
        .collect()
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
/// 2. Periodically requesting from the core the transactions that are still missing and fetching
///    them via a scheduler. This retrieves the missing transactions that were not fetched via the
///    live synchronization.
pub(crate) struct TransactionsSynchronizer<C: NetworkClient, D: CoreThreadDispatcher> {
    context: Arc<Context>,
    network_client: Arc<C>,
    core_dispatcher: Arc<D>,
    dag_state: Arc<RwLock<DagState>>,
    /// Applies the same transaction limit and batch verification checks to
    /// fetched payloads as the direct block-bundle route.
    block_verifier: Arc<dyn BlockVerifier>,
    /// Charges faults for fetched payloads that fail verification: the author,
    /// when the payload is provably theirs, and the peer that served it.
    misbehavior_store: Arc<MisbehaviorStore>,
    inflight_transactions_map: Arc<InflightTransactionsMap>,
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
        let misbehavior_store = dag_state.read().misbehavior_store().clone();
        let synchronizer = Arc::new(Self {
            context,
            network_client,
            core_dispatcher,
            dag_state,
            block_verifier,
            misbehavior_store,
            inflight_transactions_map: Arc::default(),
        });

        let (live_fetch_sender, live_fetch_receiver) = channel(
            "consensus_transactions_synchronizer_live_fetches",
            LIVE_FETCH_TRANSACTIONS_CONCURRENCY,
        );
        let mut tasks = JoinSet::new();
        let live_fetcher = synchronizer.clone().live_fetcher(live_fetch_receiver);
        tasks.spawn(monitored_future!(live_fetcher));
        tasks.spawn(monitored_future!(synchronizer.run()));

        Arc::new(TransactionsSynchronizerHandle {
            live_fetch_requests: live_fetch_sender,
            tasks: tokio::sync::Mutex::new(tasks),
        })
    }

    /// Runs the periodic scheduler: on every tick, while fewer than
    /// `PERIODIC_FETCH_TRANSACTIONS_CONCURRENCY` periodic fetches are
    /// running, asks the core for the missing transactions and fetches them.
    /// Returns when the core shuts down.
    #[cfg_attr(test, tracing::instrument(skip_all, name = "", fields(authority = %self.context.own_index
    )))]
    async fn run(self: Arc<Self>) {
        let mut tasks = JoinSet::new();
        let scheduler_timeout = sleep_until(Instant::now() + TRANSACTIONS_SYNCHRONIZER_TIMEOUT);
        tokio::pin!(scheduler_timeout);

        loop {
            tokio::select! {
                Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                    resume_if_panicked(result);
                },
                () = &mut scheduler_timeout => {
                    let mut next_tick = TRANSACTIONS_SYNCHRONIZER_TIMEOUT;
                    if tasks.len() < PERIODIC_FETCH_TRANSACTIONS_CONCURRENCY {
                        let missing_transactions = match self.missing_transactions().await {
                            Ok(missing_transactions) => missing_transactions,
                            Err(err) => {
                                debug!("Core is shutting down, transactions synchronizer is shutting down: {err:?}");
                                return;
                            }
                        };
                        if !missing_transactions.is_empty() {
                            // Retry sooner while there is something to fetch.
                            next_tick /= 2;
                            let synchronizer = self.clone();
                            tasks.spawn(monitored_future!(async move {
                                let _scope = monitored_scope("FetchMissingTransactionsScheduler");
                                fail_point_async!("consensus-delay");
                                let _inflight = GaugeGuard::acquire(
                                    &synchronizer
                                        .context
                                        .metrics
                                        .node_metrics
                                        .transactions_synchronizer_periodic_inflight,
                                );
                                synchronizer
                                    .fetch_from_authorities(missing_transactions, SyncMethod::Periodic)
                                    .await;
                            }));
                        }
                    }
                    scheduler_timeout.as_mut().reset(Instant::now() + next_tick);
                }
            }
        }
    }

    /// Serves the live fetch requests, at most
    /// `LIVE_FETCH_TRANSACTIONS_CONCURRENCY` at a time. Returns once the
    /// handle is dropped.
    async fn live_fetcher(
        self: Arc<Self>,
        mut receiver: Receiver<BTreeMap<TransactionRef, BTreeSet<AuthorityIndex>>>,
    ) {
        let mut tasks = JoinSet::new();
        loop {
            tokio::select! {
                request = receiver.recv(), if tasks.len() < LIVE_FETCH_TRANSACTIONS_CONCURRENCY => {
                    let Some(missing_transactions) = request else {
                        info!("Live fetcher task will now abort.");
                        return;
                    };
                    let synchronizer = self.clone();
                    tasks.spawn(async move {
                        synchronizer
                            .fetch_from_authorities(missing_transactions, SyncMethod::Live)
                            .await;
                    });
                },
                Some(result) = tasks.join_next(), if !tasks.is_empty() => {
                    resume_if_panicked(result);
                },
            }
        }
    }

    /// Asks the core for the missing transactions and publishes the gap to
    /// the earliest unavailable one and the per-authority counts, so they
    /// read zero once nothing is missing. Fails only when the core is
    /// shutting down; a ref of the wrong kind skips this tick.
    async fn missing_transactions(
        &self,
    ) -> ConsensusResult<BTreeMap<TransactionRef, BTreeSet<AuthorityIndex>>> {
        let missing_transactions = self
            .core_dispatcher
            .get_missing_transaction_data()
            .await
            .map_err(|_err| ConsensusError::Shutdown)?;
        let missing_transactions = match transaction_refs(missing_transactions) {
            Ok(missing_transactions) => missing_transactions,
            Err(err) => {
                warn!("Skipping the periodic transaction fetch: {err}");
                return Ok(BTreeMap::new());
            }
        };

        let metrics = &self.context.metrics.node_metrics;
        let accepted_round = self.dag_state.read().highest_accepted_round();
        let earliest_unavailable_round = missing_transactions
            .first_key_value()
            .map(|(tx_ref, _)| tx_ref.round)
            .unwrap_or(accepted_round);
        metrics
            .gap_to_unavailable_transactions
            .set(accepted_round.saturating_sub(earliest_unavailable_round) as i64);

        let mut missing_per_authority = vec![0u64; self.context.committee.size()];
        for tx_ref in missing_transactions.keys() {
            missing_per_authority[tx_ref.author] += 1;
        }
        for (missing, (_, authority)) in missing_per_authority
            .into_iter()
            .zip(self.context.committee.authorities())
        {
            let hostname = authority.hostname.as_str();
            if missing > 0 {
                metrics
                    .transactions_synchronizer_missing_transactions_by_authority
                    .with_label_values(&[hostname])
                    .inc_by(missing);
            }
            metrics
                .transactions_synchronizer_current_missing_transactions_by_authority
                .with_label_values(&[hostname])
                .set(missing as i64);
        }
        Ok(missing_transactions)
    }

    /// Fetches the missing transactions from the authorities that acknowledged
    /// them. Up to `MAX_ASSIGNED_AUTHORITIES_PER_TRANSACTION_FETCH` peers are
    /// asked concurrently, each for the transactions it could lock; a
    /// transaction another peer is already fetching is skipped. Every response
    /// feeds the peer's responsiveness ranking.
    async fn fetch_from_authorities(
        self: &Arc<Self>,
        missing_transactions: BTreeMap<TransactionRef, BTreeSet<AuthorityIndex>>,
        sync_method: SyncMethod,
    ) {
        let context = &self.context;
        let mut transaction_refs_by_authority: BTreeMap<AuthorityIndex, BTreeSet<TransactionRef>> =
            BTreeMap::new();
        for (tx_ref, authorities) in &missing_transactions {
            for authority in authorities {
                if *authority != context.own_index {
                    transaction_refs_by_authority
                        .entry(*authority)
                        .or_default()
                        .insert(*tx_ref);
                }
            }
        }

        // Responsive acknowledgers are tried first when ranking is enabled;
        // a peer whose last fetch failed is ordered behind the healthy
        // candidates rather than dropped. Without ranking the order is
        // uniform.
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
            order.shuffle(&mut rng);
        }

        let mut request_futures = FuturesUnordered::new();
        for authority in order {
            let authority_transaction_refs = transaction_refs_by_authority
                .remove(&authority)
                .expect("the order is a permutation of the candidate set");
            let Some(transactions_guard) = self.inflight_transactions_map.lock_transactions(
                authority_transaction_refs,
                authority,
                context
                    .parameters
                    .max_transactions_per_transaction_sync_fetch,
                sync_method,
            ) else {
                continue;
            };
            request_futures.push(async move {
                let result = self
                    .fetch_from_authority(authority, transactions_guard, sync_method)
                    .await;
                (authority, result)
            });
            if request_futures.len() == MAX_ASSIGNED_AUTHORITIES_PER_TRANSACTION_FETCH {
                break;
            }
        }

        // The recorded latency spans fetch and verification. A successful
        // fetch records that latency scaled up by the fraction of requested
        // transactions it returned, so unrelated or partial responses cannot
        // improve a peer's rank; the scaled sample is capped at the failure
        // penalty, so a partial delivery never records worse than a failed
        // fetch. An empty response, an error or a timeout demote the peer to
        // a timeout-scale latency, ordering it behind healthy peers in later
        // selections without dropping it from the set.
        while let Some((peer, result)) = request_futures.next().await {
            match result {
                Ok(stats) if stats.matched_requested > 0 => {
                    let latency = stats
                        .latency
                        .mul_f64(shortfall_factor(stats.requested, stats.matched_requested))
                        .min(FETCH_REQUEST_TIMEOUT);
                    context.peer_responsiveness.record_success(
                        DataSource::TransactionSynchronizer,
                        peer,
                        latency,
                    );
                }
                other => {
                    context.peer_responsiveness.record_failure_with_timeout(
                        DataSource::TransactionSynchronizer,
                        peer,
                        FETCH_REQUEST_TIMEOUT,
                    );
                    if let Err(err) = other {
                        warn!(
                            "[{}] Error when fetching and processing transactions from authority {peer}: {err}",
                            sync_method.as_str(),
                        );
                    }
                }
            }
        }
    }

    /// Fetches the locked transactions from `peer`, verifies them and hands
    /// them to the core. The reported latency spans fetch and verification,
    /// consistent with the commit syncer.
    async fn fetch_from_authority(
        self: &Arc<Self>,
        peer: AuthorityIndex,
        transactions_guard: TransactionsGuard,
        sync_method: SyncMethod,
    ) -> ConsensusResult<FetchStats> {
        let peer_hostname = &self.context.committee.authority(peer).hostname;
        let requested = transactions_guard.transactions_refs.len();
        debug!(
            "[{}] Syncing {requested} missing committed transactions from authority {peer} {peer_hostname}",
            sync_method.as_str(),
        );

        let started = Instant::now();
        let serialized_transactions = self
            .fetch_transactions_request(peer, &transactions_guard.transactions_refs, sync_method)
            .await?;
        debug!(
            "Transactions from {requested} blocks requested, fetched from {} blocks",
            serialized_transactions.len()
        );
        let matched_requested = self
            .process_fetched_transactions(
                serialized_transactions,
                peer,
                transactions_guard,
                sync_method,
            )
            .await?;

        Ok(FetchStats {
            latency: started.elapsed(),
            requested,
            matched_requested,
        })
    }

    /// Requests the transactions from `peer` and records the outcome.
    async fn fetch_transactions_request(
        &self,
        peer: AuthorityIndex,
        transactions_refs: &BTreeSet<TransactionRef>,
        sync_method: SyncMethod,
    ) -> ConsensusResult<Vec<Bytes>> {
        let metrics = &self.context.metrics.node_metrics;
        let _inflight = GaugeGuard::acquire(&metrics.transactions_synchronizer_inflight_requests);
        let peer_hostname = self.context.committee.authority(peer).hostname.as_str();

        let start_time = Instant::now();
        let result = timeout(
            FETCH_REQUEST_TIMEOUT,
            self.network_client.fetch_transactions(
                peer,
                transactions_refs.iter().copied().collect(),
                FETCH_REQUEST_TIMEOUT,
            ),
        )
        .await;

        fail_point_async!("consensus-delay");

        let fetch_duration = start_time.elapsed().as_secs_f64();
        metrics
            .transactions_synchronizer_fetch_latency
            .observe(fetch_duration);
        metrics
            .transactions_synchronizer_fetch_latency_by_peer
            .with_label_values(&[peer_hostname, sync_method.as_str()])
            .observe(fetch_duration);

        let record_failure = |reason: &str| {
            metrics
                .transactions_synchronizer_failure_by_peer
                .with_label_values(&[peer_hostname, sync_method.as_str(), reason])
                .inc();
        };
        match result {
            Ok(Ok(serialized_transactions)) => {
                metrics
                    .transactions_synchronizer_success_by_peer
                    .with_label_values(&[peer_hostname, sync_method.as_str()])
                    .inc();
                Ok(serialized_transactions)
            }
            Ok(Err(err)) => {
                self.misbehavior_store.record_fetch_fault(peer, &err);
                record_failure(err.name());
                Err(err)
            }
            Err(elapsed) => {
                record_failure("timeout");
                Err(ConsensusError::NetworkRequestTimeout(elapsed.to_string()))
            }
        }
    }

    /// Verifies the payloads `peer` served and hands them to the core; the
    /// locked transactions are released only once the core accepted them.
    /// Returns how many requested transactions were delivered, or an error if
    /// the response contains a transaction that was not requested.
    async fn process_fetched_transactions(
        self: &Arc<Self>,
        serialized_transactions_vec: Vec<Bytes>,
        peer: AuthorityIndex,
        transactions_guard: TransactionsGuard,
        sync_method: SyncMethod,
    ) -> ConsensusResult<usize> {
        let metrics = &self.context.metrics.node_metrics;
        let _timer = metrics
            .scope_processing_time
            .with_label_values(&["Synchronizer::process_fetched_transactions"])
            .start_timer();
        // Ensure that all the returned transactions do not go over the total max
        // allowed returned transactions
        if serialized_transactions_vec.len() > transactions_guard.transactions_refs.len() {
            self.misbehavior_store
                .record_faulty_transactions(peer, false, [peer]);
            return Err(ConsensusError::TooManyFetchedTransactionsReturned(peer));
        }

        // Deserialization, commitment checks and the transaction batch
        // verification run on the blocking pool.
        let (transactions, transactions_guard) = spawn_blocking({
            let synchronizer = self.clone();
            move || {
                let transactions = synchronizer.verify_fetched_transactions(
                    serialized_transactions_vec,
                    &transactions_guard.transactions_refs,
                    peer,
                );
                (transactions, transactions_guard)
            }
        })
        .await?;
        let transactions = transactions?;

        let peer_hostname = self.context.committee.authority(peer).hostname.as_str();
        metrics
            .transactions_synchronizer_fetched_transactions_by_peer
            .with_label_values(&[peer_hostname, sync_method.as_str()])
            .inc_by(transactions.len() as u64);
        for transactions in &transactions {
            let block_hostname = &self
                .context
                .committee
                .authority(transactions.author())
                .hostname;
            metrics
                .transactions_synchronizer_fetched_transactions_by_authority
                .with_label_values(&[block_hostname.as_str(), sync_method.as_str()])
                .inc();
        }

        let matched_requested = transactions.len();
        self.core_dispatcher
            .add_transactions(transactions, DataSource::TransactionSynchronizer)
            .await
            .map_err(|_| ConsensusError::Shutdown)?;
        drop(transactions_guard);

        Ok(matched_requested)
    }

    /// Deserializes the fetched payloads, checks each against the commitment
    /// in its transaction reference and runs the same validity checks as the
    /// block-bundle route. Records metrics and misbehavior for a failure.
    fn verify_fetched_transactions(
        &self,
        serialized_transactions_vec: Vec<Bytes>,
        requested_transactions_refs: &BTreeSet<TransactionRef>,
        peer_index: AuthorityIndex,
    ) -> ConsensusResult<Vec<CommitmentVerifiedTransactions>> {
        let context = &self.context;
        let misbehavior_store = &self.misbehavior_store;
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
            if let Err(err) = self
                .block_verifier
                .verify_transactions_validity(verified_transactions)
            {
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

/// Re-raises a panic from a fetch task; a cancelled task is expected on
/// shutdown.
fn resume_if_panicked(result: Result<(), JoinError>) {
    if let Err(err) = result {
        if err.is_panic() {
            std::panic::resume_unwind(err.into_panic());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc, time::Duration};

    use async_trait::async_trait;
    use bytes::Bytes;
    use rand::{RngExt, rng};
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
        network::{BlockBundleStream, NetworkClient},
        storage::mem_store::MemStore,
        transaction_ref::TransactionRef,
    };

    /// How long a test waits for an expected delivery.
    const WAIT_DEADLINE: Duration = Duration::from_secs(5);

    /// A payload together with the header committing to it.
    struct Payload {
        header: VerifiedBlockHeader,
        transactions: CommitmentVerifiedTransactions,
    }

    impl Payload {
        fn transaction_ref(&self) -> GenericTransactionRef {
            GenericTransactionRef::from(self.header.transaction_ref())
        }
    }

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

        /// Payloads of one random 32-byte transaction per `(round, author)`.
        fn payloads(&self, blocks: &[(Round, u8)]) -> Vec<Payload> {
            let mut rng = rng();
            self.payloads_with(blocks.iter().map(|&(round, author)| {
                let transaction = Transaction::new((0..32).map(|_| rng.random()).collect());
                (round, author, vec![transaction])
            }))
        }

        fn payloads_with(
            &self,
            blocks: impl IntoIterator<Item = (Round, u8, Vec<Transaction>)>,
        ) -> Vec<Payload> {
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
                    let serialized = Transaction::serialize(&transactions).unwrap();
                    let transactions = CommitmentVerifiedTransactions::new(
                        transactions,
                        header.transaction_ref(),
                        Some(header.digest()),
                        serialized,
                    );
                    Payload {
                        header,
                        transactions,
                    }
                })
                .collect()
        }

        fn accept_headers(&self, payloads: &[Payload]) {
            let headers = payloads.iter().map(|p| p.header.clone()).collect();
            self.dag_state
                .write()
                .accept_block_headers(headers, DataSource::Test);
        }

        fn misbehavior_counts(&self, authority: u8) -> (u64, u64) {
            let counts = self.dag_state.read().misbehavior_store().snapshot_totals();
            let counts = counts[AuthorityIndex::new_for_test(authority).value()].as_v2();
            (
                counts.faulty_blocks_provable,
                counts.faulty_blocks_unprovable,
            )
        }

        /// Waits for `expected` transactions to reach the core.
        async fn wait_for_fetched(&self, expected: usize) -> Vec<CommitmentVerifiedTransactions> {
            wait_until(|| self.core_dispatcher.fetched_count() >= expected).await;
            self.core_dispatcher.fetched_transactions()
        }

        async fn stop(self) {
            self.handle.stop().await.unwrap();
        }
    }

    /// Polls `condition` every 10 ms until it holds; panics at the deadline.
    async fn wait_until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + WAIT_DEADLINE;
        while !condition() {
            assert!(
                Instant::now() < deadline,
                "condition not met within {WAIT_DEADLINE:?}"
            );
            sleep(Duration::from_millis(10)).await;
        }
    }

    /// The missing map naming `peers` as the acknowledgers of every payload.
    fn missing(
        payloads: &[Payload],
        peers: &[u8],
    ) -> BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>> {
        let peers: BTreeSet<_> = peers
            .iter()
            .map(|&peer| AuthorityIndex::new_for_test(peer))
            .collect();
        payloads
            .iter()
            .map(|payload| (payload.transaction_ref(), peers.clone()))
            .collect()
    }

    fn assert_all_fetched(fetched: &[CommitmentVerifiedTransactions], payloads: &[Payload]) {
        assert_eq!(fetched.len(), payloads.len());
        for payload in payloads {
            assert!(
                fetched
                    .iter()
                    .any(|t| t.transaction_ref() == payload.header.transaction_ref()),
                "payload {:?} was not fetched",
                payload.header.reference()
            );
        }
    }

    /// Peer 1 behaves as `behavior` and peer 2 serves; every payload is still
    /// fetched. Returns the fixture for further assertions.
    async fn fetches_despite_bad_peer(behavior: PeerBehavior, ranking: bool) -> Fixture {
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.enable_peer_responsiveness_ranking = ranking;
        let fixture = Fixture::start(Arc::new(context), Arc::new(NoopBlockVerifier));
        let payloads = fixture.payloads(&[(1, 0), (2, 1), (3, 2)]);
        let missing = missing(&payloads, &[1, 2]);
        fixture.network_client.set_behavior(1, behavior);
        fixture.network_client.serve(2, &payloads);
        fixture
            .core_dispatcher
            .stub_missing_transactions(missing.clone());
        fixture.accept_headers(&payloads);

        fixture.handle.fetch_transactions(missing).unwrap();

        let fetched = fixture.wait_for_fetched(payloads.len()).await;
        assert_all_fetched(&fetched, &payloads);
        fixture
    }

    #[tokio::test]
    async fn successful_live_syncing() {
        let fixture = Fixture::new(4);
        let payloads = fixture.payloads(&[(1, 1), (2, 1), (3, 2)]);
        fixture.network_client.serve(1, &payloads);
        fixture.accept_headers(&payloads);

        fixture
            .handle
            .fetch_transactions(missing(&payloads, &[1, 2]))
            .unwrap();

        let fetched = fixture.wait_for_fetched(payloads.len()).await;
        assert_all_fetched(&fetched, &payloads);
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
        fixture.accept_headers(&payloads);

        // WHEN
        fixture
            .handle
            .fetch_transactions(missing(&payloads, &[author]))
            .unwrap();
        wait_until(|| fixture.misbehavior_counts(author).0 >= 1).await;

        // THEN the author is charged for the provably invalid payload, not
        // separately as the peer, and the payload never reaches Core, so it
        // can never be acknowledged.
        assert_eq!(fixture.misbehavior_counts(author), (1, 0));
        assert!(fixture.core_dispatcher.fetched_transactions().is_empty());
        fixture.stop().await;
    }

    #[tokio::test]
    async fn live_syncing_charges_serving_peer_for_too_many_transactions() {
        // GIVEN a synchronizer requesting a single transaction from peer 1.
        // The payload is never deserialized: the wrong-length guard fires
        // first, so the header only needs to mint a requested ref.
        let fixture = Fixture::new(4);
        let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(1, 2).build());
        let peer = 1;
        fixture.network_client.set_behavior(
            peer,
            PeerBehavior::Fixed(vec![Bytes::from(vec![0u8; 4]), Bytes::from(vec![0u8; 4])]),
        );
        let missing = BTreeMap::from([(
            GenericTransactionRef::from(header.transaction_ref()),
            BTreeSet::from([AuthorityIndex::new_for_test(peer)]),
        )]);

        // WHEN the peer returns more transactions than requested.
        fixture.handle.fetch_transactions(missing).unwrap();
        wait_until(|| fixture.misbehavior_counts(peer).1 == 1).await;

        // THEN nothing reaches the core.
        assert!(fixture.core_dispatcher.fetched_transactions().is_empty());
        fixture.stop().await;
    }

    #[tokio::test]
    async fn live_syncing_with_saturated_tasks() {
        // GIVEN one timing-out peer per request, so every live fetch holds
        // its slot for the whole request timeout.
        let requests = LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 3;
        let fixture = Fixture::new(requests);
        let missing_per_request: Vec<_> = (1..=requests)
            .map(|i| {
                let header =
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(i as Round, 1).build());
                let peer = i as u8;
                fixture
                    .network_client
                    .set_behavior(peer, PeerBehavior::Timeout);
                BTreeMap::from([(
                    GenericTransactionRef::from(header.transaction_ref()),
                    BTreeSet::from([AuthorityIndex::new_for_test(peer)]),
                )])
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

        let timed_out_peer = AuthorityIndex::new_for_test(1);
        let timeouts = fixture
            .context
            .metrics
            .node_metrics
            .transactions_synchronizer_failure_by_peer
            .with_label_values(&[
                fixture
                    .context
                    .committee
                    .authority(timed_out_peer)
                    .hostname
                    .as_str(),
                SyncMethod::Live.as_str(),
                "timeout",
            ]);
        wait_until(|| timeouts.get() == 1).await;
        let latency = fixture
            .context
            .peer_responsiveness
            .effective_latency_ms(DataSource::TransactionSynchronizer, timed_out_peer)
            .expect("the timed-out peer has a recorded latency");
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
        wait_until(|| fixture.misbehavior_counts(1).1 == 1).await;
        fixture.stop().await;
    }

    /// Both peers are fed back, and the error peer ranks slower than the peer
    /// that delivered.
    #[tokio::test]
    async fn responsiveness_feedback_records_success_and_failure() {
        let error = ConsensusError::NetworkRequest("boom".to_string());
        let fixture = fetches_despite_bad_peer(PeerBehavior::Error(error), true).await;

        let latency = |peer: u8| {
            fixture.context.peer_responsiveness.effective_latency_ms(
                DataSource::TransactionSynchronizer,
                AuthorityIndex::new_for_test(peer),
            )
        };
        wait_until(|| latency(1).is_some() && latency(2).is_some()).await;
        let (failure, success) = (latency(1).unwrap(), latency(2).unwrap());
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
        fixture.network_client.serve(peer, &payloads[..1]);
        fixture.network_client.serve_unrequested(peer, &unrequested);
        fixture.accept_headers(&payloads);

        // WHEN the peer returns the unrequested payload with a requested one.
        fixture
            .handle
            .fetch_transactions(missing(&payloads, &[peer]))
            .unwrap();
        wait_until(|| fixture.misbehavior_counts(peer).1 == 1).await;

        // THEN nothing reaches the core and the named author is not blamed.
        assert!(fixture.core_dispatcher.fetched_transactions().is_empty());
        assert_eq!(fixture.misbehavior_counts(3), (0, 0));
        fixture.stop().await;
    }

    #[tokio::test]
    async fn live_syncing_with_all_peers_failing() {
        let fixture = Fixture::new(4);
        let payloads = fixture.payloads(&[(1, 0), (2, 1), (3, 2)]);
        let missing = missing(&payloads, &[1, 2]);
        fixture
            .network_client
            .set_behavior(1, PeerBehavior::Timeout);
        fixture.network_client.set_behavior(
            2,
            PeerBehavior::Error(ConsensusError::NetworkRequest("Test error".to_string())),
        );
        fixture
            .core_dispatcher
            .stub_missing_transactions(missing.clone());
        fixture.accept_headers(&payloads);

        fixture.handle.fetch_transactions(missing).unwrap();
        sleep(Duration::from_millis(100)).await;

        assert!(fixture.core_dispatcher.fetched_transactions().is_empty());
        fixture.stop().await;
    }

    /// A live fetch must not wait for the periodic core query, which can
    /// take as long as the core thread's queue.
    #[tokio::test]
    async fn live_fetch_proceeds_while_periodic_core_query_is_blocked() {
        let fixture = Fixture::new(4);
        fixture.core_dispatcher.block_missing_transactions_queries();
        fixture
            .core_dispatcher
            .missing_transactions_query_entered()
            .await;
        let payloads = fixture.payloads(&[(1, 1)]);
        fixture.network_client.serve(2, &payloads);
        fixture.accept_headers(&payloads);

        fixture
            .handle
            .fetch_transactions(missing(&payloads, &[2]))
            .unwrap();

        let fetched = fixture.wait_for_fetched(payloads.len()).await;
        assert_all_fetched(&fetched, &payloads);
        fixture.stop().await;
    }

    /// The per-authority missing gauge is reset once the core reports
    /// nothing missing, not left at its last non-zero value.
    #[tokio::test]
    async fn periodic_fetch_clears_the_missing_transactions_gauge() {
        let fixture = Fixture::new(4);
        let author = 1;
        let payloads = fixture.payloads(&[(1, author), (2, author)]);
        let missing = missing(&payloads, &[2]);
        fixture.network_client.serve(2, &payloads);
        fixture.core_dispatcher.stub_missing_transactions(missing);
        fixture.accept_headers(&payloads);
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

        // The scheduler reports the payloads missing and fetches them; the
        // next tick finds nothing missing.
        wait_until(|| gauge.get() == payloads.len() as i64).await;
        let fetched = fixture.wait_for_fetched(payloads.len()).await;
        assert_all_fetched(&fetched, &payloads);
        wait_until(|| gauge.get() == 0).await;
        fixture.stop().await;
    }

    #[tokio::test]
    async fn inflight_transactions_map_with_active_requests() {
        telemetry_subscribers::init_for_testing();

        // GIVEN
        let map = Arc::new(InflightTransactionsMap::default());
        let sync_method = SyncMethod::Periodic;

        let context = Context::new_for_test(10).0;
        let max_transactions = context
            .parameters
            .max_transactions_per_transaction_sync_fetch;
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
        // We keep the guards so that drops happen at the end
        let mut all_guards: Vec<TransactionsGuard> = Vec::new();

        // Try to acquire the transaction locks for authorities
        // 0..MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION
        for i in 0..=MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION {
            let authority = AuthorityIndex::new_for_test(i as u8);

            let guard = map.lock_transactions(
                missing_transactions_refs.clone(),
                authority,
                max_transactions,
                sync_method,
            );

            if i == MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION {
                // Trying to acquire for authority MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION will
                // fail - as we have maxed out the number of allowed peers for
                // each transaction
                assert!(guard.is_none());
                break;
            }
            let guard = guard.expect("Guard should be created");
            assert_eq!(guard.transactions_refs.len(), 4);

            all_guards.push(guard);

            // trying to acquire any of them again for the *same* authority should not
            // succeed
            let guard = map.lock_transactions(
                missing_transactions_refs.clone(),
                authority,
                max_transactions,
                sync_method,
            );
            assert!(guard.is_none());
        }

        // Explicitly drop the guard of authority 0 (the first we stored) and try for
        // authority MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION again - it will now
        // succeed because one slot per transaction got freed
        drop(all_guards.remove(0));

        let guard = map.lock_transactions(
            missing_transactions_refs.clone(),
            AuthorityIndex::new_for_test(MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION as u8),
            max_transactions,
            sync_method,
        );
        let guard = guard.expect("Guard should be successfully acquired");
        assert_eq!(guard.transactions_refs, missing_transactions_refs);

        // Dropping all guards should unlock all transaction refs
        drop(guard);
        drop(all_guards);

        assert_eq!(map.num_of_locked_transactions(), 0);
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
        /// Appended to every response from the peer.
        unrequested: Mutex<HashMap<AuthorityIndex, Vec<Bytes>>>,
        behaviors: Mutex<HashMap<AuthorityIndex, PeerBehavior>>,
    }

    impl MockNetworkClient {
        fn serve(&self, peer: u8, payloads: &[Payload]) {
            let peer = AuthorityIndex::new_for_test(peer);
            let mut served = self.served.lock();
            for payload in payloads {
                served.insert(
                    (peer, payload.header.transaction_ref()),
                    serialize(&payload.transactions),
                );
            }
        }

        fn serve_unrequested(&self, peer: u8, payloads: &[Payload]) {
            let peer = AuthorityIndex::new_for_test(peer);
            self.unrequested
                .lock()
                .entry(peer)
                .or_default()
                .extend(payloads.iter().map(|p| serialize(&p.transactions)));
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
                Some(PeerBehavior::Timeout) => {
                    sleep(Duration::from_secs(10)).await;
                    return Ok(Vec::new());
                }
                Some(PeerBehavior::Error(error)) => return Err(error),
                Some(PeerBehavior::Empty) => return Ok(Vec::new()),
                Some(PeerBehavior::Corrupted) => {
                    return Ok(vec![Bytes::from(vec![0, 1, 2, 3]); transaction_refs.len()]);
                }
                Some(PeerBehavior::Fixed(response)) => return Ok(response),
                None => {}
            }

            let served = self.served.lock();
            let mut result: Vec<Bytes> = transaction_refs
                .iter()
                .filter_map(|transaction_ref| served.get(&(peer, *transaction_ref)).cloned())
                .collect();
            if let Some(unrequested) = self.unrequested.lock().get(&peer) {
                result.extend(unrequested.iter().cloned());
            }
            Ok(result)
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
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>, Option<ConsensusError>)> {
            unimplemented!("fetch_commits_and_transactions not implemented in mock")
        }
    }
}
