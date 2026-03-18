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
use itertools::Itertools as _;
use parking_lot::{Mutex, RwLock};
use rand::{
    SeedableRng,
    rngs::{OsRng, StdRng},
    seq::SliceRandom,
};
use starfish_config::AuthorityIndex;
use tokio::{
    runtime::Handle,
    sync::{Semaphore, mpsc::error::TrySendError, oneshot},
    task::{JoinError, JoinSet},
    time::{Instant, sleep_until, timeout},
};
use tracing::{debug, info, warn};

use crate::{
    block_header::BlockRef,
    commit_syncer::{verify_transactions_with_headers, verify_transactions_with_transactions_refs},
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::{DagState, DataSource},
    error::{ConsensusError, ConsensusResult},
    network::{NetworkClient, SerializedTransactionsV1, SerializedTransactionsV2},
    transaction_ref::{GenericTransactionRef, GenericTransactionRefAPI as _},
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
const FETCH_REQUEST_TIMEOUT: Duration = Duration::from_millis(2000);

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

/// Records when the transaction synchronizer failed for the last time when
/// fetching from peers.
struct LastFailureByPeer {
    inner: Mutex<Vec<Option<Instant>>>,
    context: Context,
}

impl LastFailureByPeer {
    fn new(context: &Context) -> Arc<Self> {
        let committee_size = context.committee.size();
        Arc::new(Self {
            inner: Mutex::new(vec![None; committee_size]),
            context: context.clone(),
        })
    }
    fn update_with_new_instant(self: &Arc<Self>, peer: AuthorityIndex, new_instant: Instant) {
        let mut inner = self.inner.lock();
        inner[peer] = Some(new_instant);
    }

    /// Determine which authorities are less reliable to fetch transactions.
    /// Returns less than f+1 authorities by stake.
    fn get_excluded_authorities_by_stake(self: &Arc<Self>) -> BTreeSet<AuthorityIndex> {
        let last_round_by_peer = { self.inner.lock().clone() };

        let mut indexed_rounds: Vec<(AuthorityIndex, Instant)> = last_round_by_peer
            .iter()
            .enumerate()
            .filter_map(|(idx, last_instant)| {
                last_instant.map(|last_instant| (AuthorityIndex::from(idx as u8), last_instant))
            })
            .collect();

        indexed_rounds.sort_by_key(|&(_, instant)| std::cmp::Reverse(instant));

        let mut excluded_authorities = BTreeSet::new();
        let mut stake = 0;
        for (authority_index, _last_instant) in indexed_rounds {
            stake += self.context.committee.stake(authority_index);
            if self.context.committee.reached_validity(stake) {
                break;
            }
            excluded_authorities.insert(authority_index);
        }

        excluded_authorities
    }
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
    transactions_refs: BTreeSet<GenericTransactionRef>,
    peer: AuthorityIndex,
}

impl Drop for TransactionsGuard {
    fn drop(&mut self) {
        self.map
            .unlock_transactions(&self.transactions_refs, self.peer);
    }
}

// Keeps a mapping between the missing transactions that have been instructed to
// be fetched and the authorities that are currently fetching them. For a block
// ref there is a maximum number of authorities that can concurrently fetch it.
// The authority ids that are currently fetching a transaction are set on the
// corresponding `BTreeSet` and basically they act as "locks".
struct InflightTransactionsMap {
    inner: Mutex<HashMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>>,
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
    /// transaction by attempting to lock per block_ref. In addition, we check
    /// whether a given `peer` has many concurrent requests. If so, we will
    /// not lock transactions. The method return optionally two guards. One for
    /// the fetched transactions and one for active fetch request.
    fn lock_transactions_and_active_request(
        self: &Arc<Self>,
        missing_block_refs: BTreeSet<GenericTransactionRef>,
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
        let mut selected_block_refs_num = 0;

        for block_ref in missing_block_refs {
            let authorities = transaction_map.entry(block_ref).or_default();

            if authorities.len() < MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION
                && authorities.insert(peer)
            {
                selected_transactions_to_fetch.insert(block_ref);
                selected_block_refs_num += 1;

                if selected_block_refs_num >= max_number_transactions_per_fetch {
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

    /// Unlocks the provided block references for the given `peer`. The
    /// unlocking is strict, meaning that if this method is called for a
    /// specific block ref and peer more times than the corresponding lock
    /// has been called, it will panic.
    fn unlock_transactions(
        self: &Arc<Self>,
        tx_refs: &BTreeSet<GenericTransactionRef>,
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

enum Command {
    FetchTransactions {
        missing_transaction_refs: BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
        result: oneshot::Sender<Result<(), ConsensusError>>,
    },
    KickOffScheduler,
}

pub(crate) struct TransactionsSynchronizerHandle {
    commands_sender: Sender<Command>,
    tasks: tokio::sync::Mutex<JoinSet<()>>,
}

impl TransactionsSynchronizerHandle {
    /// Explicitly asks from the transactions synchronizer to fetch the
    /// transactions - provided the block_refs set - from the peer
    /// authority.
    pub(crate) async fn fetch_transactions(
        &self,
        missing_block_refs: BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
    ) -> ConsensusResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.commands_sender
            .send(Command::FetchTransactions {
                missing_transaction_refs: missing_block_refs,
                result: sender,
            })
            .await
            .map_err(|_| ConsensusError::Shutdown)?;
        receiver.await.map_err(|_| ConsensusError::Shutdown)?
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
/// 1. Explicitly requesting missing transactions from authorities that have
///    acknowledged them in their blocks that were committed. A locking
///    mechanism allows concurrent requests for missing transactions from a
///    limited number of authorities simultaneously, enhancing the chances of
///    timely retrieval.
///
/// 2. Periodically requesting missing transactions via a scheduler. This
///    primarily serves to retrieve missing transactions that were not fetched
///    via the live synchronization. The scheduler operates on either a fixed
///    periodic basis or is triggered immediately after explicit fetches
///    described in (1), ensuring continued transaction retrieval if gaps
///    persist.
pub(crate) struct TransactionsSynchronizer<C: NetworkClient, D: CoreThreadDispatcher> {
    context: Arc<Context>,
    commands_receiver: Receiver<Command>,
    live_fetch_requests: Sender<BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>>,
    core_dispatcher: Arc<D>,
    dag_state: Arc<RwLock<DagState>>,
    active_requests: Arc<InflightActiveRequests>,
    fetch_transactions_scheduler_task: JoinSet<()>,
    network_client: Arc<C>,
    inflight_transactions_map: Arc<InflightTransactionsMap>,
    commands_sender: Sender<Command>,
    last_failure_by_peer: Arc<LastFailureByPeer>,
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
    ) -> Arc<TransactionsSynchronizerHandle> {
        let (commands_sender, commands_receiver) =
            channel("consensus_transactions_synchronizer_commands", 1_000);
        let inflight_transactions_map = InflightTransactionsMap::new();

        // Create a channel for live fetch requests
        let (live_fetch_sender, live_fetch_receiver) = channel(
            "consensus_transactions_synchronizer_live_fetches",
            LIVE_FETCH_TRANSACTIONS_CONCURRENCY,
        );

        let mut tasks = JoinSet::new();
        let active_requests = InflightActiveRequests::new();
        let last_failure_by_peer = LastFailureByPeer::new(&context);
        // Spawn the live fetcher task
        let live_fetcher_async = Self::live_fetcher(
            active_requests.clone(),
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
            live_fetch_receiver,
            inflight_transactions_map.clone(),
            last_failure_by_peer.clone(),
        );
        tasks.spawn(monitored_future!(live_fetcher_async));

        let commands_sender_clone = commands_sender.clone();

        // Spawn the task to listen to the live requests & periodic runs
        tasks.spawn(monitored_future!(async move {
            let mut s = Self {
                context,
                commands_receiver,
                live_fetch_requests: live_fetch_sender,
                core_dispatcher,
                fetch_transactions_scheduler_task: JoinSet::new(),
                active_requests,
                network_client,
                inflight_transactions_map,
                commands_sender: commands_sender_clone,
                dag_state,
                last_failure_by_peer,
            };
            s.run().await;
        }));

        Arc::new(TransactionsSynchronizerHandle {
            commands_sender,
            tasks: tokio::sync::Mutex::new(tasks),
        })
    }

    // The main loop to listen for the submitted commands.
    #[cfg_attr(test, tracing::instrument(skip_all, name = "", fields(authority = %self.context.own_index
    )))]
    async fn run(&mut self) {
        // We want the transactions synchronizer to run periodically to
        // fetch any missing transactions.
        let scheduler_timeout = sleep_until(Instant::now() + TRANSACTIONS_SYNCHRONIZER_TIMEOUT);

        tokio::pin!(scheduler_timeout);

        loop {
            tokio::select! {
                Some(command) = self.commands_receiver.recv() => {
                    match command {
                        Command::FetchTransactions{ missing_transaction_refs, result } => {
                            // Enqueue the request to the live fetcher and return immediately.
                            let r =  self.live_fetch_requests.try_send(missing_transaction_refs)
                            .map_err(|err| {
                                match err {
                                    TrySendError::Full(_) => ConsensusError::TransactionSynchronizerSaturated,
                                    TrySendError::Closed(_) => ConsensusError::Shutdown
                                }
                            });

                            result.send(r).ok();
                        }
                        Command::KickOffScheduler => {
                            // Reset the scheduler timeout timer to run immediately if not already running.
                            // If the scheduler is already running, then reduce the remaining time to run.
                            let timeout = if self.fetch_transactions_scheduler_task.is_empty() {
                                Instant::now()
                            } else {
                                Instant::now() + TRANSACTIONS_SYNCHRONIZER_TIMEOUT.checked_div(2).unwrap()
                            };

                            // only reset if it is earlier than the next deadline
                            if timeout < scheduler_timeout.deadline() {
                                scheduler_timeout.as_mut().reset(timeout);
                            }
                        }
                    }
                },
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
                    if self.fetch_transactions_scheduler_task.len() < PERIODIC_FETCH_TRANSACTIONS_CONCURRENCY {
                        if let Err(err) = self.start_fetch_missing_transactions_task().await {
                            debug!("Core is shutting down, transactions synchronizer is shutting down: {err:?}");
                            return;
                        };
                    }

                    scheduler_timeout
                        .as_mut()
                        .reset(Instant::now() + TRANSACTIONS_SYNCHRONIZER_TIMEOUT);
                }
            }
        }
    }

    // The live fetcher task that processes fetch requests from the queue
    async fn live_fetcher(
        active_requests: Arc<InflightActiveRequests>,
        network_client: Arc<C>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        dag_state: Arc<RwLock<DagState>>,
        mut receiver: Receiver<BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>>,
        inflight_transactions_map: Arc<InflightTransactionsMap>,
        last_failure_by_peer: Arc<LastFailureByPeer>,
    ) {
        let semaphore = Arc::new(Semaphore::new(LIVE_FETCH_TRANSACTIONS_CONCURRENCY));

        loop {
            // Wait for a permit asynchronously
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .expect("We expect semaphore to be valid");

            match receiver.recv().await {
                Some(missing_transactions_block_refs) => {
                    let context = context.clone();
                    let active_requests = active_requests.clone();
                    let inflight_transactions_map = inflight_transactions_map.clone();
                    let network_client = network_client.clone();
                    let core_dispatcher = core_dispatcher.clone();
                    let dag_state = dag_state.clone();
                    let last_failure_by_peer = last_failure_by_peer.clone();
                    tokio::spawn(async move {
                        Self::fetch_and_process_transactions_from_authorities(
                            context,
                            active_requests,
                            inflight_transactions_map,
                            network_client,
                            missing_transactions_block_refs,
                            core_dispatcher,
                            dag_state,
                            last_failure_by_peer,
                            SyncMethod::Live,
                        )
                        .await;

                        // Release the permit when done
                        drop(permit);
                    });
                }
                None => {
                    // Channel closed → shutdown
                    info!("Live fetcher task will now abort.");
                    break;
                }
            }
        }
    }

    /// Starts a task to fetch missing transactions from other authorities.
    async fn start_fetch_missing_transactions_task(&mut self) -> ConsensusResult<()> {
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
            .map(|(block_ref, _)| block_ref.round())
            .unwrap_or(accepted_round);
        let gap_to_unavailable_transactions =
            accepted_round.saturating_sub(earliest_unavailable_transaction_round);
        self.context
            .metrics
            .node_metrics
            .gap_to_unavailable_transactions
            .set(gap_to_unavailable_transactions as i64);

        // If there are no missing transactions, we don't need to fetch anything.
        if missing_transactions.is_empty() {
            return Ok(());
        }

        let context = self.context.clone();

        // Update metrics for missing transactions per authority before fetching
        let mut missing_transactions_per_authority = vec![0; context.committee.size()];
        for block_ref in missing_transactions.keys() {
            missing_transactions_per_authority[block_ref.author()] += 1;
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
        let network_client = self.network_client.clone();
        let core_dispatcher = self.core_dispatcher.clone();
        let commands_sender = self.commands_sender.clone();
        let dag_state = self.dag_state.clone();
        let inflight_transactions_map = self.inflight_transactions_map.clone();
        let active_requests = self.active_requests.clone();
        let last_failure_by_round = self.last_failure_by_peer.clone();

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
                    dag_state,
                    last_failure_by_round,
                    SyncMethod::Periodic,
                )
                .await;
                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_periodic_inflight
                    .dec();
            }));
        // Kick off the scheduler to fetch any remaining missing transactions
        commands_sender
            .try_send(Command::KickOffScheduler)
            .map_err(|_| ConsensusError::Shutdown)?;
        Ok(())
    }

    /// Fetches missing transactions from authorities.
    async fn fetch_and_process_transactions_from_authorities(
        context: Arc<Context>,
        active_requests: Arc<InflightActiveRequests>,
        inflight_transactions_map: Arc<InflightTransactionsMap>,
        network_client: Arc<C>,
        missing_transactions: BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
        core_dispatcher: Arc<D>,
        dag_state: Arc<RwLock<DagState>>,
        last_failure_by_peer: Arc<LastFailureByPeer>,
        sync_method: SyncMethod,
    ) {
        // Build a mapping from authority -> set of BlockRefs it has acknowledged
        let mut blocks_by_authority: BTreeMap<AuthorityIndex, BTreeSet<GenericTransactionRef>> =
            BTreeMap::new();
        for (tx_ref, authorities) in &missing_transactions {
            for authority in authorities {
                if *authority != context.own_index {
                    blocks_by_authority
                        .entry(*authority)
                        .or_default()
                        .insert(*tx_ref);
                }
            }
        }

        // For each authority, try to lock up the
        // maximum possible amount of acknowledged transactions and fetch
        // those transactions. The logic is as follows:
        // * Iterate in random order all authorities that have acknowledged missing
        //   transactions.
        // * Attempt to lock max_transactions_per_fetch acknowledged transactions using
        //   the inflight_transactions_map. Some transactions may already be locked by
        //   other authorities, but continue with the transactions that were
        //   successfully locked.
        // * For each authority, if transactions were successfully locked, then send a
        //   request to the network client to fetch the transactions from the authority.
        // * If the transactions were successfully fetched, then process them and send
        //   them to the core for processing.
        // Each request is performed individually to avoid blocking the
        // synchronizer for too long, as certain peers may take a while to respond.
        // The number of requests to each peer is limited by the parameters.

        // Initialize randomness for shuffling authorities
        let mut rng = StdRng::from_rng(OsRng).expect("OsRng should be available");

        // Create an iterator over authorities with their corresponding block refs
        // This will allow us to iterate over authorities in a stable (for test) or
        // random order (for production).
        let iter_authorities: Box<
            dyn Iterator<Item = (AuthorityIndex, BTreeSet<GenericTransactionRef>)>,
        > = if cfg!(test) {
            // Stable order for tests
            Box::new(blocks_by_authority.into_iter())
        } else {
            // Get less than f+1 excluded authorities by stake
            let excluded_authorities = last_failure_by_peer.get_excluded_authorities_by_stake();
            // Exclude authorities with latest recorded failures
            let mut vec: Vec<_> = blocks_by_authority
                .into_iter()
                .filter(|(authority, _)| !excluded_authorities.contains(authority))
                .collect();
            vec.shuffle(&mut rng);
            Box::new(vec.into_iter())
        };

        let mut request_futures = FuturesUnordered::new();

        let mut assigned_authorities_for_transaction_fetch = 0;

        for (authority, authority_block_refs) in iter_authorities {
            // * If transactions are successfully locked, and we didn't make too many to
            //   this authority, then send a request to the network client to fetch the
            //   transactions from the authority. If the fetch is successful, then process
            //   the transactions and send them to the core for processing.
            if let Some((transactions_guard, active_request_guard)) = inflight_transactions_map
                .lock_transactions_and_active_request(
                    authority_block_refs.clone(),
                    authority,
                    context.parameters.max_transactions_per_regular_sync_fetch,
                    sync_method,
                    active_requests.clone(),
                )
            {
                let context = context.clone();
                let network_client = network_client.clone();
                let core_dispatcher = core_dispatcher.clone();
                let dag_state = dag_state.clone();
                request_futures.push(async move {
                    let result = Self::fetch_and_process_transactions_from_authority(
                        authority,
                        context,
                        transactions_guard,
                        network_client,
                        core_dispatcher,
                        dag_state,
                        sync_method,
                        active_request_guard,
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

        // Await all authority requests to complete
        while let Some((peer, result)) = request_futures.next().await {
            if let Err(err) = result {
                last_failure_by_peer.update_with_new_instant(peer, Instant::now());
                warn!(
                    "[{}] Error when fetching and processing transactions from authority {peer}: {err}",
                    sync_method.get_string(),
                );
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
        dag_state: Arc<RwLock<DagState>>,
        sync_method: SyncMethod,
        _active_guard: ActiveRequestGuard,
    ) -> ConsensusResult<()> {
        let peer_hostname = &context.committee.authority(peer).hostname;
        let total_requested = transactions_guard.transactions_refs.len();

        debug!(
            "[{}] Syncing {total_requested} missing committed transactions from authority {peer} {peer_hostname}",
            sync_method.get_string(),
        );

        let (fetched_serialized_transactions, transactions_guard, peer) =
            Self::fetch_transactions_request(
                network_client.clone(),
                peer,
                transactions_guard,
                FETCH_REQUEST_TIMEOUT,
                context.clone(),
                sync_method,
            )
            .await?;

        let total_fetched = fetched_serialized_transactions.len();

        debug!(
            "Transactions from {total_requested} blocks requested, fetched from {total_fetched} blocks"
        );

        Self::process_fetched_transactions(
            fetched_serialized_transactions,
            peer,
            transactions_guard,
            core_dispatcher.clone(),
            context,
            dag_state.clone(),
            sync_method,
        )
        .await?;

        Ok(())
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
            .collect::<Vec<GenericTransactionRef>>();

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
            .with_label_values(&[peer_hostname.as_str(), &sync_method.get_string()])
            .observe(fetch_duration.as_secs_f64());

        let resp = match result {
            Ok(Err(err)) => {
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
    /// immediately sent to Core for processing.
    async fn process_fetched_transactions(
        serialized_transactions_vec: Vec<Bytes>,
        peer_index: AuthorityIndex,
        requested_transactions_guard: TransactionsGuard,
        core_dispatcher: Arc<D>,
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
        sync_method: SyncMethod,
    ) -> ConsensusResult<()> {
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
            return Err(ConsensusError::TooManyFetchedTransactionsReturned(
                peer_index,
            ));
        }

        let metrics = &context.metrics.node_metrics;
        let peer_hostname = &context.committee.authority(peer_index).hostname;

        // Deserialize and verify the transactions
        // inside verify_transactions
        let transactions = if !context.protocol_config.consensus_fast_commit_sync() {
            match Handle::current()
                .spawn_blocking({
                    // Use the block_refs from the requested_transactions_guard
                    let block_refs: ConsensusResult<Vec<BlockRef>> = requested_transactions_guard
                        .transactions_refs
                        .iter()
                        .map(|ctr| match ctr {
                            GenericTransactionRef::TransactionRef(_) => {
                                Err(ConsensusError::TransactionRefVariantMismatch {
                                    protocol_flag_enabled: false,
                                    expected_variant: "BlockRef",
                                    received_variant: "TransactionRef",
                                })
                            }
                            GenericTransactionRef::BlockRef(block_ref) => Ok(*block_ref),
                        })
                        .collect();
                    let block_refs = block_refs?;
                    let block_headers_vec =
                        dag_state.read().get_verified_block_headers(&block_refs);
                    let mut block_headers_map = BTreeMap::new();
                    for block_header_opt in block_headers_vec.into_iter() {
                        let block_header = block_header_opt
                            .expect("block header for requested transactions must exist");
                        block_headers_map.insert(block_header.reference(), block_header);
                    }

                    let mut serialized_transactions_map: BTreeMap<GenericTransactionRef, Bytes> =
                        BTreeMap::new();
                    for serialized_transaction_bytes in &serialized_transactions_vec {
                        let serialized_transactions: SerializedTransactionsV1 =
                            bcs::from_bytes(serialized_transaction_bytes)
                                .map_err(ConsensusError::MalformedTransactions)?;
                        let committed_transaction_ref =
                            GenericTransactionRef::BlockRef(serialized_transactions.block_ref);
                        serialized_transactions_map.insert(
                            committed_transaction_ref,
                            serialized_transactions.serialized_transactions,
                        );
                    }
                    let context_cloned = context.clone();

                    move || {
                        verify_transactions_with_headers(
                            context_cloned,
                            peer_index,
                            serialized_transactions_map,
                            block_headers_map,
                        )
                    }
                })
                .await
                .expect("Spawn blocking should not fail")
            {
                Ok(transactions) => transactions,
                Err(err) => {
                    // Update metrics for invalid transactions.
                    metrics
                        .invalid_transactions
                        .with_label_values(&[
                            peer_hostname.as_str(),
                            "transaction_synchronizer",
                            err.name(),
                        ])
                        .inc();
                    return Err(err);
                }
            }
        } else {
            match Handle::current()
                .spawn_blocking({
                    // Validate that all refs are TransactionRef as expected when
                    // consensus_fast_commit_sync is true
                    for tx_ref in requested_transactions_guard.transactions_refs.iter() {
                        if let GenericTransactionRef::BlockRef(_) = tx_ref {
                            return Err(ConsensusError::TransactionRefVariantMismatch {
                                protocol_flag_enabled: true,
                                expected_variant: "TransactionRef",
                                received_variant: "BlockRef",
                            });
                        }
                    }

                    let mut serialized_transactions_map: BTreeMap<GenericTransactionRef, Bytes> =
                        BTreeMap::new();
                    for serialized_transaction_bytes in &serialized_transactions_vec {
                        let serialized_transactions: SerializedTransactionsV2 =
                            bcs::from_bytes(serialized_transaction_bytes)
                                .map_err(ConsensusError::MalformedTransactions)?;
                        let committed_transaction_ref = GenericTransactionRef::TransactionRef(
                            serialized_transactions.transaction_ref,
                        );
                        serialized_transactions_map.insert(
                            committed_transaction_ref,
                            serialized_transactions.serialized_transactions,
                        );
                    }
                    let context_cloned = context.clone();

                    move || {
                        verify_transactions_with_transactions_refs(
                            &context_cloned,
                            peer_index,
                            serialized_transactions_map,
                        )
                    }
                })
                .await
                .expect("Spawn blocking should not fail")
            {
                Ok(transactions) => transactions,
                Err(err) => {
                    // Update metrics for invalid transactions.
                    metrics
                        .invalid_transactions
                        .with_label_values(&[
                            peer_hostname.as_str(),
                            "transaction_synchronizer",
                            err.name(),
                        ])
                        .inc();
                    return Err(err);
                }
            }
        }
        .iter()
        .map(|x| x.1)
        .cloned()
        .collect::<Vec<_>>();

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

        info!(
            "[{}] Synced and processed {} missing transactions from peer {peer_index} {peer_hostname}: {}",
            sync_method.get_string(),
            transactions.len(),
            transactions
                .iter()
                .map(|b| b.transaction_ref().to_string())
                .join(", "),
        );

        // Add the transactions to the core
        core_dispatcher
            .add_transactions(transactions, DataSource::TransactionSynchronizer)
            .await
            .map_err(|_| ConsensusError::Shutdown)?;

        // now release all the locked blocks as they have been fetched, verified &
        // processed
        drop(requested_transactions_guard);

        Ok(())
    }
}

struct InflightGuard<'a> {
    metric: &'a prometheus::IntGauge,
}

impl<'a> Drop for InflightGuard<'a> {
    fn drop(&mut self) {
        self.metric.dec();
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc, time::Duration};

    use async_trait::async_trait;
    use bytes::Bytes;
    use rand::{Rng, thread_rng};
    use rstest::rstest;
    use tokio::{sync::Mutex, time::sleep};

    use super::*;
    use crate::{
        Round, TestBlockHeader, Transaction,
        block_header::{
            BlockHeaderDigest, BlockRef, TransactionsCommitment, VerifiedBlock,
            VerifiedBlockHeader, VerifiedOwnShard, VerifiedTransactions,
        },
        commit::{CertifiedCommits, CommitRange},
        context::Context,
        core::ReasonToCreateBlock,
        core_thread::CoreError,
        dag_state::{DagState, DataSource},
        encoder::create_encoder,
        network::{BlockBundleStream, NetworkClient, SerializedTransactionsV1},
        storage::mem_store::MemStore,
    };

    #[rstest]
    #[tokio::test]
    async fn successful_live_syncing(#[values(true, false)] consensus_fast_commit_sync: bool) {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        let context = Arc::new(context);
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let transaction_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        let mut encoder = create_encoder(&context);

        // Create some test transactions
        let block_round_author: Vec<(Round, u8)> = vec![(1, 1), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();
        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment = TransactionsCommitment::compute_transactions_commitment(
                    &serialized,
                    &context,
                    &mut encoder,
                )
                .unwrap();

                // Create a test block header with the correct commitment
                let header = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author)
                        .set_commitment(commitment)
                        .build(),
                );

                block_headers.push(header.clone());

                VerifiedTransactions::new(
                    transactions,
                    header.transaction_ref(),
                    Some(header.digest()),
                    serialized,
                )
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1));
            authorities.insert(AuthorityIndex::new_for_test(2));
            let gen_ref = if consensus_fast_commit_sync {
                GenericTransactionRef::from(header.transaction_ref())
            } else {
                GenericTransactionRef::from(header.reference())
            };
            missing_transactions.insert(gen_ref, authorities);
        }

        // Stub the transactions in the network client
        for transaction in &transactions {
            network_client
                .stub_fetch_transactions(
                    vec![transaction.clone()],
                    AuthorityIndex::new_for_test(1),
                    consensus_fast_commit_sync,
                )
                .await;
        }

        dag_state
            .write()
            .accept_block_headers(block_headers, DataSource::Test);

        // WHEN
        // Request the transactions
        let result = transaction_synchronizer
            .fetch_transactions(missing_transactions)
            .await;

        // THEN
        assert!(result.is_ok());

        // Wait a bit for processing to complete
        sleep(Duration::from_millis(1000)).await;

        // Verify that the transactions were added to the core
        let fetched_transactions = core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(fetched_transactions.len(), transactions.len());

        // Verify that each transaction was fetched
        for transaction in &transactions {
            assert!(
                fetched_transactions
                    .iter()
                    .any(|t| t.transactions_commitment() == transaction.transactions_commitment())
            );
        }

        // Clean up
        transaction_synchronizer.stop().await.unwrap();
    }

    #[tokio::test]
    async fn live_syncing_with_saturated_tasks() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (context, _) = Context::new_for_test(LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 3);
        let context = Arc::new(context);
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        let mut encoder = create_encoder(&context);

        // Create block round author pairs
        let block_round_authors = (1..=LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 3)
            .map(|i| (i as Round, 1u8))
            .collect::<Vec<_>>();

        let mut block_headers = Vec::with_capacity(block_round_authors.len());
        let mut verified_transactions = Vec::with_capacity(block_round_authors.len());
        let mut rng = thread_rng();

        // Create verified transactions with high latency to ensure saturation
        for (round, author) in &block_round_authors {
            // Create a dummy transaction
            let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
            let serialized_vec = bcs::to_bytes(&transactions).unwrap();
            let serialized = Bytes::from(serialized_vec);
            let commitment = TransactionsCommitment::compute_transactions_commitment(
                &serialized,
                &context,
                &mut encoder,
            )
            .unwrap();

            // Create a test block header with the correct commitment
            let header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(*round, *author)
                    .set_commitment(commitment)
                    .build(),
            );

            block_headers.push(header.clone());

            let verified_transaction = VerifiedTransactions::new(
                transactions,
                header.transaction_ref(),
                Some(header.digest()),
                serialized,
            );

            verified_transactions.push(verified_transaction);
        }

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = Vec::new();
        for (index, header) in block_headers.iter().enumerate() {
            let mut authorities = BTreeSet::new();
            let from_whom = AuthorityIndex::new_for_test(index as u8 + 1);
            authorities.insert(from_whom);
            network_client.set_timeout_peer(from_whom).await;
            let mut missing_txs = BTreeMap::new();
            missing_txs.insert(GenericTransactionRef::from(header.reference()), authorities);
            missing_transactions.push(missing_txs)
        }

        // Delay fetch transactions response to simulate saturation deterministically.

        // Add block headers to the dag state
        dag_state
            .write()
            .accept_block_headers(block_headers, DataSource::Test);

        // WHEN
        // Send many requests to saturate the tasks
        let mut results = Vec::new();
        for missing_transactions_to_request in missing_transactions
            .iter()
            .take(LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 3)
        {
            results.push(
                handle
                    .fetch_transactions(missing_transactions_to_request.clone())
                    .await,
            );
        }

        // THEN
        // LIVE_FETCH_TRANSACTIONS_CONCURRENCY tasks will start processing, another set
        // of LIVE_FETCH_TRANSACTIONS_CONCURRENCY tasks will be stuck in the
        // queue, and the last LIVE_FETCH_TRANSACTIONS_CONCURRENCY tasks will be
        // returned with TransactionSynchronizerSaturated error.
        // The test should be deterministic because the responses will timeout, so all
        // tasks should be sent to the queue before the first request is processed.
        let successes = results.iter().filter(|r| r.is_ok()).count();
        let saturated = results
            .iter()
            .filter(|r| matches!(r, Err(ConsensusError::TransactionSynchronizerSaturated)))
            .count();

        assert_eq!(
            successes,
            LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 2,
            "Expected {} requests to succeed",
            LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 2
        );
        assert_eq!(
            saturated, LIVE_FETCH_TRANSACTIONS_CONCURRENCY,
            "Expected {LIVE_FETCH_TRANSACTIONS_CONCURRENCY} requests to be saturated"
        );

        // Clean up
        handle.stop().await.unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn live_syncing_with_timeout_peer(
        #[values(true, false)] consensus_fast_commit_sync: bool,
    ) {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        let context = Arc::new(context);
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        let mut encoder = create_encoder(&context);

        // Create some test transactions
        let block_round_author: Vec<(Round, u8)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment = TransactionsCommitment::compute_transactions_commitment(
                    &serialized,
                    &context,
                    &mut encoder,
                )
                .unwrap();

                // Create a test block header with the correct commitment
                let header = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author)
                        .set_commitment(commitment)
                        .build(),
                );

                block_headers.push(header.clone());

                VerifiedTransactions::new(
                    transactions,
                    header.transaction_ref(),
                    Some(header.digest()),
                    serialized,
                )
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1)); // This peer will timeout
            authorities.insert(AuthorityIndex::new_for_test(2)); // This peer will succeed
            let gen_ref = if consensus_fast_commit_sync {
                GenericTransactionRef::from(header.transaction_ref())
            } else {
                GenericTransactionRef::from(header.reference())
            };
            missing_transactions.insert(gen_ref, authorities);
        }

        // Set peer 1 to timeout
        network_client
            .set_timeout_peer(AuthorityIndex::new_for_test(1))
            .await;

        // Stub the transactions for peer 2
        for transaction in &transactions {
            network_client
                .stub_fetch_transactions(
                    vec![transaction.clone()],
                    AuthorityIndex::new_for_test(2),
                    consensus_fast_commit_sync,
                )
                .await;
        }

        // Stub the missing transactions in the core dispatcher
        core_dispatcher
            .stub_missing_transactions(missing_transactions.clone())
            .await;

        // Add block headers to the dag state
        dag_state
            .write()
            .accept_block_headers(block_headers, DataSource::Test);

        // WHEN
        // Request the transactions
        let result = handle.fetch_transactions(missing_transactions).await;

        // THEN
        assert!(result.is_ok());

        sleep(Duration::from_millis(100)).await; // Wait shorter than the timeout to ensure the requests are still being processed.

        // Verify that the transactions were added to the core
        let fetched_transactions = core_dispatcher.get_and_drain_transactions().await;
        assert!(
            fetched_transactions.is_empty(),
            "Expected no transactions to be fetched due to timeout"
        );

        // Wait a bit for processing to complete
        sleep(Duration::from_millis(11_000)).await; // Wait longer than the timeout to ensure the request is processed.

        // Verify that the transactions were added to the core
        let fetched_transactions = core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(fetched_transactions.len(), transactions.len());

        // Verify that each transaction was fetched
        for transaction in &transactions {
            assert!(
                fetched_transactions
                    .iter()
                    .any(|t| t.transactions_commitment() == transaction.transactions_commitment())
            );
        }

        // Clean up
        handle.stop().await.unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn live_syncing_with_error_peer(#[values(true, false)] consensus_fast_commit_sync: bool) {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        let context = Arc::new(context);
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        let mut encoder = create_encoder(&context);

        // Create some test transactions
        let block_round_author: Vec<(Round, u8)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment = TransactionsCommitment::compute_transactions_commitment(
                    &serialized,
                    &context,
                    &mut encoder,
                )
                .unwrap();

                // Create a test block header with the correct commitment
                let header = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author)
                        .set_commitment(commitment)
                        .build(),
                );

                block_headers.push(header.clone());

                VerifiedTransactions::new(
                    transactions,
                    header.transaction_ref(),
                    Some(header.digest()),
                    serialized,
                )
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1)); // This peer will return an error
            authorities.insert(AuthorityIndex::new_for_test(2)); // This peer will succeed
            let gen_ref = if consensus_fast_commit_sync {
                GenericTransactionRef::from(header.transaction_ref())
            } else {
                GenericTransactionRef::from(header.reference())
            };
            missing_transactions.insert(gen_ref, authorities);
        }

        // Set peer 1 to return an error
        network_client
            .set_error_peer(
                AuthorityIndex::new_for_test(1),
                ConsensusError::NetworkRequest("Test error".to_string()),
            )
            .await;

        // Stub the transactions for peer 2
        for transaction in &transactions {
            network_client
                .stub_fetch_transactions(
                    vec![transaction.clone()],
                    AuthorityIndex::new_for_test(2),
                    consensus_fast_commit_sync,
                )
                .await;
        }

        // Add block headers to the dag state
        dag_state
            .write()
            .accept_block_headers(block_headers, DataSource::Test);

        // WHEN
        // Request the transactions
        let result = handle.fetch_transactions(missing_transactions).await;

        // THEN
        assert!(result.is_ok());

        // Wait a bit for processing to complete
        sleep(Duration::from_millis(100)).await;

        // Verify that the transactions were added to the core
        let fetched_transactions = core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(fetched_transactions.len(), transactions.len());

        // Verify that each transaction was fetched
        for transaction in &transactions {
            assert!(
                fetched_transactions
                    .iter()
                    .any(|t| t.transactions_commitment() == transaction.transactions_commitment())
            );
        }

        // Clean up
        handle.stop().await.unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn live_syncing_with_empty_peer(#[values(true, false)] consensus_fast_commit_sync: bool) {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        let context = Arc::new(context);
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        let mut encoder = create_encoder(&context);

        // Create some test transactions
        let block_round_author: Vec<(Round, u8)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment = TransactionsCommitment::compute_transactions_commitment(
                    &serialized,
                    &context,
                    &mut encoder,
                )
                .unwrap();

                // Create a test block header with the correct commitment
                let header = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author)
                        .set_commitment(commitment)
                        .build(),
                );

                block_headers.push(header.clone());

                VerifiedTransactions::new(
                    transactions,
                    header.transaction_ref(),
                    Some(header.digest()),
                    serialized,
                )
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1)); // This peer will return empty results
            authorities.insert(AuthorityIndex::new_for_test(2)); // This peer will succeed
            let gen_ref = if consensus_fast_commit_sync {
                GenericTransactionRef::from(header.transaction_ref())
            } else {
                GenericTransactionRef::from(header.reference())
            };
            missing_transactions.insert(gen_ref, authorities);
        }

        // Set peer 1 to return empty results
        network_client
            .set_empty_peer(AuthorityIndex::new_for_test(1))
            .await;

        // Stub the transactions for peer 2
        for transaction in &transactions {
            network_client
                .stub_fetch_transactions(
                    vec![transaction.clone()],
                    AuthorityIndex::new_for_test(2),
                    consensus_fast_commit_sync,
                )
                .await;
        }

        // Stub the missing transactions in the core dispatcher
        core_dispatcher
            .stub_missing_transactions(missing_transactions.clone())
            .await;

        // Add block headers to the dag state
        dag_state
            .write()
            .accept_block_headers(block_headers, DataSource::Test);

        // WHEN
        // Request the transactions
        let result = handle.fetch_transactions(missing_transactions).await;

        // THEN
        assert!(result.is_ok());

        // Wait a bit for processing to complete
        sleep(Duration::from_millis(100)).await;

        // Verify that the transactions were added to the core
        let fetched_transactions = core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(fetched_transactions.len(), transactions.len());

        // Verify that each transaction was fetched
        for transaction in &transactions {
            assert!(
                fetched_transactions
                    .iter()
                    .any(|t| t.transactions_commitment() == transaction.transactions_commitment())
            );
        }

        // Clean up
        handle.stop().await.unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn live_syncing_with_corrupted_peer(
        #[values(true, false)] consensus_fast_commit_sync: bool,
    ) {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (mut context, _) = Context::new_for_test(4);
        context.parameters.enable_fast_commit_syncer = consensus_fast_commit_sync;
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(consensus_fast_commit_sync);
        let context = Arc::new(context);
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        let mut encoder = create_encoder(&context);

        // Create some test transactions
        let block_round_author: Vec<(Round, u8)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment = TransactionsCommitment::compute_transactions_commitment(
                    &serialized,
                    &context,
                    &mut encoder,
                )
                .unwrap();

                // Create a test block header with the correct commitment
                let header = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author)
                        .set_commitment(commitment)
                        .build(),
                );

                block_headers.push(header.clone());

                VerifiedTransactions::new(
                    transactions,
                    header.transaction_ref(),
                    Some(header.digest()),
                    serialized,
                )
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1)); // This peer will return corrupted data
            authorities.insert(AuthorityIndex::new_for_test(2)); // This peer will succeed
            if consensus_fast_commit_sync {
                missing_transactions.insert(
                    GenericTransactionRef::from(header.transaction_ref()),
                    authorities,
                );
            } else {
                missing_transactions
                    .insert(GenericTransactionRef::from(header.reference()), authorities);
            }
        }

        // Set peer 1 to return corrupted data
        network_client
            .set_corrupted_peer(AuthorityIndex::new_for_test(1))
            .await;

        // Stub the transactions for peer 2
        for transaction in &transactions {
            network_client
                .stub_fetch_transactions(
                    vec![transaction.clone()],
                    AuthorityIndex::new_for_test(2),
                    consensus_fast_commit_sync,
                )
                .await;
        }

        // Stub the missing transactions in the core dispatcher
        core_dispatcher
            .stub_missing_transactions(missing_transactions.clone())
            .await;

        // Add block headers to the dag state
        dag_state
            .write()
            .accept_block_headers(block_headers, DataSource::Test);

        // WHEN
        // Request the transactions
        let result = handle.fetch_transactions(missing_transactions).await;

        // THEN
        assert!(result.is_ok());

        // Wait a bit for processing to complete
        sleep(Duration::from_millis(100)).await;

        // Verify that the transactions were added to the core
        let fetched_transactions = core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(fetched_transactions.len(), transactions.len());

        // Verify that each transaction was fetched
        for transaction in &transactions {
            assert!(
                fetched_transactions
                    .iter()
                    .any(|t| t.transactions_commitment() == transaction.transactions_commitment())
            );
        }

        // Clean up
        handle.stop().await.unwrap();
    }

    #[tokio::test]
    async fn live_syncing_with_all_peers_failing() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        let mut encoder = create_encoder(&context);

        // Create some test transactions
        let block_round_author: Vec<(Round, u8)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        for (round, author) in &block_round_author {
            // Create a dummy transaction
            let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
            let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
            let commitment = TransactionsCommitment::compute_transactions_commitment(
                &serialized,
                &context,
                &mut encoder,
            )
            .unwrap();

            // Create a test block header with the correct commitment
            let header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(*round, *author)
                    .set_commitment(commitment)
                    .build(),
            );

            block_headers.push(header);
        }

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1)); // This peer will timeout
            authorities.insert(AuthorityIndex::new_for_test(2)); // This peer will return an error
            missing_transactions
                .insert(GenericTransactionRef::from(header.reference()), authorities);
        }

        // Set peer 1 to timeout
        network_client
            .set_timeout_peer(AuthorityIndex::new_for_test(1))
            .await;

        // Set peer 2 to return an error
        network_client
            .set_error_peer(
                AuthorityIndex::new_for_test(2),
                ConsensusError::NetworkRequest("Test error".to_string()),
            )
            .await;

        // Stub the missing transactions in the core dispatcher
        core_dispatcher
            .stub_missing_transactions(missing_transactions.clone())
            .await;

        // Add block headers to the dag state
        dag_state
            .write()
            .accept_block_headers(block_headers, DataSource::Test);

        // WHEN
        // Request the transactions
        let result = handle.fetch_transactions(missing_transactions).await;

        // THEN
        assert!(result.is_ok());

        // Wait a bit for processing to complete
        sleep(Duration::from_millis(100)).await;

        // Verify that no transactions were added to the core
        let fetched_transactions = core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(fetched_transactions.len(), 0);

        // Clean up
        handle.stop().await.unwrap();
    }

    #[tokio::test]
    async fn inflight_transactions_map_with_active_requests() {
        telemetry_subscribers::init_for_testing();

        // GIVEN
        let map = InflightTransactionsMap::new();
        let active_requests = InflightActiveRequests::new();
        let sync_method = SyncMethod::Periodic;

        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
            BlockRef::new(12, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
            BlockRef::new(15, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
        ];
        let context = Context::new_for_test(10).0;
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();
        let missing_transactions_refs = missing_block_refs
            .iter()
            .map(|&br| GenericTransactionRef::from(br))
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
                context.parameters.max_transactions_per_regular_sync_fetch,
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
                context.parameters.max_transactions_per_regular_sync_fetch,
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
            context.parameters.max_transactions_per_regular_sync_fetch,
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

    #[tokio::test]
    async fn excluded_authorities_updates_and_results() {
        telemetry_subscribers::init_for_testing();

        // GIVEN a committee of 7 authorities
        let (context, _) = Context::new_for_test(7);
        let context = Arc::new(context);

        let last_failure = LastFailureByPeer::new(&context);
        let now = Instant::now();

        // WHEN: no updates → excluded set should be empty
        let mut excluded = last_failure.get_excluded_authorities_by_stake();
        assert!(
            excluded.is_empty(),
            "Initially no authorities should be excluded"
        );

        // WHEN: authority 1 fails now
        last_failure.update_with_new_instant(AuthorityIndex::new_for_test(1), now);
        excluded = last_failure.get_excluded_authorities_by_stake();
        assert!(
            excluded.contains(&AuthorityIndex::new_for_test(1)),
            "Authority 1 should be excluded after failure"
        );

        // WHEN: authority 2 fails later
        last_failure.update_with_new_instant(
            AuthorityIndex::new_for_test(2),
            now + Duration::from_millis(50),
        );
        excluded = last_failure.get_excluded_authorities_by_stake();
        assert!(
            excluded.contains(&AuthorityIndex::new_for_test(2)),
            "Authority 2 (latest failure) should be excluded"
        );
        assert!(
            excluded.contains(&AuthorityIndex::new_for_test(1)),
            "Authority 1 should remain excluded as an older failure"
        );

        // WHEN: authority 3 fails even later (newest)
        last_failure.update_with_new_instant(
            AuthorityIndex::new_for_test(3),
            now + Duration::from_millis(100),
        );
        excluded = last_failure.get_excluded_authorities_by_stake();

        // THEN: authority 3 should now be the first excluded one (most recent),
        // but the total excluded stake must remain below the validity threshold.
        assert!(
            excluded.contains(&AuthorityIndex::new_for_test(3)),
            "Newest failed authority (3) should be excluded"
        );
        assert!(
            excluded.contains(&AuthorityIndex::new_for_test(2)),
            "Newest failed authority (3) should be excluded"
        );
        assert!(
            excluded.len() <= 2,
            "Excluded authorities should be strictly less than f+1 stake limit"
        );
    }

    struct MockNetworkClient {
        transactions: Arc<Mutex<HashMap<(AuthorityIndex, GenericTransactionRef), Bytes>>>,
        error_peers: Arc<Mutex<HashMap<AuthorityIndex, ConsensusError>>>,
        timeout_peers: Arc<Mutex<BTreeSet<AuthorityIndex>>>,
        empty_peers: Arc<Mutex<BTreeSet<AuthorityIndex>>>,
        corrupted_peers: Arc<Mutex<BTreeSet<AuthorityIndex>>>,
    }

    impl MockNetworkClient {
        fn new() -> Self {
            Self {
                transactions: Arc::new(Mutex::new(HashMap::new())),
                error_peers: Arc::new(Mutex::new(HashMap::new())),
                timeout_peers: Arc::new(Mutex::new(BTreeSet::new())),
                empty_peers: Arc::new(Mutex::new(BTreeSet::new())),
                corrupted_peers: Arc::new(Mutex::new(BTreeSet::new())),
            }
        }

        async fn stub_fetch_transactions(
            &self,
            transactions: Vec<VerifiedTransactions>,
            peer: AuthorityIndex,
            consensus_fast_commit_sync: bool,
        ) {
            let mut transactions_map = self.transactions.lock().await;
            for transaction in transactions {
                let transaction_ref = transaction.transaction_ref();

                if consensus_fast_commit_sync {
                    // Create a SerializedTransactionsV2 struct with TransactionRef
                    let serialized_transactions = SerializedTransactionsV2 {
                        transaction_ref,
                        serialized_transactions: transaction.serialized().clone(),
                    };
                    let tx_ref = GenericTransactionRef::TransactionRef(transaction_ref);
                    // Serialize the SerializedTransactions struct
                    let serialized = bcs::to_bytes(&serialized_transactions).unwrap();
                    transactions_map.insert((peer, tx_ref), serialized.into());
                } else {
                    // Create a SerializedTransactionsV1 struct with BlockRef
                    let block_ref = transaction
                        .block_ref()
                        .expect("block_ref must be present in non-transaction-ref path");
                    let serialized_transactions = SerializedTransactionsV1 {
                        block_ref,
                        serialized_transactions: transaction.serialized().clone(),
                    };
                    let tx_ref = GenericTransactionRef::BlockRef(block_ref);
                    // Serialize the SerializedTransactions struct
                    let serialized = bcs::to_bytes(&serialized_transactions).unwrap();
                    transactions_map.insert((peer, tx_ref), serialized.into());
                }
            }
        }

        // Set a peer to return an error
        async fn set_error_peer(&self, peer: AuthorityIndex, error: ConsensusError) {
            let mut error_peers = self.error_peers.lock().await;
            error_peers.insert(peer, error);
        }

        // Set a peer to timeout
        async fn set_timeout_peer(&self, peer: AuthorityIndex) {
            let mut timeout_peers = self.timeout_peers.lock().await;
            timeout_peers.insert(peer);
        }

        // Set a peer to return empty results
        async fn set_empty_peer(&self, peer: AuthorityIndex) {
            let mut empty_peers = self.empty_peers.lock().await;
            empty_peers.insert(peer);
        }

        // Set a peer to return corrupted data
        async fn set_corrupted_peer(&self, peer: AuthorityIndex) {
            let mut corrupted_peers = self.corrupted_peers.lock().await;
            corrupted_peers.insert(peer);
        }
    }

    // Extended MockCoreThreadDispatcher that implements the methods needed for
    // TransactionsSynchronizer tests
    #[derive(Default)]
    struct MockCoreThreadDispatcher {
        transactions: Mutex<Vec<VerifiedTransactions>>,
        missing_transactions: Mutex<BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>>,
    }

    impl MockCoreThreadDispatcher {
        fn new() -> Self {
            Self {
                transactions: Mutex::new(Vec::new()),
                missing_transactions: Mutex::new(BTreeMap::new()),
            }
        }

        async fn get_and_drain_transactions(&self) -> Vec<VerifiedTransactions> {
            let mut transactions = self.transactions.lock().await;
            transactions.drain(0..).collect()
        }

        async fn stub_missing_transactions(
            &self,
            missing_transactions: BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
        ) {
            let mut missing = self.missing_transactions.lock().await;
            *missing = missing_transactions;
        }
    }

    #[async_trait]
    impl CoreThreadDispatcher for MockCoreThreadDispatcher {
        async fn add_blocks(
            &self,
            _blocks: Vec<VerifiedBlock>,
            _source: DataSource,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            unimplemented!()
        }

        async fn add_block_headers(
            &self,
            _block_headers: Vec<VerifiedBlockHeader>,
            _source: DataSource,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            unimplemented!()
        }

        async fn add_transactions(
            &self,
            transactions: Vec<VerifiedTransactions>,
            _source: DataSource,
        ) -> Result<(), CoreError> {
            let mut txns = self.transactions.lock().await;

            // Add unique transactions to avoid duplicates
            let mut seen = BTreeSet::new();
            // Populate with txns
            for transaction in txns.iter() {
                seen.insert(transaction.transactions_commitment());
            }
            for transaction in transactions {
                if !seen.contains(&transaction.transactions_commitment()) {
                    seen.insert(transaction.transactions_commitment());
                    txns.push(transaction);
                }
            }
            Ok(())
        }

        async fn add_shards(&self, _shards: Vec<VerifiedOwnShard>) -> Result<(), CoreError> {
            unimplemented!("Unimplemented")
        }
        async fn get_missing_transaction_data(
            &self,
        ) -> Result<BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>, CoreError> {
            let missing = self.missing_transactions.lock().await;

            // Lock transactions once, outside the loop
            let transactions = self.transactions.lock().await;

            let mut filtered: BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>> =
                BTreeMap::new();

            for (gen_tr_ref, authority_set) in missing.iter() {
                let exists = transactions.iter().any(|txn| {
                    let tx_ref_match =
                        GenericTransactionRef::TransactionRef(txn.transaction_ref()) == *gen_tr_ref;
                    let block_ref_match = txn
                        .block_ref()
                        .is_some_and(|br| GenericTransactionRef::BlockRef(br) == *gen_tr_ref);
                    tx_ref_match || block_ref_match
                });

                if !exists {
                    filtered.insert(*gen_tr_ref, authority_set.clone());
                }
            }

            Ok(filtered)
        }

        async fn add_certified_commits(
            &self,
            _commits: CertifiedCommits,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            unimplemented!()
        }

        async fn add_subdags_from_fast_sync(
            &self,
            _output: crate::commit_syncer::fast::FastSyncOutput,
        ) -> Result<(), CoreError> {
            unimplemented!()
        }

        async fn reinitialize_components(
            &self,
            _block_headers: Vec<crate::block_header::VerifiedBlockHeader>,
        ) -> Result<(), CoreError> {
            unimplemented!()
        }

        async fn new_block(
            &self,
            _round: Round,
            _reason: ReasonToCreateBlock,
        ) -> Result<BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>, CoreError> {
            unimplemented!()
        }

        async fn get_missing_block_headers(
            &self,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            unimplemented!()
        }

        fn set_quorum_subscribers_exists(&self, _exists: bool) -> Result<(), CoreError> {
            unimplemented!()
        }

        fn set_last_known_proposed_round(&self, _round: Round) -> Result<(), CoreError> {
            unimplemented!()
        }
    }

    #[async_trait]
    impl NetworkClient for MockNetworkClient {
        async fn subscribe_block_bundles(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockBundleStream> {
            // Not needed for transactions synchronizer tests
            unimplemented!("fetch_latest_block_headers not implemented in mock")
        }

        async fn fetch_transactions(
            &self,
            peer: AuthorityIndex,
            block_refs: Vec<GenericTransactionRef>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            // Check if this peer is set to timeout
            let timeout_peers = self.timeout_peers.lock().await;
            if timeout_peers.contains(&peer) {
                // Sleep for a long time to simulate timeout
                // The actual timeout will be handled by the caller
                sleep(Duration::from_secs(10)).await;
                return Ok(Vec::new());
            }

            // Check if this peer is set to return an error
            let error_peers = self.error_peers.lock().await;
            if let Some(error) = error_peers.get(&peer) {
                return Err(error.clone());
            }

            // Check if this peer is set to return empty results
            let empty_peers = self.empty_peers.lock().await;
            if empty_peers.contains(&peer) {
                return Ok(Vec::new());
            }

            // Check if this peer is set to return corrupted data
            let corrupted_peers = self.corrupted_peers.lock().await;
            if corrupted_peers.contains(&peer) {
                // Return corrupted data (invalid bytes that can't be deserialized)
                let mut result = Vec::new();
                for _ in 0..block_refs.len() {
                    result.push(Bytes::from(vec![0, 1, 2, 3])); // Invalid serialized data
                }
                return Ok(result);
            }

            // Normal case - return transactions from the map
            let transactions_map = self.transactions.lock().await;
            let mut result = Vec::new();
            for block_ref in block_refs {
                if let Some(serialized) = transactions_map.get(&(peer, block_ref)) {
                    result.push(serialized.clone());
                }
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
            // Not needed for transactions synchronizer tests
            unimplemented!("fetch_block_headers not implemented in mock")
        }

        async fn fetch_commits(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)> {
            // Not needed for transactions synchronizer tests
            unimplemented!("fetch_commits not implemented in mock")
        }

        async fn fetch_latest_block_headers(
            &self,
            _peer: AuthorityIndex,
            _authorities: Vec<AuthorityIndex>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            // Not needed for transactions synchronizer tests
            unimplemented!("fetch_latest_block_headers not implemented in mock")
        }

        async fn fetch_commits_and_transactions(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("fetch_commits_and_transactions not implemented in mock")
        }
    }
}
