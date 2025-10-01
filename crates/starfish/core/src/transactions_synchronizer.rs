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
    time::{Instant, sleep, sleep_until, timeout},
};
use tracing::{debug, info, warn};

use crate::{
    Transaction, VerifiedBlockHeader,
    block_header::{BlockRef, TransactionsCommitment, VerifiedTransactions},
    block_verifier::BlockVerifier,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    encoder::create_encoder,
    error::{ConsensusError, ConsensusResult},
    network::{NetworkClient, SerializedTransactions},
};

/// The number of concurrent live transaction fetch requests
const LIVE_FETCH_TRANSACTIONS_CONCURRENCY: usize = 1;
const PERIODIC_FETCH_TRANSACTIONS_CONCURRENCY: usize = 1;

/// The maximum number of concurrent request per authority for fetching
/// transactions Used separately for live fetches and periodic fetches.
const MAX_CONCURRENT_REQUESTS_PER_AUTHORITY: usize = 5;

/// Timeout for the transactions synchronizer to run periodically and fetch
/// missing transactions.
const TRANSACTIONS_SYNCHRONIZER_TIMEOUT: Duration = Duration::from_millis(200);

/// Timeout to fetch transactions from a given peer.
const FETCH_REQUEST_TIMEOUT: Duration = Duration::from_millis(500);

/// Timeout to fetch and process transactions from all peers in one call of
/// `fetch_and_process_transactions_from_authorities`.
const FETCH_AND_PROCESS_FROM_PEERS_TIMEOUT: Duration = Duration::from_millis(700);

/// Maximum number of authorities that can concurrently fetch transactions for a
/// given block ref.
const MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION: usize = 3;

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

struct ActiveRequestGuard {
    authority: AuthorityIndex,
    sync_method: SyncMethod,
    active_requests: Arc<Mutex<BTreeMap<(AuthorityIndex, SyncMethod), usize>>>,
}

impl ActiveRequestGuard {
    fn new(
        authority: AuthorityIndex,
        sync_method: SyncMethod,
        active_requests: Arc<Mutex<BTreeMap<(AuthorityIndex, SyncMethod), usize>>>,
    ) -> Self {
        {
            let mut map = active_requests.lock();
            *map.entry((authority, sync_method)).or_insert(0) += 1;
        }
        Self {
            authority,
            sync_method,
            active_requests,
        }
    }
}

impl Drop for ActiveRequestGuard {
    fn drop(&mut self) {
        let mut map = self.active_requests.lock();
        if let Some(val) = map.get_mut(&(self.authority, self.sync_method)) {
            *val = val.saturating_sub(1);
        }
    }
}

struct TransactionsGuard {
    map: Arc<InflightTransactionsMap>,
    block_refs: BTreeSet<BlockRef>,
    peer: AuthorityIndex,
}

impl Drop for TransactionsGuard {
    fn drop(&mut self) {
        self.map.unlock_transactions(&self.block_refs, self.peer);
    }
}

// Keeps a mapping between the missing transactions that have been instructed to
// be fetched and the authorities that are currently fetching them. For a block
// ref there is a maximum number of authorities that can concurrently fetch it.
// The authority ids that are currently fetching a transaction are set on the
// corresponding `BTreeSet` and basically they act as "locks".
struct InflightTransactionsMap {
    inner: Mutex<HashMap<BlockRef, BTreeSet<AuthorityIndex>>>,
}

