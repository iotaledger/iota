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
    seq::IteratorRandom,
};
use starfish_config::AuthorityIndex;
use tokio::{
    runtime::Handle,
    sync::{mpsc::error::TrySendError, oneshot},
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
    error::{ConsensusError, ConsensusResult},
    network::{NetworkClient, SerializedTransactions},
};

/// The number of concurrent fetch transactions requests per authority
const FETCH_TRANSACTIONS_CONCURRENCY: usize = 5;

/// Timeouts when fetching transactions.
const FETCH_REQUEST_TIMEOUT: Duration = Duration::from_millis(2_000);
const FETCH_FROM_PEERS_TIMEOUT: Duration = Duration::from_millis(4_000);

/// Max number of transactions to fetch per request.
/// This value should be chosen so even with transactions at max size, the
/// requests can finish on hosts with good network using the timeouts above.
const MAX_TRANSACTIONS_PER_FETCH: usize = 32;

/// TODO: this should be calculated based on the number of authorities and
///  should be at least 2/3 of all authorities by stake
const MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION: usize = 2;

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
    ) -> Option<TransactionsGuard> {
        let mut blocks = BTreeSet::new();
        let mut inner = self.inner.lock();

        for block_ref in missing_block_refs {
            // check that the number of authorities that are already instructed to fetch the
            // transaction is not higher than the allowed and the `peer_index` has not
            // already been instructed to do that.
            let authorities = inner.entry(block_ref).or_default();
            if authorities.len() < MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION
                && authorities.get(&peer).is_none()
            {
                assert!(authorities.insert(peer));
                blocks.insert(block_ref);
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
                .expect("Should have found a non empty map");

            assert!(authorities.remove(&peer), "Peer index should be present!");

            // if the last one then just clean up
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
            FETCH_TRANSACTIONS_CONCURRENCY,
        );

        let mut tasks = JoinSet::new();

        // Spawn the live fetcher task
        let live_fetcher_async = Self::live_fetcher(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
            live_fetch_receiver,
            commands_sender.clone(),
            block_verifier.clone(),
            inflight_transactions_map.clone(),
        );
        tasks.spawn(monitored_future!(live_fetcher_async));

        let commands_sender_clone = commands_sender.clone();

        // Spawn the task to listen to the requests & periodic runs
        tasks.spawn(monitored_future!(async move {
            let mut s = Self {
                context,
                commands_receiver,
                live_fetch_requests: live_fetch_sender,
                core_dispatcher,
                fetch_transactions_scheduler_task: JoinSet::new(),
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
    async fn run(&mut self) {
        // We want the transactions synchronizer to run periodically every 500ms to
        // fetch any missing transactions.
        const TRANSACTIONS_SYNCHRONIZER_TIMEOUT: Duration = Duration::from_millis(500);
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
                    // we want to start a new task only if the previous one has already finished.
                    if self.fetch_transactions_scheduler_task.is_empty() {
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
        network_client: Arc<C>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        dag_state: Arc<RwLock<DagState>>,
        mut receiver: Receiver<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>,
        commands_sender: Sender<Command>,
        block_verifier: Arc<V>,
        inflight_transactions_map: Arc<InflightTransactionsMap>,
    ) {
        // TODO: limit number of live requests
        let mut requests = FuturesUnordered::new();

        loop {
            tokio::select! {
                Some(missing_block_refs) = receiver.recv(), if requests.len() < FETCH_TRANSACTIONS_CONCURRENCY => {
                    requests.push(Self::fetch_transactions_from_authorities(
                        context.clone(),
                        inflight_transactions_map.clone(),
                        network_client.clone(),
                        missing_block_refs,
                        "live",
                    ));
                },
                Some(result) = requests.next() => {
                    if result.is_empty() {
                        debug!("No results returned while requesting missing transactions");
                        continue;
                    }
                    // Add process_fetched_transactions futures to the FuturesUnordered collection
                    for (transactions_guard, fetched_transactions, peer) in result {

                       if let Err(err) = Self::process_fetched_transactions(
                            fetched_transactions,
                            peer,
                            transactions_guard,
                            core_dispatcher.clone(),
                            context.clone(),
                            commands_sender.clone(),
                            block_verifier.clone(),
                            dag_state.clone(),
                            "live",
                       )
                       .await
                       {
                           warn!(
                               "Error occurred while processing fetched blocks from peer {peer}: {err}"
                           );
                       }
                    };
                },
                else => {
                    info!("Live fetcher task will now abort.");
                    break;
                }
            }
        }
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
        commands_sender: Sender<Command>,
        block_verifier: Arc<V>,
        dag_state: Arc<RwLock<DagState>>,
        sync_method: &str,
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
                move || {
                    Self::verify_transactions(
                        serialized_transactions,
                        block_verifier,
                        peer_index,
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
        };

        metrics
            .transactions_synchronizer_fetched_transactions_by_peer
            .with_label_values(&[peer_hostname.as_str(), sync_method])
            .inc_by(transactions.len() as u64);
        for transactions in &transactions {
            let block_hostname = &context
                .committee
                .authority(transactions.block_ref().author)
                .hostname;
            metrics
                .transactions_synchronizer_fetched_transactions_by_authority
                .with_label_values(&[block_hostname.as_str(), sync_method])
                .inc();
        }

        info!(
            "[{sync_method}] Synced {} missing transactions from peer {peer_index} {peer_hostname}: {}",
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

        // Kick off the scheduler to fetch any remaining missing transactions
        commands_sender
            .try_send(Command::KickOffScheduler)
            .map_err(|_| ConsensusError::Shutdown)?;

        Ok(())
    }

    fn verify_transactions(
        serialized_transactions_bytes: Vec<Bytes>,
        block_verifier: Arc<V>,
        peer_index: AuthorityIndex,
        block_headers_map: BTreeMap<BlockRef, VerifiedBlockHeader>,
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
            if block_header.transactions_commitment()
                != TransactionsCommitment::compute_transactions_commitment(
                    &serialized_transactions.serialized_transactions,
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

    async fn fetch_transactions_request(
        network_client: Arc<C>,
        peer: AuthorityIndex,
        transactions_guard: TransactionsGuard,
        request_timeout: Duration,
        context: Arc<Context>,
        sync_method: &str,
    ) -> (
        ConsensusResult<Vec<Bytes>>,
        TransactionsGuard,
        AuthorityIndex,
    ) {
        let block_refs = transactions_guard
            .block_refs
            .iter()
            .cloned()
            .collect::<Vec<_>>();

        let peer_hostname = &context.committee.authority(peer).hostname;
        let start_time = Instant::now();
        // Update inflight requests metric
        context
            .metrics
            .node_metrics
            .transactions_synchronizer_inflight_requests
            .inc();
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
            .with_label_values(&[peer_hostname.as_str(), sync_method])
            .observe(fetch_duration.as_secs_f64());
        // Update inflight requests metric
        context
            .metrics
            .node_metrics
            .transactions_synchronizer_inflight_requests
            .dec();

        let resp = match result {
            Ok(Err(err)) => {
                // Record failure
                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_failure_by_peer
                    .with_label_values(&[peer_hostname.as_str(), sync_method, err.name()])
                    .inc();
                Err(err) // network error
            }
            Err(err) => {
                // Record timeout failure
                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_failure_by_peer
                    .with_label_values(&[peer_hostname.as_str(), sync_method, "timeout"])
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
                    .with_label_values(&[peer_hostname.as_str(), sync_method])
                    .inc();

                result
            }
        };
        (resp, transactions_guard, peer)
    }

    /// Starts a task to fetch missing transactions from other authorities.
    async fn start_fetch_missing_transactions_task(&mut self) -> ConsensusResult<()> {
        // Get missing transactions from the core
        let missing_transactions = self
            .core_dispatcher
            .get_missing_transaction_data()
            .await
            .map_err(|_err| ConsensusError::Shutdown)?;

        if missing_transactions.is_empty() {
            return Ok(());
        }
        let context = self.context.clone();
        let network_client = self.network_client.clone();
        let core_dispatcher = self.core_dispatcher.clone();
        let inflight_transactions_map = self.inflight_transactions_map.clone();
        let commands_sender = self.commands_sender.clone();
        let block_verifier = self.block_verifier.clone();
        let dag_state = self.dag_state.clone();

        self.fetch_transactions_scheduler_task
            .spawn(monitored_future!(async move {
                let _scope = monitored_scope("FetchMissingTransactionsScheduler");
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

                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_scheduler_inflight
                    .inc();
                let total_requested = missing_transactions.len();

                fail_point_async!("consensus-delay");
                // Fetch the missing transactions from other authorities
                let results = Self::fetch_transactions_from_authorities(
                    context.clone(),
                    inflight_transactions_map,
                    network_client,
                    missing_transactions,
                    "periodic",
                )
                .await;
                context
                    .metrics
                    .node_metrics
                    .transactions_synchronizer_scheduler_inflight
                    .dec();
                if results.is_empty() {
                    warn!("No results returned while requesting missing transactions");
                    return;
                }
                let mut total_fetched = 0;
                for (transactions_guard, fetched_transactions, peer) in results {
                    total_fetched += fetched_transactions.len();

                    if let Err(err) = Self::process_fetched_transactions(
                        fetched_transactions,
                        peer,
                        transactions_guard,
                        core_dispatcher.clone(),
                        context.clone(),
                        commands_sender.clone(),
                        block_verifier.clone(),
                        dag_state.clone(),
                        "periodic",
                    )
                    .await
                    {
                        warn!(
                            "Error occurred while processing fetched blocks from peer {peer}: {err}"
                        );
                    }
                }

                debug!(
                    "Total blocks requested to fetch: {}, total fetched: {}",
                    total_requested, total_fetched
                );
            }));
        Ok(())
    }

    /// Fetches missing transactions from other authorities.
    async fn fetch_transactions_from_authorities(
        context: Arc<Context>,
        inflight_transactions_map: Arc<InflightTransactionsMap>,
        network_client: Arc<C>,
        missing_transactions: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        sync_method: &str,
    ) -> Vec<(TransactionsGuard, Vec<Bytes>, AuthorityIndex)> {
        let mut rng = StdRng::from_rng(OsRng).expect("OsRng should be available");

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

        // let mut assigned_block_refs = BTreeSet::new();
        let mut request_futures = FuturesUnordered::new();

        // For each authority, randomize and try to lock up to
        // MAX_TRANSACTIONS_PER_FETCH acknowledged blocks.
        // The logic is as follows:
        // * Iterate all authorities that have acknowledged missing transactions.
        // * Randomly select up to MAX_TRANSACTIONS_PER_FETCH missing transactions
        //   acknowledged by the authority.
        // * Attempt to lock the selected transactions using the
        //   inflight_transactions_map. Some transactions may already be locked by other
        //   authorities, but continue with the transactions that were successfully
        //   locked. If no transactions can be locked and there are remaining missing
        //   transactions acknowledged by the authority, then try again with a new
        //   random selection. TODO: This part of the logic needs to be improved!
        // * If transactions are successfully locked, then send a request to the network
        //   client to fetch the transactions from the authority.
        for (authority, authority_block_refs) in blocks_by_authority {
            let peer_hostname = &context.committee.authority(authority).hostname;
            // Remove block_refs already assigned to another peer
            let mut available_refs: BTreeSet<_> = authority_block_refs.clone();
            // .difference(&assigned_block_refs)
            // .cloned()
            // .collect();
            while !available_refs.is_empty() {
                // TODO: remove this and use inflight_transactions_map to choose transactions
                //  randomly
                let selected_block_refs = available_refs
                    .iter()
                    .choose_multiple(&mut rng, MAX_TRANSACTIONS_PER_FETCH)
                    .into_iter()
                    .cloned()
                    .collect::<BTreeSet<_>>();

                if selected_block_refs.is_empty() {
                    break;
                }

                if let Some(transactions_guard) = inflight_transactions_map
                    .lock_transactions(selected_block_refs.clone(), authority)
                {
                    debug!(
                        "[{sync_method}] Syncing {} missing committed transactions from authority {} {}: {}",
                        selected_block_refs.len(),
                        authority,
                        peer_hostname,
                        selected_block_refs
                            .iter()
                            .map(|b| b.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    // assigned_block_refs.extend(&selected_block_refs);
                    // TODO: we need to measure how many requests are sent in each run and possibly
                    //  limit that. This can be limited after we implement Starfish with shards as
                    //  there will definitely be more requests perform.
                    request_futures.push(Self::fetch_transactions_request(
                        network_client.clone(),
                        authority,
                        transactions_guard,
                        FETCH_REQUEST_TIMEOUT,
                        context.clone(),
                        sync_method,
                    ));
                    break;
                } else {
                    // Remove attempted refs and try again with the rest
                    available_refs
                        .retain(|ref_to_remove| !selected_block_refs.contains(ref_to_remove));
                }
            }
        }

        let mut results = Vec::new();
        let fetcher_timeout = sleep(FETCH_FROM_PEERS_TIMEOUT);

        tokio::pin!(fetcher_timeout);

        // Track the number of concurrent requests
        context
            .metrics
            .node_metrics
            .transactions_synchronizer_inflight_requests
            .set(request_futures.len() as i64);

        loop {
            tokio::select! {
                Some((response, transactions_guard, peer_index)) = request_futures.next() => {
                    let peer_hostname = &context.committee.authority(peer_index).hostname;
                    match response {
                        Ok(fetched_blocks) => {
                            results.push((transactions_guard, fetched_blocks, peer_index));

                            // Update concurrent requests metric
                            context.metrics.node_metrics.transactions_synchronizer_inflight_requests
                                .set(request_futures.len() as i64);

                            // no more pending requests are left, break the loop
                            if request_futures.is_empty() {
                                break;
                            }
                        },
                        Err(e) => {
                            warn!("Error while trying to fetch blocks from peer {peer_index} {peer_hostname}. Error: {e}");
                            // we don't necessarily need to do, but dropping the guard here to unlock the blocks
                            drop(transactions_guard);

                            // Update concurrent requests metric
                            context.metrics.node_metrics.transactions_synchronizer_inflight_requests
                                .set(request_futures.len() as i64);
                        }
                    }
                },
                _ = &mut fetcher_timeout => {
                    debug!("Timed out while fetching missing blocks");
                    break;
                }
            }
        }

        results
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

        // Create some test transactions
        let block_round_author: Vec<(Round, u32)> = vec![(1, 1), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();
        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment =
                    TransactionsCommitment::compute_transactions_commitment(&serialized).unwrap();

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

        // Create block round author pairs
        let block_round_authors = (1..FETCH_TRANSACTIONS_CONCURRENCY * 2 + 1)
            .map(|i| (i as Round, 1u32))
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
            let commitment =
                TransactionsCommitment::compute_transactions_commitment(&serialized).unwrap();

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
        for _ in 0..FETCH_TRANSACTIONS_CONCURRENCY * 3 {
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
            FETCH_TRANSACTIONS_CONCURRENCY * 2,
            "Expected {} requests to succeed",
            FETCH_TRANSACTIONS_CONCURRENCY * 2
        );
        assert_eq!(
            saturated, FETCH_TRANSACTIONS_CONCURRENCY,
            "Expected {FETCH_TRANSACTIONS_CONCURRENCY} requests to be saturated"
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

        // Create some test transactions
        let block_round_author: Vec<(Round, u32)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment =
                    TransactionsCommitment::compute_transactions_commitment(&serialized).unwrap();

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

        // Create some test transactions
        let block_round_author: Vec<(Round, u32)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment =
                    TransactionsCommitment::compute_transactions_commitment(&serialized).unwrap();

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

        // Create some test transactions
        let block_round_author: Vec<(Round, u32)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment =
                    TransactionsCommitment::compute_transactions_commitment(&serialized).unwrap();

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

        // Create some test transactions
        let block_round_author: Vec<(Round, u32)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        let transactions = block_round_author
            .into_iter()
            .map(|(round, author)| {
                // Create a dummy transaction
                let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
                let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
                let commitment =
                    TransactionsCommitment::compute_transactions_commitment(&serialized).unwrap();

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

        // Create some test transactions
        let block_round_author: Vec<(Round, u32)> = vec![(1, 0), (2, 1), (3, 2)];

        let mut block_headers = Vec::with_capacity(block_round_author.len());

        let mut rng = thread_rng();

        // Create verified transactions
        for (round, author) in &block_round_author {
            // Create a dummy transaction
            let transactions = vec![Transaction::new((0..32).map(|_| rng.gen()).collect())];
            let serialized = Bytes::from(bcs::to_bytes(&transactions).unwrap());
            let commitment =
                TransactionsCommitment::compute_transactions_commitment(&serialized).unwrap();

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

    #[test]
    fn inflight_transactions_map() {
        telemetry_subscribers::init_for_testing();
        // GIVEN
        let map = InflightTransactionsMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
            BlockRef::new(12, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
            BlockRef::new(15, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
        ];
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // Lock & unlock transactions
        {
            let mut all_guards = Vec::new();

            // Try to acquire the transaction locks for authorities 1 & 2
            for i in 1..=2 {
                let authority = AuthorityIndex::new_for_test(i);

                let guard = map.lock_transactions(missing_block_refs.clone(), authority);
                let guard = guard.expect("Guard should be created");
                assert_eq!(guard.block_refs.len(), 4);

                all_guards.push(guard);

                // trying to acquire any of them again will not succeed
                let guard = map.lock_transactions(missing_block_refs.clone(), authority);
                assert!(guard.is_none());
            }

            // Trying to acquire for authority 3 it will fail - as we have maxed out the
            // number of allowed peers (MAX_AUTHORITIES_TO_FETCH_PER_TRANSACTION = 2)
            let authority_3 = AuthorityIndex::new_for_test(3);

            let guard = map.lock_transactions(missing_block_refs.clone(), authority_3);
            assert!(guard.is_none());

            // Explicitly drop the guard of authority 1 and try for authority 3 again - it
            // will now succeed
            drop(all_guards.remove(0));

            let guard = map.lock_transactions(missing_block_refs.clone(), authority_3);
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

        async fn get_missing_blocks(
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