impl InflightTransactionsMap {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(HashMap::new()),
        })
    }

    /// Locks the transactions to be fetched for the assigned `peer_index`. We
    /// want to avoid re-fetching the missing transactions from too many
    /// authorities at the same time, thus we limit the concurrency per
    /// transaction by attempting to lock per block. If a transaction is
    /// already fetched by the maximum allowed number of authorities, then
    /// the block ref will not be included in the returned set. The method
    /// returns all the block refs that have been successfully locked and
    /// allowed to be fetched.
    fn lock_transactions(
        self: &Arc<Self>,
        missing_block_refs: BTreeSet<BlockRef>,
        peer: AuthorityIndex,
        max_number_transactions_per_fetch: usize,
    ) -> Option<TransactionsGuard> {
        let mut blocks = BTreeSet::new();
        let mut inner = self.inner.lock();
        let mut selected_block_refs_num = 0;

        for block_ref in missing_block_refs {
            // check that the number of authorities that are already instructed to fetch the
            // transaction is not higher than the allowed and the `peer_index` has not
            // already been instructed to do that.
            let authorities = inner.entry(block_ref).or_default();
            if authorities.len() < MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION
                && authorities.insert(peer)
            {
                blocks.insert(block_ref);
                selected_block_refs_num += 1;
            }
            if selected_block_refs_num >= max_number_transactions_per_fetch {
                break;
            }
        }

        if blocks.is_empty() {
            None
        } else {
            Some(TransactionsGuard {
                map: self.clone(),
                block_refs: blocks,
                peer,
            })
        }
    }

    /// Unlocks the provided block references for the given `peer`. The
    /// unlocking is strict, meaning that if this method is called for a
    /// specific block ref and peer more times than the corresponding lock
    /// has been called, it will panic.
    fn unlock_transactions(
        self: &Arc<Self>,
        block_refs: &BTreeSet<BlockRef>,
        peer: AuthorityIndex,
    ) {
        // Now mark all the transactions as fetched from the map
        let mut transactions_to_fetch = self.inner.lock();
        for block_ref in block_refs {
            let authorities = transactions_to_fetch
                .get_mut(block_ref)
                .expect("We should expect a non empty map with at least one peer");
            assert!(authorities.remove(&peer), "Peer index should be present!");
            // If the last one then just clean up
            if authorities.is_empty() {
                transactions_to_fetch.remove(block_ref);
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
        missing_block_refs: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
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
        missing_block_refs: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
    ) -> ConsensusResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.commands_sender
            .send(Command::FetchTransactions {
                missing_block_refs,
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
pub(crate) struct TransactionsSynchronizer<
    C: NetworkClient,
    V: BlockVerifier,
    D: CoreThreadDispatcher,
> {
    context: Arc<Context>,
    commands_receiver: Receiver<Command>,
    live_fetch_requests: Sender<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>,
    core_dispatcher: Arc<D>,
    dag_state: Arc<RwLock<DagState>>,
    active_requests: Arc<Mutex<BTreeMap<(AuthorityIndex, SyncMethod), usize>>>,
    fetch_transactions_scheduler_task: JoinSet<()>,
    network_client: Arc<C>,
    block_verifier: Arc<V>,
    inflight_transactions_map: Arc<InflightTransactionsMap>,
    commands_sender: Sender<Command>,
}

impl<C: NetworkClient, V: BlockVerifier, D: CoreThreadDispatcher>
    TransactionsSynchronizer<C, V, D>
{
    /// Starts the transactions synchronizer, which is responsible for fetching
    /// transactions from other authorities and managing transaction
    /// synchronization tasks.
    pub fn start(
        network_client: Arc<C>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        block_verifier: Arc<V>,
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
        let active_requests = Arc::new(Mutex::new(BTreeMap::new()));
        // Spawn the live fetcher task
        let live_fetcher_async = Self::live_fetcher(
            active_requests.clone(),
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
            live_fetch_receiver,
            block_verifier.clone(),
            inflight_transactions_map.clone(),
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
                block_verifier,
                inflight_transactions_map,
                commands_sender: commands_sender_clone,
                dag_state,
            };
            s.run().await;
        }));

        Arc::new(TransactionsSynchronizerHandle {
            commands_sender,
            tasks: tokio::sync::Mutex::new(tasks),
        })
    }

    // The main loop to listen for the submitted commands.
    #[cfg_attr(test,tracing::instrument(skip_all, name ="",fields(authority = %self.context.own_index)))]
    async fn run(&mut self) {
        // We want the transactions synchronizer to run periodically to
        // fetch any missing transactions.
        let scheduler_timeout = sleep_until(Instant::now() + TRANSACTIONS_SYNCHRONIZER_TIMEOUT);

        tokio::pin!(scheduler_timeout);

        loop {
            tokio::select! {
                Some(command) = self.commands_receiver.recv() => {
                    match command {
                        Command::FetchTransactions{ missing_block_refs, result } => {
                            // Enqueue the request to the live fetcher and return immediately.
                            let r =  self.live_fetch_requests.try_send(missing_block_refs)
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
        active_requests: Arc<Mutex<BTreeMap<(AuthorityIndex, SyncMethod), usize>>>,
        network_client: Arc<C>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        dag_state: Arc<RwLock<DagState>>,
        mut receiver: Receiver<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>,
        block_verifier: Arc<V>,
        inflight_transactions_map: Arc<InflightTransactionsMap>,
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
                    let block_verifier = block_verifier.clone();
                    let dag_state = dag_state.clone();

                    tokio::spawn(async move {
                        Self::fetch_and_process_transactions_from_authorities(
                            context,
                            active_requests,
                            inflight_transactions_map,
                            network_client,
                            missing_transactions_block_refs,
                            core_dispatcher,
                            block_verifier,
                            dag_state,
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
        info!("Kick in periodic synchronizer to fetch missing transactions");
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
            .map(|(block_ref, _)| block_ref.round)
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
            missing_transactions_per_authority[block_ref.author] += 1;
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
        let block_verifier = self.block_verifier.clone();
        let dag_state = self.dag_state.clone();
        let inflight_transactions_map = self.inflight_transactions_map.clone();
        let active_requests = self.active_requests.clone();

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
                    block_verifier,
                    dag_state,
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
        active_requests: Arc<Mutex<BTreeMap<(AuthorityIndex, SyncMethod), usize>>>,
        inflight_transactions_map: Arc<InflightTransactionsMap>,
        network_client: Arc<C>,
        missing_transactions: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        core_dispatcher: Arc<D>,
        block_verifier: Arc<V>,
        dag_state: Arc<RwLock<DagState>>,
        sync_method: SyncMethod,
    ) {
        // Build a mapping from authority -> set of BlockRefs it has acknowledged
        let mut blocks_by_authority: BTreeMap<AuthorityIndex, BTreeSet<BlockRef>> = BTreeMap::new();
        for (block_ref, authorities) in &missing_transactions {
            for authority in authorities {
                if *authority != context.own_index {
                    blocks_by_authority
                        .entry(*authority)
                        .or_default()
                        .insert(*block_ref);
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
        let iter_authorities: Box<dyn Iterator<Item = (AuthorityIndex, BTreeSet<BlockRef>)>> =
            if cfg!(test) {
                // Stable order for tests
                Box::new(blocks_by_authority.into_iter())
            } else {
                let mut vec: Vec<_> = blocks_by_authority.into_iter().collect();
                vec.shuffle(&mut rng);
                Box::new(vec.into_iter())
            };

        let mut request_futures = FuturesUnordered::new();

        for (authority, authority_block_refs) in iter_authorities {
            {
                let count = active_requests
                    .lock()
                    .get(&(authority, sync_method))
                    .copied()
                    .unwrap_or(0);

                // Skip assigning a request if the limit is reached
                if count >= MAX_CONCURRENT_REQUESTS_PER_AUTHORITY {
                    let peer_hostname = &context.committee.authority(authority).hostname;
                    debug!(
                        "Skipping fetch for authority {peer_hostname} as the maximum number of concurrent requests is reached"
                    );
                    continue;
                }
            }

            // * If transactions are successfully locked, then send a request to the network
            //   client to fetch the transactions from the authority. If the fetch is
            //   successful, then process the transactions and send them to the core for
            //   processing.
            if let Some(transactions_guard) = inflight_transactions_map.lock_transactions(
                authority_block_refs.clone(),
                authority,
                context.parameters.max_transactions_per_fetch,
            ) {
                let active_request_guard =
                    ActiveRequestGuard::new(authority, sync_method, active_requests.clone());

                request_futures.push(Self::fetch_and_process_transactions_from_authority(
                    authority,
                    context.clone(),
                    transactions_guard,
                    network_client.clone(),
                    core_dispatcher.clone(),
                    block_verifier.clone(),
                    dag_state.clone(),
                    sync_method,
                    active_request_guard,
                ));
            }
        }

        if request_futures.is_empty() {
            return;
        }

        // Each fetch request has its own timeout, but processing not.
        // Using timeout, we limit the overall time spent primarily on processing
        let timeout = sleep(FETCH_AND_PROCESS_FROM_PEERS_TIMEOUT);

        tokio::pin!(timeout);

        loop {
            tokio::select! {
                    Some(res) = request_futures.next() => {
                        if let Err(err) = res {
                            warn!("[{}] Error when fetching and processing transactions from authority: {err}", sync_method.get_string());
                        }
            },
                    _ = &mut timeout => {
                        warn!("[{}] Timed out while fetching and processing missing transactions", sync_method.get_string());
                         // Drop all pending requests immediately — frees all transaction guards
                        drop(request_futures);
                        break;
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
        block_verifier: Arc<V>,
        dag_state: Arc<RwLock<DagState>>,
        sync_method: SyncMethod,
        _active_guard: ActiveRequestGuard,
    ) -> ConsensusResult<()> {
        let peer_hostname = &context.committee.authority(peer).hostname;
        let total_requested = transactions_guard.block_refs.len();

        debug!(
            "[{}] Syncing {total_requested} missing committed transactions from authority {peer} {peer_hostname}",
            sync_method.get_string(),
        );

        let (fetched_serialized_transactions, transactions_guard, _peer_index) =
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
            block_verifier.clone(),
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

        let block_refs = transactions_guard
            .block_refs
            .iter()
            .cloned()
            .collect::<Vec<_>>();

        let peer_hostname = &context.committee.authority(peer).hostname;
        let start_time = Instant::now();
        // Fetch the transactions from the peer
        let result = timeout(
            request_timeout,
            network_client.fetch_transactions(peer, block_refs.clone(), request_timeout),
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
        serialized_transactions: Vec<Bytes>,
        peer_index: AuthorityIndex,
        requested_transactions_guard: TransactionsGuard,
        core_dispatcher: Arc<D>,
        context: Arc<Context>,
        block_verifier: Arc<V>,
        dag_state: Arc<RwLock<DagState>>,
        sync_method: SyncMethod,
    ) -> ConsensusResult<()> {
        // Ensure that all the returned transactions do not go over the total max
        // allowed returned transactions
        if serialized_transactions.len() > requested_transactions_guard.block_refs.len() {
            return Err(ConsensusError::TooManyFetchedTransactionsReturned(
                peer_index,
            ));
        }

        let metrics = &context.metrics.node_metrics;
        let peer_hostname = &context.committee.authority(peer_index).hostname;

        // Deserialize and verify the transactions
        // inside verify_transactions
        let transactions = match Handle::current()
            .spawn_blocking({
                // Use the block_refs from the requested_transactions_guard
                let block_refs: Vec<BlockRef> = requested_transactions_guard
                    .block_refs
                    .iter()
                    .cloned()
                    .collect();
                let block_headers_vec = dag_state.read().get_block_headers(&block_refs);
                let mut block_headers_map = BTreeMap::new();
                for block_header_opt in block_headers_vec.into_iter() {
                    let block_header = block_header_opt
                        .expect("block header for requested transactions must exist");
                    block_headers_map.insert(block_header.reference(), block_header);
                }

                let block_verifier = block_verifier.clone();
                let context_cloned = context.clone();
                move || {
                    Self::verify_transactions(
                        serialized_transactions,
                        block_verifier,
                        peer_index,
                        block_headers_map,
                        context_cloned,
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
        };

        metrics
            .transactions_synchronizer_fetched_transactions_by_peer
            .with_label_values(&[peer_hostname.as_str(), &sync_method.get_string()])
            .inc_by(transactions.len() as u64);
        for transactions in &transactions {
            let block_hostname = &context
                .committee
                .authority(transactions.block_ref().author)
                .hostname;
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
                .map(|b| b.block_ref().to_string())
                .join(", "),
        );

        // Add the transactions to the core
        core_dispatcher
            .add_transactions(transactions)
            .await
            .map_err(|_| ConsensusError::Shutdown)?;

        // now release all the locked blocks as they have been fetched, verified &
        // processed
        drop(requested_transactions_guard);

        Ok(())
    }

    fn verify_transactions(
        serialized_transactions_bytes: Vec<Bytes>,
        block_verifier: Arc<V>,
        peer_index: AuthorityIndex,
        block_headers_map: BTreeMap<BlockRef, VerifiedBlockHeader>,
        context: Arc<Context>,
    ) -> ConsensusResult<Vec<VerifiedTransactions>> {
        let mut collected_verified_transactions = Vec::new();

        for serialized_transaction_bytes in &serialized_transactions_bytes {
            // Step 1: Deserialize the outer SerializedTransactions wrapper to get the block
            // reference and the inner serialized transactions bytes. This
            // allows us to identify which block these transactions belong to
            // and access their commitment in the block header.
            let serialized_transactions: SerializedTransactions =
                bcs::from_bytes(serialized_transaction_bytes)
                    .map_err(ConsensusError::MalformedTransactions)?;

            // Step 2: Get the block header and verify that the transactions commitment
            // matches. This ensures the transactions we received are exactly
            // the ones that were included in the block when it was created.
            let block_header = block_headers_map
                .get(&serialized_transactions.block_ref)
                .expect("header for fetched transactions must exist");

            let mut encoder = create_encoder(&context);
            if block_header.transactions_commitment()
                != TransactionsCommitment::compute_transactions_commitment(
                    &serialized_transactions.serialized_transactions,
                    &context,
                    &mut encoder,
                )
                .expect("correct computation of the transactions commitment should be successful")
            {
                return Err(ConsensusError::TransactionCommitmentFailure {
                    round: serialized_transactions.block_ref.round,
                    author: serialized_transactions.block_ref.author,
                    peer: peer_index,
                });
            }

            // Step 3: Deserialize and verify the actual transactions vector.
            let transactions: Vec<Transaction> =
                bcs::from_bytes(&serialized_transactions.serialized_transactions)
                    .map_err(ConsensusError::MalformedTransactions)?;

            block_verifier.check_and_verify_transactions(&transactions)?;

            // Step 4: Create a VerifiedTransactions instance containing both the verified
            // transactions and their original serialized form for efficient re-sharing
            let verified_transactions = VerifiedTransactions::new(
                transactions,
                serialized_transactions.block_ref,
                block_header.transactions_commitment(),
                serialized_transactions.serialized_transactions,
            );

            collected_verified_transactions.push(verified_transactions);
        }

        Ok(collected_verified_transactions)
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
    use tokio::{sync::Mutex, time::sleep};

    use super::*;
    use crate::{
        Round, TestBlockHeader,
        block_header::{
            BlockHeaderDigest, BlockRef, TransactionsCommitment, VerifiedBlock,
            VerifiedBlockHeader, VerifiedTransactions,
        },
        block_verifier::NoopBlockVerifier,
        commit::{CertifiedCommits, CommitRange},
        context::Context,
        core_thread::CoreError,
        dag_state::DagState,
        network::{BlockBundleStream, NetworkClient, SerializedTransactions},
        storage::mem_store::MemStore,
    };

    #[tokio::test]
    async fn successful_live_syncing() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let transaction_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
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

                VerifiedTransactions::new(transactions, header.reference(), commitment, serialized)
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1));
            authorities.insert(AuthorityIndex::new_for_test(2));
            missing_transactions.insert(header.reference(), authorities);
        }

        // Stub the transactions in the network client
        for transaction in &transactions {
            network_client
                .stub_fetch_transactions(vec![transaction.clone()], AuthorityIndex::new_for_test(1))
                .await;
        }

        dag_state.write().accept_block_headers(block_headers);

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
                    .any(|t| t.block_ref() == transaction.block_ref())
            );
        }

        // Clean up
        transaction_synchronizer.stop().await.unwrap();
    }

    #[tokio::test]
    async fn live_syncing_with_saturated_tasks() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
            dag_state.clone(),
        );
        let mut encoder = create_encoder(&context);

        // Create block round author pairs
        let block_round_authors = (1..LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 2 + 1)
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

            let verified_transaction =
                VerifiedTransactions::new(transactions, header.reference(), commitment, serialized);

            verified_transactions.push(verified_transaction);
        }

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1));
            missing_transactions.insert(header.reference(), authorities);
        }

        // Delay fetch transactions response to simulate saturation deterministically.
        network_client
            .set_timeout_peer(AuthorityIndex::new_for_test(1))
            .await;

        // Add block headers to the dag state
        dag_state.write().accept_block_headers(block_headers);

        // WHEN
        // Send many requests to saturate the tasks
        let mut results = Vec::new();
        for _ in 0..LIVE_FETCH_TRANSACTIONS_CONCURRENCY * 3 {
            results.push(
                handle
                    .fetch_transactions(missing_transactions.clone())
                    .await,
            );
        }

        // THEN
        // FETCH_TRANSACTIONS_CONCURRENCY tasks will start processing, another set of
        // FETCH_TRANSACTIONS_CONCURRENCY tasks will be stuck in the queue, and the last
        // FETCH_TRANSACTIONS_CONCURRENCY tasks will be returned with
        // TransactionSynchronizerSaturated error.
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

    #[tokio::test]
    async fn live_syncing_with_timeout_peer() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
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

                VerifiedTransactions::new(transactions, header.reference(), commitment, serialized)
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1)); // This peer will timeout
            authorities.insert(AuthorityIndex::new_for_test(2)); // This peer will succeed
            missing_transactions.insert(header.reference(), authorities);
        }

        // Set peer 1 to timeout
        network_client
            .set_timeout_peer(AuthorityIndex::new_for_test(1))
            .await;

        // Stub the transactions for peer 2
        for transaction in &transactions {
            network_client
                .stub_fetch_transactions(vec![transaction.clone()], AuthorityIndex::new_for_test(2))
                .await;
        }

        // Stub the missing transactions in the core dispatcher
        core_dispatcher
            .stub_missing_transactions(missing_transactions.clone())
            .await;

        // Add block headers to the dag state
        dag_state.write().accept_block_headers(block_headers);

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
                    .any(|t| t.block_ref() == transaction.block_ref())
            );
        }

        // Clean up
        handle.stop().await.unwrap();
    }

    #[tokio::test]
    async fn live_syncing_with_error_peer() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
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

                VerifiedTransactions::new(transactions, header.reference(), commitment, serialized)
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1)); // This peer will return an error
            authorities.insert(AuthorityIndex::new_for_test(2)); // This peer will succeed
            missing_transactions.insert(header.reference(), authorities);
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
                .stub_fetch_transactions(vec![transaction.clone()], AuthorityIndex::new_for_test(2))
                .await;
        }

        // Add block headers to the dag state
        dag_state.write().accept_block_headers(block_headers);

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
                    .any(|t| t.block_ref() == transaction.block_ref())
            );
        }

        // Clean up
        handle.stop().await.unwrap();
    }

    #[tokio::test]
    async fn live_syncing_with_empty_peer() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
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

                VerifiedTransactions::new(transactions, header.reference(), commitment, serialized)
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1)); // This peer will return empty results
            authorities.insert(AuthorityIndex::new_for_test(2)); // This peer will succeed
            missing_transactions.insert(header.reference(), authorities);
        }

        // Set peer 1 to return empty results
        network_client
            .set_empty_peer(AuthorityIndex::new_for_test(1))
            .await;

        // Stub the transactions for peer 2
        for transaction in &transactions {
            network_client
                .stub_fetch_transactions(vec![transaction.clone()], AuthorityIndex::new_for_test(2))
                .await;
        }

        // Stub the missing transactions in the core dispatcher
        core_dispatcher
            .stub_missing_transactions(missing_transactions.clone())
            .await;

        // Add block headers to the dag state
        dag_state.write().accept_block_headers(block_headers);

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
                    .any(|t| t.block_ref() == transaction.block_ref())
            );
        }

        // Clean up
        handle.stop().await.unwrap();
    }

    #[tokio::test]
    async fn live_syncing_with_corrupted_peer() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
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

                VerifiedTransactions::new(transactions, header.reference(), commitment, serialized)
            })
            .collect::<Vec<_>>();

        // Create a map of block refs to authorities that have them
        let mut missing_transactions = BTreeMap::new();
        for header in &block_headers {
            let mut authorities = BTreeSet::new();
            authorities.insert(AuthorityIndex::new_for_test(1)); // This peer will return corrupted data
            authorities.insert(AuthorityIndex::new_for_test(2)); // This peer will succeed
            missing_transactions.insert(header.reference(), authorities);
        }

        // Set peer 1 to return corrupted data
        network_client
            .set_corrupted_peer(AuthorityIndex::new_for_test(1))
            .await;

        // Stub the transactions for peer 2
        for transaction in &transactions {
            network_client
                .stub_fetch_transactions(vec![transaction.clone()], AuthorityIndex::new_for_test(2))
                .await;
        }

        // Stub the missing transactions in the core dispatcher
        core_dispatcher
            .stub_missing_transactions(missing_transactions.clone())
            .await;

        // Add block headers to the dag state
        dag_state.write().accept_block_headers(block_headers);

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
                    .any(|t| t.block_ref() == transaction.block_ref())
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
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
        let network_client = Arc::new(MockNetworkClient::new());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Start the transactions synchronizer
        let handle = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            block_verifier.clone(),
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
            missing_transactions.insert(header.reference(), authorities);
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
        dag_state.write().accept_block_headers(block_headers);

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
    async fn inflight_transactions_map() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let map = InflightTransactionsMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
            BlockRef::new(12, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
            BlockRef::new(15, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
        ];
        let context = Context::new_for_test(4).0;
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // Lock & unlock transactions
        {
            let mut all_guards = Vec::new();

            // Try to acquire the transaction locks for authorities 0, 1 & 2
            for i in 0..=2 {
                let authority = AuthorityIndex::new_for_test(i);

                let guard = map.lock_transactions(
                    missing_block_refs.clone(),
                    authority,
                    context.parameters.max_transactions_per_fetch,
                );
                let guard = guard.expect("Guard should be created");
                assert_eq!(guard.block_refs.len(), 4);

                all_guards.push(guard);

                // trying to acquire any of them again will not succeed
                let guard = map.lock_transactions(
                    missing_block_refs.clone(),
                    authority,
                    context.parameters.max_transactions_per_fetch,
                );
                assert!(guard.is_none());
            }

            // Trying to acquire for authority 3 it will fail - as we have maxed out the
            // number of allowed peers (MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION = 3)
            let authority_3 = AuthorityIndex::new_for_test(3);

            let guard = map.lock_transactions(
                missing_block_refs.clone(),
                authority_3,
                context.parameters.max_transactions_per_fetch,
            );
            assert!(guard.is_none());

            // Explicitly drop the guard of authority 1 and try for authority 3 again - it
            // will now succeed
            drop(all_guards.remove(0));

            let guard = map.lock_transactions(
                missing_block_refs.clone(),
                authority_3,
                context.parameters.max_transactions_per_fetch,
            );
            let guard = guard.expect("Guard should be successfully acquired");

            assert_eq!(guard.block_refs, missing_block_refs);

            // Dropping all guards should unlock on the block refs
            drop(guard);
            drop(all_guards);

            assert_eq!(map.num_of_locked_transactions(), 0);
        }
    }
    struct MockNetworkClient {
        transactions: Arc<Mutex<HashMap<(AuthorityIndex, BlockRef), Bytes>>>,
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
        ) {
            let mut transactions_map = self.transactions.lock().await;
            for transaction in transactions {
                let block_ref = transaction.block_ref();

                // Create a SerializedTransactions struct
                let serialized_transactions = SerializedTransactions {
                    block_ref,
                    serialized_transactions: transaction.serialized().clone(),
                };

                // Serialize the SerializedTransactions struct
                let serialized = bcs::to_bytes(&serialized_transactions).unwrap();
                transactions_map.insert((peer, block_ref), serialized.into());
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
        missing_transactions: Mutex<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>,
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
            missing_transactions: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
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
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            unimplemented!()
        }

        async fn add_block_headers(
            &self,
            _block_headers: Vec<VerifiedBlockHeader>,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            unimplemented!()
        }

        async fn add_transactions(
            &self,
            transactions: Vec<VerifiedTransactions>,
        ) -> Result<(), CoreError> {
            let mut txns = self.transactions.lock().await;

            // Add unique transactions to avoid duplicates
            let mut seen = BTreeSet::new();
            // Populate with txns
            for transaction in txns.iter() {
                seen.insert(transaction.block_ref());
            }
            for transaction in transactions {
                if !seen.contains(&transaction.block_ref()) {
                    seen.insert(transaction.block_ref());
                    txns.push(transaction);
                }
            }
            Ok(())
        }

        async fn get_missing_transaction_data(
            &self,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            let missing = self.missing_transactions.lock().await;

            // Lock transactions once, outside the loop
            let transactions = self.transactions.lock().await;

            let mut filtered: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>> = BTreeMap::new();

            for (block_ref, authority_set) in missing.iter() {
                let exists = transactions.iter().any(|txn| txn.block_ref() == *block_ref);

                if !exists {
                    filtered.insert(*block_ref, authority_set.clone());
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
                BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            unimplemented!()
        }

        async fn new_block(
            &self,
            _round: Round,
            _force: bool,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
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

        fn highest_received_rounds(&self) -> Vec<Round> {
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
            block_refs: Vec<BlockRef>,
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
    }
}
