// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    num::NonZeroUsize,
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
use lru::LruCache;
use parking_lot::{Mutex, RwLock};
#[cfg(not(test))]
use rand::{
    SeedableRng,
    prelude::{IteratorRandom, SliceRandom, StdRng},
};
use starfish_config::AuthorityIndex;
use tap::TapFallible;
use tokio::{
    runtime::Handle,
    sync::{mpsc::error::TrySendError, oneshot},
    task::{JoinError, JoinSet},
    time::{Instant, sleep, sleep_until, timeout},
};
use tracing::{debug, error, info, trace, warn};

use crate::{
    CommitIndex, Round,
    authority_service::COMMIT_LAG_MULTIPLIER,
    block_header::{
        BlockHeaderAPI, BlockHeaderDigest, BlockRef, GENESIS_ROUND, SignedBlockHeader,
        VerifiedBlockHeader,
    },
    block_verifier::BlockVerifier,
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::{DagState, DataSource},
    error::{ConsensusError, ConsensusResult},
    network::NetworkClient,
    transactions_synchronizer::TransactionsSynchronizerHandle,
};

/// The number of concurrent fetch block headers requests per authority
const FETCH_BLOCK_HEADERS_CONCURRENCY: usize = 5;

/// /// The timeout for synchronizer to fetch blocks from a given peer
/// authority.
const FETCH_REQUEST_TIMEOUT: Duration = Duration::from_millis(2_000);

/// The timeout for periodic synchronizer to fetch blocks from the peers.
const FETCH_FROM_PEERS_TIMEOUT: Duration = Duration::from_millis(4_000);

/// The maximum number of authorities from which we will try to periodically
/// fetch block header at the same moment. The guard will protect that we will
/// not ask from more than this number of authorities at the same time.
const MAX_AUTHORITIES_TO_FETCH_PER_BLOCK_HEADER: usize = 3;

/// The maximum number of authorities from which the live synchronizer will try
/// to fetch block headers at the same moment. This is lower than the periodic
/// sync limit to prioritize periodic sync.
const MAX_AUTHORITIES_TO_LIVE_FETCH_PER_BLOCK_HEADER: usize = 1;

/// The maximum number of peers from which the periodic synchronizer will
/// request block headers.
const MAX_PERIODIC_SYNC_PEERS: usize = 4;

/// The maximum number of peers in the periodic synchronizer which are chosen
/// totally random to fetch block headers from. The other peers will be chosen
/// based on their knowledge of the DAG.
const MAX_PERIODIC_SYNC_RANDOM_PEERS: usize = 2;

/// The maximum number of verified block header references to cache for
/// deduplication.
const VERIFIED_HEADERS_CACHE_CAP: usize = 200_000;

/// Represents the different methods used for synchronization
#[derive(Debug, Clone, Copy, Ord, Eq, PartialOrd, PartialEq)]
pub(crate) enum SyncMethod {
    Live,
    Periodic,
}

impl std::fmt::Display for SyncMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncMethod::Live => write!(f, "live"),
            SyncMethod::Periodic => write!(f, "periodic"),
        }
    }
}

impl SyncMethod {
    fn max_authorities_to_fetch_per_block_header(&self) -> usize {
        match self {
            SyncMethod::Live => MAX_AUTHORITIES_TO_LIVE_FETCH_PER_BLOCK_HEADER,
            SyncMethod::Periodic => MAX_AUTHORITIES_TO_FETCH_PER_BLOCK_HEADER,
        }
    }
}

struct BlocksGuard {
    map: Arc<InflightBlockHeadersMap>,
    block_refs: BTreeSet<BlockRef>,
    peer: AuthorityIndex,
    method: SyncMethod,
}

impl Drop for BlocksGuard {
    fn drop(&mut self) {
        self.map.unlock_headers(&self.block_refs, self.peer);
    }
}

// Keeps a mapping between the missing headers that have been instructed to be
// fetched and the authorities that are currently fetching them. For a block ref
// there is a maximum number of authorities that can concurrently fetch it. The
// authority ids that are currently fetching a block are set on the
// corresponding `BTreeSet` and basically they act as "locks".
struct InflightBlockHeadersMap {
    inner: Mutex<HashMap<BlockRef, BTreeSet<AuthorityIndex>>>,
}

impl InflightBlockHeadersMap {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(HashMap::new()),
        })
    }

    /// Locks the block headers to be fetched for the assigned `peer_index`. We
    /// want to avoid re-fetching the missing block headers from too many
    /// authorities at the same time, thus we limit the concurrency per
    /// block by attempting to lock per block. If a header is already
    /// fetched by the maximum allowed number of authorities, then the block
    /// ref will not be included in the returned set. The method returns all
    /// the block refs that have been successfully locked and allowed to be
    /// fetched.
    ///
    /// Different limits apply based on the sync method:
    /// - Periodic sync: Can lock if total authorities <
    ///   MAX_AUTHORITIES_TO_FETCH_PER_BLOCK_HEADER (3)
    /// - Live sync: Can lock if total authorities <
    ///   MAX_AUTHORITIES_TO_LIVE_FETCH_PER_BLOCK_HEADER (1)
    fn lock_headers(
        self: &Arc<Self>,
        missing_block_refs: BTreeSet<BlockRef>,
        peer: AuthorityIndex,
        method: SyncMethod,
    ) -> Option<BlocksGuard> {
        let mut block_refs = BTreeSet::new();
        let mut inner = self.inner.lock();

        for block_ref in missing_block_refs {
            let authorities = inner.entry(block_ref).or_default();

            // Check if this peer is already fetching this header
            if authorities.contains(&peer) {
                continue;
            }

            // Count total authorities currently fetching this header
            let total_count = authorities.len();

            // Determine the limit based on the sync method
            let max_limit = method.max_authorities_to_fetch_per_block_header();

            // Check if we can acquire the lock
            if total_count < max_limit {
                assert!(authorities.insert(peer));
                block_refs.insert(block_ref);
            }
        }

        if block_refs.is_empty() {
            None
        } else {
            Some(BlocksGuard {
                map: self.clone(),
                block_refs,
                peer,
                method,
            })
        }
    }

    /// Unlocks the provided block references for the given `peer`. The
    /// unlocking is strict, meaning that if this method is called for a
    /// specific block ref and peer more times than the corresponding lock
    /// has been called, it will panic.
    fn unlock_headers(self: &Arc<Self>, block_refs: &BTreeSet<BlockRef>, peer: AuthorityIndex) {
        // Now mark all the blocks as fetched from the map
        let mut headers_to_fetch = self.inner.lock();
        for block_ref in block_refs {
            let authorities = headers_to_fetch
                .get_mut(block_ref)
                .expect("Should have found a non empty map");

            assert!(authorities.remove(&peer), "Peer index should be present!");

            // if the last one then just clean up
            if authorities.is_empty() {
                headers_to_fetch.remove(block_ref);
            }
        }
    }

    /// Drops the provided `blocks_guard` which will force to unlock the blocks,
    /// and lock now again the referenced block refs. The swap is best
    /// effort and there is no guarantee that the `peer` will be able to
    /// acquire the new locks.
    fn swap_locks(
        self: &Arc<Self>,
        blocks_guard: BlocksGuard,
        peer: AuthorityIndex,
    ) -> Option<BlocksGuard> {
        let block_refs = blocks_guard.block_refs.clone();
        let method = blocks_guard.method;

        // Explicitly drop the guard
        drop(blocks_guard);

        // Now create a new guard with the same sync method
        self.lock_headers(block_refs, peer, method)
    }

    #[cfg(test)]
    fn num_of_locked_headers(self: &Arc<Self>) -> usize {
        let inner = self.inner.lock();
        inner.len()
    }
}

enum Command {
    FetchBlockHeaders {
        missing_block_refs: BTreeSet<BlockRef>,
        peer_index: AuthorityIndex,
        result: oneshot::Sender<Result<(), ConsensusError>>,
    },
    FetchOwnLastBlockHeader,
    KickOffScheduler,
}

pub(crate) struct HeaderSynchronizerHandle {
    commands_sender: Sender<Command>,
    tasks: tokio::sync::Mutex<JoinSet<()>>,
}

impl HeaderSynchronizerHandle {
    /// Explicitly asks from the synchronizer to fetch block headers - provided
    /// the block_refs set - from the peer authority.
    pub(crate) async fn fetch_headers(
        &self,
        missing_block_refs: BTreeSet<BlockRef>,
        peer_index: AuthorityIndex,
    ) -> ConsensusResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.commands_sender
            .send(Command::FetchBlockHeaders {
                missing_block_refs,
                peer_index,
                result: sender,
            })
            .await
            .map_err(|_err| ConsensusError::Shutdown)?;
        receiver.await.map_err(|_err| ConsensusError::Shutdown)?
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

/// `Synchronizer` oversees live block synchronization, crucial for node
/// progress. Live synchronization refers to the process of retrieving missing
/// blocks, particularly those essential for advancing a node when data from
/// only a few rounds is absent. If a node significantly lags behind the
/// network, `commit_syncer` handles fetching missing headers via a more
/// efficient approach. Transactions that are sequenced and included in commit
/// are fetched through the transaction synchronizer once block headers are
/// processed. `Synchronizer` aims for swift catch-up employing two
/// mechanisms:
///
/// 1. Explicitly requesting missing headers from designated authorities via the
///    bundle streaming path. This includes attempting to fetch any missing
///    ancestors necessary for processing a received bundle of block and
///    headers. Such requests prioritize the block author, maximizing the chance
///    of prompt retrieval. A locking mechanism allows concurrent requests for
///    missing blocks from up to three authorities simultaneously, enhancing the
///    chances of timely retrieval. Notably, if additional missing blocks arise
///    during block processing, requests are deferred to the scheduler.
///
/// 2. Periodically requesting missing block headers via a scheduler. This
///    primarily serves to retrieve missing headers that were not ancestors of a
///    received block bundle via the bundle streaming path. The scheduler
///    operates on either a fixed periodic basis or is triggered immediately
///    after explicit fetches described in (1), ensuring continued block
///    retrieval if gaps persist.
///
/// Additionally to the above, the synchronizer can synchronize and fetch the
/// last own proposed header from the network peers as best effort approach to
/// recover node from amnesia and avoid making the node equivocate.
pub(crate) struct HeaderSynchronizer<C: NetworkClient, V: BlockVerifier, D: CoreThreadDispatcher> {
    context: Arc<Context>,
    commands_receiver: Receiver<Command>,
    fetch_block_senders: BTreeMap<AuthorityIndex, Sender<BlocksGuard>>,
    core_dispatcher: Arc<D>,
    commit_vote_monitor: Arc<CommitVoteMonitor>,
    dag_state: Arc<RwLock<DagState>>,
    transactions_synchronizer: Arc<TransactionsSynchronizerHandle>,
    fetch_block_headers_scheduler_task: JoinSet<()>,
    fetch_own_last_header_task: JoinSet<()>,
    network_client: Arc<C>,
    block_verifier: Arc<V>,
    inflight_block_headers_map: Arc<InflightBlockHeadersMap>,
    verified_headers_cache: Arc<Mutex<LruCache<BlockHeaderDigest, ()>>>,
    commands_sender: Sender<Command>,
}

impl<C: NetworkClient, V: BlockVerifier, D: CoreThreadDispatcher> HeaderSynchronizer<C, V, D> {
    /// Starts the synchronizer, which is responsible for fetching block headers
    /// from other authorities and managing block synchronization tasks.
    pub fn start(
        network_client: Arc<C>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        commit_vote_monitor: Arc<CommitVoteMonitor>,
        transactions_synchronizer: Arc<TransactionsSynchronizerHandle>,
        block_verifier: Arc<V>,
        dag_state: Arc<RwLock<DagState>>,
        sync_last_known_own_block: bool,
    ) -> Arc<HeaderSynchronizerHandle> {
        let (commands_sender, commands_receiver) =
            channel("consensus_synchronizer_commands", 1_000);
        let inflight_block_headers_map = InflightBlockHeadersMap::new();
        let verified_headers_cache = Arc::new(Mutex::new(LruCache::new(
            NonZeroUsize::new(VERIFIED_HEADERS_CACHE_CAP).unwrap(),
        )));

        // Spawn the tasks to fetch the blocks from the others
        let mut fetch_block_senders = BTreeMap::new();
        let mut tasks = JoinSet::new();
        for (index, _) in context.committee.authorities() {
            if index == context.own_index {
                continue;
            }
            let (sender, receiver) = channel(
                "consensus_synchronizer_fetches",
                FETCH_BLOCK_HEADERS_CONCURRENCY,
            );
            let fetch_blocks_from_authority_async = Self::fetch_headers_from_authority_live(
                index,
                network_client.clone(),
                block_verifier.clone(),
                verified_headers_cache.clone(),
                transactions_synchronizer.clone(),
                commit_vote_monitor.clone(),
                context.clone(),
                core_dispatcher.clone(),
                dag_state.clone(),
                receiver,
                commands_sender.clone(),
            );
            tasks.spawn(monitored_future!(fetch_blocks_from_authority_async));
            fetch_block_senders.insert(index, sender);
        }

        let commands_sender_clone = commands_sender.clone();

        if sync_last_known_own_block {
            commands_sender
                .try_send(Command::FetchOwnLastBlockHeader)
                .expect("Failed to sync our last block");
        }

        // Spawn the task to listen to the requests & periodic runs
        tasks.spawn(monitored_future!(async move {
            let mut s = Self {
                context,
                commands_receiver,
                fetch_block_senders,
                core_dispatcher,
                commit_vote_monitor,
                fetch_block_headers_scheduler_task: JoinSet::new(),
                fetch_own_last_header_task: JoinSet::new(),
                transactions_synchronizer,
                network_client,
                block_verifier,
                inflight_block_headers_map,
                verified_headers_cache,
                commands_sender: commands_sender_clone,
                dag_state,
            };
            s.run().await;
        }));

        Arc::new(HeaderSynchronizerHandle {
            commands_sender,
            tasks: tokio::sync::Mutex::new(tasks),
        })
    }

    // The main loop to listen for the submitted commands.
    #[cfg_attr(test,tracing::instrument(skip_all, name ="",fields(authority = %self.context.own_index)))]
    async fn run(&mut self) {
        // We want the synchronizer to run periodically every 200ms to fetch any missing
        // blocks.
        const PERIODIC_SYNCHRONIZER_TIMEOUT: Duration = Duration::from_millis(200);
        let scheduler_timeout = sleep_until(Instant::now() + PERIODIC_SYNCHRONIZER_TIMEOUT);

        tokio::pin!(scheduler_timeout);

        loop {
            tokio::select! {
                Some(command) = self.commands_receiver.recv() => {
                    match command {
                        Command::FetchBlockHeaders{ missing_block_refs, peer_index, result } => {
                            if peer_index == self.context.own_index {
                                error!("We should never attempt to fetch block headers from our own node");
                                continue;
                            }

                            // Keep only the max allowed blocks to request. It is ok to reduce here as the scheduler
                            // task will take care syncing whatever is leftover.
                            let missing_block_refs = missing_block_refs
                                .into_iter()
                                .take(self.context.parameters.max_headers_per_regular_sync_fetch)
                                .collect();

                            let blocks_guard = self.inflight_block_headers_map.lock_headers(missing_block_refs, peer_index, SyncMethod::Live);
                            let Some(blocks_guard) = blocks_guard else {
                                result.send(Ok(())).ok();
                                continue;
                            };

                            // We don't block if the corresponding peer task is saturated - but we rather drop the request. That's ok as the periodic
                            // synchronization task will handle any still missing blocks in next run.
                            let r = self
                                .fetch_block_senders
                                .get(&peer_index)
                                .expect("Fatal error, sender should be present")
                                .try_send(blocks_guard)
                                .map_err(|err| {
                                    match err {
                                        TrySendError::Full(_) => ConsensusError::SynchronizerSaturated(peer_index),
                                        TrySendError::Closed(_) => ConsensusError::Shutdown
                                    }
                                });

                            result.send(r).ok();
                        }
                        Command::FetchOwnLastBlockHeader => {
                            if self.fetch_own_last_header_task.is_empty() {
                                self.start_fetch_own_last_block_header_task();
                            }
                        }
                        Command::KickOffScheduler => {
                            // just reset the scheduler timeout timer to run immediately if not already running.
                            // If the scheduler is already running then just reduce the remaining time to run.
                            let timeout = if self.fetch_block_headers_scheduler_task.is_empty() {
                                Instant::now()
                            } else {
                                Instant::now() + PERIODIC_SYNCHRONIZER_TIMEOUT.checked_div(2).unwrap()
                            };

                            // only reset if it is earlier than the next deadline
                            if timeout < scheduler_timeout.deadline() {
                                scheduler_timeout.as_mut().reset(timeout);
                            }
                        }
                    }
                },
                Some(result) = self.fetch_own_last_header_task.join_next(), if !self.fetch_own_last_header_task.is_empty() => {
                    match result {
                        Ok(()) => {},
                        Err(e) => {
                            if e.is_cancelled() {
                            } else if e.is_panic() {
                                std::panic::resume_unwind(e.into_panic());
                            } else {
                                panic!("fetch our last block task failed: {e}");
                            }
                        },
                    };
                },
                Some(result) = self.fetch_block_headers_scheduler_task.join_next(), if !self.fetch_block_headers_scheduler_task.is_empty() => {
                    match result {
                        Ok(()) => {},
                        Err(e) => {
                            if e.is_cancelled() {
                            } else if e.is_panic() {
                                std::panic::resume_unwind(e.into_panic());
                            } else {
                                panic!("fetch blocks scheduler task failed: {e}");
                            }
                        },
                    };
                },
                () = &mut scheduler_timeout => {
                    // we want to start a new task only if the previous one has already finished.
                    if self.fetch_block_headers_scheduler_task.is_empty() {
                        if let Err(err) = self.start_periodic_sync_task().await {
                            debug!("Core is shutting down, synchronizer is shutting down: {err:?}");
                            return;
                        };
                    }

                    scheduler_timeout
                        .as_mut()
                        .reset(Instant::now() + PERIODIC_SYNCHRONIZER_TIMEOUT);
                }
            }
        }
    }

    async fn fetch_headers_from_authority_live(
        peer_index: AuthorityIndex,
        network_client: Arc<C>,
        block_verifier: Arc<V>,
        verified_cache: Arc<Mutex<LruCache<BlockHeaderDigest, ()>>>,
        transactions_synchronizer: Arc<TransactionsSynchronizerHandle>,
        commit_vote_monitor: Arc<CommitVoteMonitor>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        dag_state: Arc<RwLock<DagState>>,
        mut receiver: Receiver<BlocksGuard>,
        commands_sender: Sender<Command>,
    ) {
        const MAX_RETRIES: u32 = 3;
        let peer_hostname = &context.committee.authority(peer_index).hostname;

        let mut requests = FuturesUnordered::new();

        loop {
            tokio::select! {
                Some(headers_guard) = receiver.recv(), if requests.len() < FETCH_BLOCK_HEADERS_CONCURRENCY => {
                    // get the highest accepted rounds
                    let highest_rounds = Self::get_highest_accepted_rounds(dag_state.clone(), &context);

                    // Record metrics for live synchronizer requests
                    let metrics = &context.metrics.node_metrics;
                    metrics
                        .synchronizer_requested_block_headers_by_peer
                        .with_label_values(&[peer_hostname.as_str(), "live"])
                        .inc_by(headers_guard.block_refs.len() as u64);
                    // Count requested blocks per authority and increment metric by one per authority
                    let mut authors = HashSet::new();
                    for block_ref in &headers_guard.block_refs {
                        authors.insert(block_ref.author);
                    }
                    for author in authors {
                        let host = &context.committee.authority(author).hostname;
                        metrics
                            .synchronizer_requested_block_headers_by_authority
                            .with_label_values(&[host.as_str(), "live"])
                            .inc();
                    }

                    requests.push(Self::fetch_block_headers_request(
                        network_client.clone(),
                        peer_index,
                        headers_guard,
                        highest_rounds,
                        FETCH_REQUEST_TIMEOUT,
                        1,
                    ))

                },
                Some((response, blocks_guard, retries, _peer, highest_rounds)) = requests.next() => {
                    match response {
                        Ok(blocks) => {
                            if let Err(err) = Self::process_fetched_headers_from_authority(blocks,
                                peer_index,
                                blocks_guard,
                                core_dispatcher.clone(),
                                block_verifier.clone(),
                                verified_cache.clone(),
                                commit_vote_monitor.clone(),
                                transactions_synchronizer.clone(),
                                context.clone(),
                                commands_sender.clone(),
                                "live"
                            ).await {
                                warn!("Error while processing fetched blocks from peer {peer_index} {peer_hostname}: {err}");
                                context.metrics.node_metrics.synchronizer_process_fetched_failures_by_peer.with_label_values(&[peer_hostname.as_str(), "live"]).inc();
                            }
                        },
                        Err(_) => {
                            context.metrics.node_metrics.synchronizer_fetch_failures_by_peer.with_label_values(&[peer_hostname.as_str(), "live"]).inc();
                            if retries <= MAX_RETRIES {
                                requests.push(Self::fetch_block_headers_request(network_client.clone(), peer_index, blocks_guard, highest_rounds, FETCH_REQUEST_TIMEOUT, retries))
                            } else {
                                warn!("Max retries {retries} reached while trying to fetch blocks from peer {peer_index} {peer_hostname}.");
                                // we don't necessarily need to do, but dropping the guard here to unlock the blocks
                                drop(blocks_guard);
                            }
                        }
                    }
                },
                else => {
                    info!("Fetching blocks from authority {peer_index} task will now abort.");
                    break;
                }
            }
        }
    }

    /// Processes the requested raw fetched headers from peer `peer_index`. If
    /// no error is returned then the verified blocks are immediately sent
    /// to Core for processing.
    async fn process_fetched_headers_from_authority(
        mut serialized_headers: Vec<Bytes>,
        peer_index: AuthorityIndex,
        requested_blocks_guard: BlocksGuard,
        core_dispatcher: Arc<D>,
        block_verifier: Arc<V>,
        verified_cache: Arc<Mutex<LruCache<BlockHeaderDigest, ()>>>,
        commit_vote_monitor: Arc<CommitVoteMonitor>,
        transactions_synchronizer: Arc<TransactionsSynchronizerHandle>,
        context: Arc<Context>,
        commands_sender: Sender<Command>,
        sync_method: &str,
    ) -> ConsensusResult<()> {
        if serialized_headers.is_empty() {
            return Ok(());
        }
        let _s = context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Synchronizer::process_fetched_blocks"])
            .start_timer();
        if serialized_headers.len() > context.parameters.max_headers_per_regular_sync_fetch {
            debug!(
                "Truncating fetched headers from peer {} to max allowed {} blocks",
                peer_index, context.parameters.max_headers_per_regular_sync_fetch
            );
            serialized_headers.truncate(context.parameters.max_headers_per_regular_sync_fetch);
        }

        // Verify all the fetched block headers
        let block_headers = Handle::current()
            .spawn_blocking({
                let block_verifier = block_verifier.clone();
                let verified_cache = verified_cache.clone();
                let context = context.clone();
                let sync_method = sync_method.to_string();
                move || {
                    Self::verify_block_headers(
                        serialized_headers,
                        block_verifier,
                        verified_cache,
                        &context,
                        peer_index,
                        &sync_method,
                    )
                }
            })
            .await
            .expect("Spawn blocking should not fail")?;

        // Record commit votes from the verified blocks.
        for block in &block_headers {
            commit_vote_monitor.observe_block(block);
        }

        let metrics = &context.metrics.node_metrics;
        let peer_hostname = &context.committee.authority(peer_index).hostname;
        metrics
            .synchronizer_fetched_block_headers_by_peer
            .with_label_values(&[peer_hostname.as_str(), sync_method])
            .inc_by(block_headers.len() as u64);
        for block_header in &block_headers {
            let block_header_hostname =
                &context.committee.authority(block_header.author()).hostname;
            metrics
                .synchronizer_fetched_block_headers_by_authority
                .with_label_values(&[block_header_hostname.as_str(), sync_method])
                .inc();
        }

        debug!(
            "Synced {} missing blocks from peer {peer_index} {peer_hostname}: {}",
            block_headers.len(),
            block_headers
                .iter()
                .map(|b| b.reference().to_string())
                .join(", "),
        );

        // Now send them to core for processing. Ignore the returned missing blocks as
        // we don't want this mechanism to keep feedback looping on fetching
        // more blocks. The periodic synchronization will take care of that.
        let (missing_blocks, missing_committed_txns) = core_dispatcher
            .add_block_headers(block_headers, DataSource::HeaderSynchronizer)
            .await
            .map_err(|_| ConsensusError::Shutdown)?;

        // now release all the locked blocks as they have been fetched, verified &
        // processed
        drop(requested_blocks_guard);

        // kick off immediately the scheduled synchronizer
        if !missing_blocks.is_empty() {
            // do not block here, so we avoid any possible cycles.
            if let Err(TrySendError::Full(_)) = commands_sender.try_send(Command::KickOffScheduler)
            {
                warn!("Commands channel is full")
            }
        }

        context
            .metrics
            .node_metrics
            .missing_block_headers_after_fetch_total
            .inc_by(missing_blocks.len() as u64);

        if !missing_committed_txns.is_empty() {
            debug!(
                "Missing committed transactions after fetching blocks: {:?}",
                missing_committed_txns
            );
            if let Err(err) = transactions_synchronizer
                .fetch_transactions(missing_committed_txns)
                .await
            {
                warn!(
                    "Error while trying to fetch missing transactions via transactions synchronizer: {err}"
                );
            }
        }

        Ok(())
    }

    fn get_highest_accepted_rounds(
        dag_state: Arc<RwLock<DagState>>,
        context: &Arc<Context>,
    ) -> Vec<Round> {
        let block_headers = dag_state
            .read()
            .get_last_cached_block_header_per_authority(Round::MAX);
        assert_eq!(block_headers.len(), context.committee.size());

        block_headers
            .into_iter()
            .map(|(block, _)| block.round())
            .collect::<Vec<_>>()
    }

    fn verify_block_headers(
        serialized_block_headers: Vec<Bytes>,
        block_verifier: Arc<V>,
        verified_cache: Arc<Mutex<LruCache<BlockHeaderDigest, ()>>>,
        context: &Context,
        peer_index: AuthorityIndex,
        sync_method: &str,
    ) -> ConsensusResult<Vec<VerifiedBlockHeader>> {
        let mut verified_block_headers = Vec::new();
        let mut skipped_count = 0u64;

        for serialized_block_header in serialized_block_headers {
            let block_header_digest = VerifiedBlockHeader::compute_digest(&serialized_block_header);
            // Check if this block header has already been verified
            if verified_cache.lock().get(&block_header_digest).is_some() {
                skipped_count += 1;
                continue; // Skip already verified block headers
            }

            let signed_block_header: SignedBlockHeader = bcs::from_bytes(&serialized_block_header)
                .map_err(ConsensusError::MalformedHeader)?;

            if let Err(e) = block_verifier.verify(&signed_block_header) {
                // TODO: we might want to use a different metric to track the invalid "served"
                // blocks from the invalid "proposed" ones.
                let hostname = context.committee.authority(peer_index).hostname.clone();

                context
                    .metrics
                    .node_metrics
                    .synchronizer_invalid_block_headers
                    .with_label_values(&[hostname.as_str(), "synchronizer", e.name()])
                    .inc();
                warn!("Invalid block received from {}: {}", peer_index, e);
                return Err(e);
            }

            // Add block header to verified cache after successful verification
            verified_cache.lock().put(block_header_digest, ());

            let verified_block_header = VerifiedBlockHeader::new_verified_with_digest(
                signed_block_header,
                serialized_block_header,
                block_header_digest,
            );

            // Dropping is ok because the block will be refetched.
            // TODO: improve efficiency, maybe suspend and continue processing the block
            // asynchronously.
            let now = context.clock.timestamp_utc_ms();
            if now < verified_block_header.timestamp_ms() {
                warn!(
                    "Synced block {} timestamp {} is in the future (now={}). Ignoring.",
                    verified_block_header.reference(),
                    verified_block_header.timestamp_ms(),
                    now
                );
                continue;
            }

            verified_block_headers.push(verified_block_header);
        }

        // Record skipped block headers metric
        if skipped_count > 0 {
            let peer_hostname = &context.committee.authority(peer_index).hostname;
            context
                .metrics
                .node_metrics
                .synchronizer_skipped_block_headers_by_peer
                .with_label_values(&[peer_hostname.as_str(), sync_method])
                .inc_by(skipped_count);
        }

        Ok(verified_block_headers)
    }

    async fn fetch_block_headers_request(
        network_client: Arc<C>,
        peer: AuthorityIndex,
        blocks_guard: BlocksGuard,
        highest_rounds: Vec<Round>,
        request_timeout: Duration,
        mut retries: u32,
    ) -> (
        ConsensusResult<Vec<Bytes>>,
        BlocksGuard,
        u32,
        AuthorityIndex,
        Vec<Round>,
    ) {
        let start = Instant::now();
        let resp = timeout(
            request_timeout,
            network_client.fetch_block_headers(
                peer,
                blocks_guard
                    .block_refs
                    .clone()
                    .into_iter()
                    .collect::<Vec<_>>(),
                highest_rounds.clone(),
                request_timeout,
            ),
        )
        .await;

        fail_point_async!("consensus-delay");

        let resp = match resp {
            Ok(Err(err)) => {
                // Add a delay before retrying - if that is needed. If request has timed out
                // then eventually this will be a no-op.
                sleep_until(start + request_timeout).await;
                retries += 1;
                Err(err)
            } // network error
            Err(err) => {
                // timeout
                sleep_until(start + request_timeout).await;
                retries += 1;
                Err(ConsensusError::NetworkRequestTimeout(err.to_string()))
            }
            Ok(result) => result,
        };
        (resp, blocks_guard, retries, peer, highest_rounds)
    }
    #[cfg_attr(test,tracing::instrument(skip_all, name ="",fields(authority = %self.context.own_index)))]
    fn start_fetch_own_last_block_header_task(&mut self) {
        const FETCH_OWN_BLOCK_HEADER_RETRY_DELAY: Duration = Duration::from_millis(1_000);
        const MAX_RETRY_DELAY_STEP: Duration = Duration::from_millis(4_000);

        let context = self.context.clone();
        let dag_state = self.dag_state.clone();
        let network_client = self.network_client.clone();
        let block_verifier = self.block_verifier.clone();
        let core_dispatcher = self.core_dispatcher.clone();

        self.fetch_own_last_header_task
            .spawn(monitored_future!(async move {
                let _scope = monitored_scope("FetchOwnLastBlockHeaderTask");

                let fetch_own_block_header = |authority_index: AuthorityIndex, fetch_own_block_header_delay: Duration| {
                    let network_client_cloned = network_client.clone();
                    let own_index = context.own_index;
                    async move {
                        sleep(fetch_own_block_header_delay).await;
                        let r = network_client_cloned.fetch_latest_block_headers(authority_index, vec![own_index], FETCH_REQUEST_TIMEOUT).await;
                        (r, authority_index)
                    }
                };

                let process_block_headers = |block_headers: Vec<Bytes>, authority_index: AuthorityIndex| -> ConsensusResult<Vec<VerifiedBlockHeader >> {
                                    let mut result = Vec::new();
                                    for serialized_block_header in block_headers {
                                        let signed_block_header = bcs::from_bytes(&serialized_block_header).map_err(ConsensusError::MalformedHeader)?;
                                        block_verifier.verify(&signed_block_header).tap_err(|err|{
                                            let hostname = context.committee.authority(authority_index).hostname.clone();
                                            context
                                                .metrics
                                                .node_metrics
                                                .synchronizer_invalid_block_headers
                                                .with_label_values(&[hostname.as_str(), "synchronizer_own_block_header", err.clone().name()])
                                                .inc();
                                            warn!("Invalid block header received from {}: {}", authority_index, err);
                                        })?;

                                        let verified_block_header = VerifiedBlockHeader::new_verified(signed_block_header, serialized_block_header);
                                        if verified_block_header.author() != context.own_index {
                                            return Err(ConsensusError::UnexpectedLastOwnHeader { index: authority_index, block_ref: verified_block_header.reference()});
                                        }
                                        result.push(verified_block_header);
                                    }
                                    Ok(result)
                };

                // Get the highest of all the results. Retry until at least `2f+1` results have been gathered.
                let mut highest_round = GENESIS_ROUND;
                // Keep track of the received responses to avoid fetching the own block header from same peer
                let mut received_response = vec![false; context.committee.size()];
                // Assume that our node is not Byzantine
                received_response[context.own_index] = true;
                let mut total_stake = context.committee.stake(context.own_index);
                let mut retries = 0;
                let mut retry_delay_step = Duration::from_millis(500);
                'main:loop {
                    if context.committee.size() == 1 {
                        highest_round = dag_state.read().get_last_proposed_block_header().round();
                        info!("Only one node in the network, will not try fetching own last block header from peers.");
                        break 'main;
                    }

                    // Ask all the other peers about our last block header
                    let mut results = FuturesUnordered::new();

                    for (authority_index, _authority) in context.committee.authorities() {
                        // Skip our own index and the ones that have already responded
                        if !received_response[authority_index] {
                            results.push(fetch_own_block_header(authority_index, Duration::from_millis(0)));
                        }
                    }

                    // Gather the results but wait to timeout as well
                    let timer = sleep_until(Instant::now() + context.parameters.sync_last_known_own_block_timeout);
                    tokio::pin!(timer);

                    'inner: loop {
                        tokio::select! {
                            result = results.next() => {
                                let Some((result, authority_index)) = result else {
                                    break 'inner;
                                };
                                match result {
                                    Ok(result) => {
                                        match process_block_headers(result, authority_index) {
                                            Ok(block_headers) => {
                                                received_response[authority_index] = true;
                                                let max_round = block_headers.into_iter().map(|b|b.round()).max().unwrap_or(0);
                                                highest_round = highest_round.max(max_round);

                                                total_stake += context.committee.stake(authority_index);
                                            },
                                            Err(err) => {
                                                warn!("Invalid result returned from {authority_index} while fetching last own block header: {err}");
                                            }
                                        }
                                    },
                                    Err(err) => {
                                        warn!("Error {err} while fetching our own block header from peer {authority_index}. Will retry.");
                                        results.push(fetch_own_block_header(authority_index, FETCH_OWN_BLOCK_HEADER_RETRY_DELAY));
                                    }
                                }
                            },
                            () = &mut timer => {
                                info!("Timeout while trying to sync our own last block header from peers");
                                break 'inner;
                            }
                        }
                    }

                    // Request at least a quorum of 2f+1 stake to have replied back.
                    if context.committee.reached_quorum(total_stake) {
                        info!("A quorum, {} out of {} total stake, returned acceptable results for our own last block header with highest round {}, with {retries} retries.", total_stake, context.committee.total_stake(), highest_round);
                        break 'main;
                    } else {
                        info!("Only {} out of {} total stake returned acceptable results for our own last block header with highest round {}, with {retries} retries.", total_stake, context.committee.total_stake(), highest_round);
                    }

                    retries += 1;
                    context.metrics.node_metrics.sync_last_known_own_block_header_retries.inc();
                    warn!("Not enough stake: {} out of {} total stake returned acceptable results for our own last block header with highest round {}. Will now retry {retries}.", total_stake, context.committee.total_stake(), highest_round);

                    sleep(retry_delay_step).await;

                    retry_delay_step = Duration::from_secs_f64(retry_delay_step.as_secs_f64() * 1.5);
                    retry_delay_step = retry_delay_step.min(MAX_RETRY_DELAY_STEP);
                }

                // Update the Core with the highest detected round
                context.metrics.node_metrics.last_known_own_block_header_round.set(highest_round as i64);

                if let Err(err) = core_dispatcher.set_last_known_proposed_round(highest_round) {
                    warn!("Error received while calling dispatcher, probably dispatcher is shutting down, will now exit: {err:?}");
                }
            }));
    }

    /// Starts the periodic synchronization task to fetch missing block headers
    /// from peers. This task runs periodically to ensure that the missing
    /// block headers are synced.
    async fn start_periodic_sync_task(&mut self) -> ConsensusResult<()> {
        let mut missing_blocks_refs = self
            .core_dispatcher
            .get_missing_block_headers()
            .await
            .map_err(|_err| ConsensusError::Shutdown)?;

        // No reason to kick off the scheduler if there are no missing blocks to fetch
        if missing_blocks_refs.is_empty() {
            return Ok(());
        }

        let context = self.context.clone();
        let network_client = self.network_client.clone();
        let block_verifier = self.block_verifier.clone();
        let verified_cache = self.verified_headers_cache.clone();
        let commit_vote_monitor = self.commit_vote_monitor.clone();
        let core_dispatcher = self.core_dispatcher.clone();
        let inflight_block_headers_map = self.inflight_block_headers_map.clone();
        let commands_sender = self.commands_sender.clone();
        let dag_state = self.dag_state.clone();
        let transactions_synchronizer = self.transactions_synchronizer.clone();

        let (commit_lagging, last_commit_index, quorum_commit_index) = self.is_commit_lagging();
        trace!(
            "Commit lagging: {commit_lagging}, last commit index: {last_commit_index}, quorum commit index: {quorum_commit_index}"
        );
        if commit_lagging {
            // As node is commit lagging try to sync only the missing block headers that are
            // within the acceptable round thresholds to sync. The rest we don't
            // attempt to sync yet.
            let highest_accepted_round = dag_state.read().highest_accepted_round();
            missing_blocks_refs = missing_blocks_refs
                .into_iter()
                .take_while(|(block_ref, _)| {
                    block_ref.round
                        <= highest_accepted_round + self.missing_block_header_round_threshold()
                })
                .collect::<BTreeMap<_, _>>();

            // If no missing block headers are within the acceptable thresholds to sync
            // while we commit lag, then we disable the scheduler completely for
            // this run.
            if missing_blocks_refs.is_empty() {
                trace!(
                    "Scheduled synchronizer temporarily disabled as local commit is falling behind from quorum {last_commit_index} << {quorum_commit_index} and missing block headers are too far in the future."
                );
                self.context
                    .metrics
                    .node_metrics
                    .synchronizer_fetch_block_headers_scheduler_skipped
                    .with_label_values(&["commit_lagging"])
                    .inc();
                return Ok(());
            }
        }

        self.fetch_block_headers_scheduler_task
            .spawn(monitored_future!(async move {
                let _scope = monitored_scope("FetchMissingBlockHeadersScheduler");

                context
                    .metrics
                    .node_metrics
                    .synchronizer_fetch_block_headers_scheduler_inflight
                    .inc();
                let total_requested = missing_blocks_refs.len();

                fail_point_async!("consensus-delay");

                // Fetch blocks from peers
                let results = Self::fetch_block_headers_from_authorities_periodic(
                    context.clone(),
                    inflight_block_headers_map.clone(),
                    network_client,
                    missing_blocks_refs,
                    dag_state,
                )
                .await;
                context
                    .metrics
                    .node_metrics
                    .synchronizer_fetch_block_headers_scheduler_inflight
                    .dec();
                if results.is_empty() {
                    warn!("No results returned while requesting missing block headers");
                    return;
                }

                // Now process the returned results
                let mut total_fetched = 0;
                for (blocks_guard, serialized_fetched_block_headers, peer) in results {
                    total_fetched += serialized_fetched_block_headers.len();

                    if let Err(err) = Self::process_fetched_headers_from_authority(
                        serialized_fetched_block_headers,
                        peer,
                        blocks_guard,
                        core_dispatcher.clone(),
                        block_verifier.clone(),
                        verified_cache.clone(),
                        commit_vote_monitor.clone(),
                        transactions_synchronizer.clone(),
                        context.clone(),
                        commands_sender.clone(),
                        "periodic",
                    )
                    .await
                    {
                        warn!(
                            "Error occurred while processing fetched block headers from peer {peer}: {err}"
                        );
                    }
                }

                debug!(
                    "Total block headers requested to fetch: {}, total fetched: {}",
                    total_requested, total_fetched
                );
            }));
        Ok(())
    }

    fn is_commit_lagging(&self) -> (bool, CommitIndex, CommitIndex) {
        let last_commit_index = self.dag_state.read().last_commit_index();
        let quorum_commit_index = self.commit_vote_monitor.quorum_commit_index();
        let commit_threshold = last_commit_index
            + self.context.parameters.commit_sync_batch_size * COMMIT_LAG_MULTIPLIER;
        (
            commit_threshold < quorum_commit_index,
            last_commit_index,
            quorum_commit_index,
        )
    }

    /// The number of rounds above the highest accepted round to allow fetching
    /// missing block headers via the periodic synchronization. Any missing
    /// block headers of higher rounds are considered too far in the future
    /// to fetch. This property is used only when it's detected that the
    /// node has fallen behind on its commit compared to the rest of the
    /// network, otherwise scheduler will attempt to fetch any missing block
    /// header.
    fn missing_block_header_round_threshold(&self) -> Round {
        self.context.parameters.commit_sync_batch_size
    }

    /// Fetches the given `missing_blocks` from up to `MAX_PEERS` authorities in
    /// parallel:
    ///
    /// Randomly select `MAX_PEERS - MAX_RANDOM_PEERS` peers from those who
    /// are known to hold some missing block, requesting up to
    /// `MAX_BLOCKS_PER_FETCH` block refs per peer.
    ///
    /// Randomly select `MAX_RANDOM_PEERS` additional peers from the
    ///  committee (excluding self and those already selected).
    ///
    /// The method returns a vector with the fetched blocks from each peer that
    /// successfully responded and any corresponding additional ancestor blocks.
    /// Each element of the vector is a tuple which contains the requested
    /// missing block refs, the returned blocks and the peer authority index.
    #[cfg_attr(test,tracing::instrument(skip_all, name ="",fields(authority = %context.own_index)))]
    async fn fetch_block_headers_from_authorities_periodic(
        context: Arc<Context>,
        inflight_block_headers: Arc<InflightBlockHeadersMap>,
        network_client: Arc<C>,
        missing_block_headers_refs: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        dag_state: Arc<RwLock<DagState>>,
    ) -> Vec<(BlocksGuard, Vec<Bytes>, AuthorityIndex)> {
        // Step 1: Map authorities to missing block headers refs that they are aware of
        let mut authority_to_block_headers_refs: HashMap<AuthorityIndex, Vec<BlockRef>> =
            HashMap::new();
        for (missing_block_header_ref, authorities) in &missing_block_headers_refs {
            for author in authorities {
                if author == &context.own_index {
                    // Skip our own index as we don't want to fetch block headers from ourselves
                    continue;
                }
                authority_to_block_headers_refs
                    .entry(*author)
                    .or_default()
                    .push(*missing_block_header_ref);
            }
        }

        // Step 2: Choose at most MAX_PEERS-MAX_RANDOM_PEERS peers from those who are
        // aware of some missing block headers

        #[cfg(not(test))]
        let mut rng = StdRng::from_entropy();

        // Randomly pick up MAX_PEERS - MAX_RANDOM_PEERS authorities that are aware of
        // missing block headers
        #[cfg(not(test))]
        let mut chosen_peers_with_block_headers: Vec<(
            AuthorityIndex,
            Vec<BlockRef>,
            &str,
        )> = authority_to_block_headers_refs
            .iter()
            .choose_multiple(
                &mut rng,
                MAX_PERIODIC_SYNC_PEERS - MAX_PERIODIC_SYNC_RANDOM_PEERS,
            )
            .into_iter()
            .map(|(&peer, block_refs)| {
                let limited_block_refs = block_refs
                    .iter()
                    .copied()
                    .take(context.parameters.max_headers_per_regular_sync_fetch)
                    .collect();
                (peer, limited_block_refs, "periodic_known")
            })
            .collect();
        #[cfg(test)]
        // Deterministically pick the smallest (MAX_PEERS - MAX_RANDOM_PEERS) authority indices
        let mut chosen_peers_with_block_headers: Vec<(
            AuthorityIndex,
            Vec<BlockRef>,
            &str,
        )> = {
            let mut items: Vec<(AuthorityIndex, Vec<BlockRef>, &str)> =
                authority_to_block_headers_refs
                    .iter()
                    .map(|(&peer, block_refs)| {
                        let limited_block_refs = block_refs
                            .iter()
                            .copied()
                            .take(context.parameters.max_headers_per_regular_sync_fetch)
                            .collect();
                        (peer, limited_block_refs, "periodic_known")
                    })
                    .collect();
            // Sort by AuthorityIndex (natural order), then take the first MAX_PEERS -
            // MAX_RANDOM_PEERS
            items.sort_by_key(|(peer, _, _)| *peer);
            items
                .into_iter()
                .take(MAX_PERIODIC_SYNC_PEERS - MAX_PERIODIC_SYNC_RANDOM_PEERS)
                .collect()
        };

        // Step 3: Choose at most MAX_PERIODIC_SYNC_RANDOM_PEERS random peers not known
        // to be aware of the missing block headers
        let already_chosen: HashSet<AuthorityIndex> = chosen_peers_with_block_headers
            .iter()
            .map(|(peer, _, _)| *peer)
            .collect();

        let random_candidates: Vec<_> = context
            .committee
            .authorities()
            .filter_map(|(peer_index, _)| {
                (peer_index != context.own_index && !already_chosen.contains(&peer_index))
                    .then_some(peer_index)
            })
            .collect();
        #[cfg(test)]
        let random_peers: Vec<AuthorityIndex> = random_candidates
            .into_iter()
            .take(MAX_PERIODIC_SYNC_RANDOM_PEERS)
            .collect();
        #[cfg(not(test))]
        let random_peers: Vec<AuthorityIndex> = random_candidates
            .into_iter()
            .choose_multiple(&mut rng, MAX_PERIODIC_SYNC_RANDOM_PEERS);

        #[cfg_attr(test, allow(unused_mut))]
        let mut all_missing_block_headers_refs: Vec<BlockRef> =
            missing_block_headers_refs.keys().cloned().collect();
        #[cfg(not(test))]
        all_missing_block_headers_refs.shuffle(&mut rng);

        let mut block_headers_chunks = all_missing_block_headers_refs
            .chunks(context.parameters.max_headers_per_regular_sync_fetch);

        for peer in random_peers {
            if let Some(chunk) = block_headers_chunks.next() {
                chosen_peers_with_block_headers.push((peer, chunk.to_vec(), "periodic_random"));
            } else {
                break;
            }
        }

        let mut request_futures = FuturesUnordered::new();

        let highest_rounds = Self::get_highest_accepted_rounds(dag_state, &context);

        // Record the missing blocks per authority for metrics
        let mut missing_block_headers_per_authority = vec![0; context.committee.size()];
        for block_ref in &all_missing_block_headers_refs {
            missing_block_headers_per_authority[block_ref.author] += 1;
        }
        for (missing, (_, authority)) in missing_block_headers_per_authority
            .into_iter()
            .zip(context.committee.authorities())
        {
            context
                .metrics
                .node_metrics
                .synchronizer_missing_block_headers_by_authority
                .with_label_values(&[&authority.hostname.as_str()])
                .inc_by(missing as u64);
            context
                .metrics
                .node_metrics
                .synchronizer_current_missing_block_headers_by_authority
                .with_label_values(&[&authority.hostname.as_str()])
                .set(missing as i64);
        }

        // Look at peers that were not chosen yet and try to fetch block headers from
        // them if needed later
        #[cfg_attr(test, expect(unused_mut))]
        let mut remaining_peers: Vec<_> = context
            .committee
            .authorities()
            .filter_map(|(peer_index, _)| {
                if peer_index != context.own_index
                    && !chosen_peers_with_block_headers
                        .iter()
                        .any(|(chosen_peer, _, _)| *chosen_peer == peer_index)
                {
                    Some(peer_index)
                } else {
                    None
                }
            })
            .collect();

        #[cfg(not(test))]
        remaining_peers.shuffle(&mut rng);
        let mut remaining_peers = remaining_peers.into_iter();

        // Send the initial requests
        for (peer, block_headers_to_request, label) in chosen_peers_with_block_headers {
            let peer_hostname = &context.committee.authority(peer).hostname;
            let block_refs = block_headers_to_request
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();

            // Lock the block headers to be fetched. If no lock can be acquired for any of
            // the block headers then don't bother.
            if let Some(blocks_guard) =
                inflight_block_headers.lock_headers(block_refs.clone(), peer, SyncMethod::Periodic)
            {
                info!(
                    "Periodic sync of {} missing block headers from peer {} {}: {}",
                    block_refs.len(),
                    peer,
                    peer_hostname,
                    block_refs
                        .iter()
                        .map(|b| b.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                // Record metrics about requested blocks
                let metrics = &context.metrics.node_metrics;
                metrics
                    .synchronizer_requested_block_headers_by_peer
                    .with_label_values(&[peer_hostname.as_str(), label])
                    .inc_by(block_refs.len() as u64);
                for block_ref in &block_refs {
                    let block_hostname = &context.committee.authority(block_ref.author).hostname;
                    metrics
                        .synchronizer_requested_block_headers_by_authority
                        .with_label_values(&[block_hostname.as_str(), label])
                        .inc();
                }
                request_futures.push(Self::fetch_block_headers_request(
                    network_client.clone(),
                    peer,
                    blocks_guard,
                    highest_rounds.clone(),
                    FETCH_REQUEST_TIMEOUT,
                    1,
                ));
            }
        }

        let mut results = Vec::new();
        let fetcher_timeout = sleep(FETCH_FROM_PEERS_TIMEOUT);

        tokio::pin!(fetcher_timeout);

        loop {
            tokio::select! {
                Some((response, blocks_guard, _retries, peer_index, highest_rounds)) = request_futures.next() => {
                    let peer_hostname = &context.committee.authority(peer_index).hostname;
                    match response {
                        Ok(fetched_block_headers) => {
                            info!("Fetched {} block headers from peer {}", fetched_block_headers.len(), peer_hostname);
                            results.push((blocks_guard, fetched_block_headers, peer_index));

                            // no more pending requests are left, just break the loop
                            if request_futures.is_empty() {
                                break;
                            }
                        },
                        Err(_) => {
                            context.metrics.node_metrics.synchronizer_fetch_failures_by_peer.with_label_values(&[peer_hostname.as_str(), "periodic"]).inc();
                            // try again if there is any peer left
                            if let Some(next_peer) = remaining_peers.next() {
                                // do best effort to lock guards. If we can't lock then don't bother at this run.
                                if let Some(blocks_guard) = inflight_block_headers.swap_locks(blocks_guard, next_peer) {
                                    info!(
                                        "Retrying syncing {} missing block headers from peer {}: {}",
                                        blocks_guard.block_refs.len(),
                                        peer_hostname,
                                        blocks_guard.block_refs
                                            .iter()
                                            .map(|b| b.to_string())
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    );
                                    let block_refs = blocks_guard.block_refs.clone();
                                    // Record metrics about requested blocks
                                    let metrics = &context.metrics.node_metrics;
                                    metrics
                                        .synchronizer_requested_block_headers_by_peer
                                        .with_label_values(&[peer_hostname.as_str(), "periodic_retry"])
                                        .inc_by(block_refs.len() as u64);
                                    for block_ref in &block_refs {
                                        let block_hostname =
                                            &context.committee.authority(block_ref.author).hostname;
                                        metrics
                                            .synchronizer_requested_block_headers_by_authority
                                            .with_label_values(&[block_hostname.as_str(), "periodic_retry"])
                                            .inc();
                                    }
                                    request_futures.push(Self::fetch_block_headers_request(
                                        network_client.clone(),
                                        next_peer,
                                        blocks_guard,
                                        highest_rounds,
                                        FETCH_REQUEST_TIMEOUT,
                                        1,
                                    ));
                                } else {
                                    debug!("Couldn't acquire locks to fetch block headers from peer {next_peer}.")
                                }
                            } else {
                                debug!("No more peers left to fetch block headers");
                            }
                        }
                    }
                },
                _ = &mut fetcher_timeout => {
                    debug!("Timed out while fetching missing block headers");
                    // Drop all pending requests immediately — frees all block guards
                    drop(request_futures);
                    break;
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::NonZero,
        sync::Arc,
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use iota_metrics::monitored_mpsc;
    use parking_lot::RwLock;
    use starfish_config::{AuthorityIndex, Parameters};
    use tokio::{sync::Mutex, time::sleep};

    use crate::{
        CommitDigest, CommitIndex,
        authority_service::COMMIT_LAG_MULTIPLIER,
        block_header::{
            BlockHeaderDigest, BlockRef, Round, TestBlockHeader, VerifiedBlock,
            VerifiedBlockHeader, VerifiedOwnShard, VerifiedTransactions,
        },
        block_verifier::NoopBlockVerifier,
        commit::{CertifiedCommits, CommitRange, CommitVote, TrustedCommit},
        commit_vote_monitor::CommitVoteMonitor,
        context::Context,
        core::ReasonToCreateBlock,
        core_thread::{CoreError, CoreThreadDispatcher, tests::MockCoreThreadDispatcher},
        dag_state::{DagState, DataSource},
        error::{ConsensusError, ConsensusResult},
        header_synchronizer::{
            FETCH_BLOCK_HEADERS_CONCURRENCY, FETCH_REQUEST_TIMEOUT, HeaderSynchronizer,
            InflightBlockHeadersMap, SyncMethod,
        },
        network::{BlockBundleStream, NetworkClient},
        storage::mem_store::MemStore,
        transaction_ref::GenericTransactionRef,
        transactions_synchronizer::TransactionsSynchronizer,
    };

    type FetchRequestKey = (Vec<BlockRef>, AuthorityIndex);
    type FetchRequestHeadersResponse = (Vec<VerifiedBlockHeader>, Option<Duration>);
    type FetchLatestBlockKey = (AuthorityIndex, Vec<AuthorityIndex>);
    type FetchLatestHeaderResponse = (Vec<VerifiedBlockHeader>, Option<Duration>);

    #[derive(Default)]
    struct MockNetworkClient {
        fetch_headers_response: Mutex<BTreeMap<FetchRequestKey, FetchRequestHeadersResponse>>,
        fetch_latest_header_response:
            Mutex<BTreeMap<FetchLatestBlockKey, Vec<FetchLatestHeaderResponse>>>,
    }

    impl MockNetworkClient {
        async fn stub_fetch_headers_response(
            &self,
            block_headers: Vec<VerifiedBlockHeader>,
            peer: AuthorityIndex,
            latency: Option<Duration>,
        ) {
            let mut lock = self.fetch_headers_response.lock().await;
            let block_refs = block_headers
                .iter()
                .map(|block| block.reference())
                .collect::<Vec<_>>();
            lock.insert((block_refs, peer), (block_headers, latency));
        }

        async fn stub_fetch_latest_block_headers_response(
            &self,
            block_headers: Vec<VerifiedBlockHeader>,
            peer: AuthorityIndex,
            authorities: Vec<AuthorityIndex>,
            latency: Option<Duration>,
        ) {
            let mut lock = self.fetch_latest_header_response.lock().await;
            lock.entry((peer, authorities))
                .or_default()
                .push((block_headers, latency));
        }

        async fn fetch_latest_block_headers_pending_calls(&self) -> usize {
            let lock = self.fetch_latest_header_response.lock().await;
            lock.len()
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
            unimplemented!("Unimplemented")
        }

        async fn fetch_transactions(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<GenericTransactionRef>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_block_headers(
            &self,
            peer: AuthorityIndex,
            block_refs: Vec<BlockRef>,
            _highest_accepted_rounds: Vec<Round>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            let mut lock = self.fetch_headers_response.lock().await;
            // If the key is not found, just return an empty vector and no delay.
            let response = lock
                .remove(&(block_refs, peer))
                .unwrap_or((Vec::new(), None));

            let mut block_headers = vec![];
            for block_header in response.0.into_iter() {
                block_headers.push(block_header.serialized().clone());
            }

            drop(lock);

            if let Some(latency) = response.1 {
                sleep(latency).await;
            }

            Ok(block_headers)
        }

        async fn fetch_commits(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_latest_block_headers(
            &self,
            peer: AuthorityIndex,
            authorities: Vec<AuthorityIndex>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            let mut lock = self.fetch_latest_header_response.lock().await;
            let mut responses = lock
                .remove(&(peer, authorities.clone()))
                .expect("Unexpected fetch blocks request made");

            let response = responses.remove(0);
            let mut serialized_headers = vec![];
            for block in response.0.into_iter() {
                let serialized_header = block.serialized();
                serialized_headers.push(serialized_header.clone());
            }

            if !responses.is_empty() {
                lock.insert((peer, authorities), responses);
            }

            drop(lock);

            if let Some(latency) = response.1 {
                sleep(latency).await;
            }

            Ok(serialized_headers)
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

    #[test]
    fn test_inflight_blocks_map() {
        // GIVEN
        let map = InflightBlockHeadersMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
            BlockRef::new(12, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
            BlockRef::new(15, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
        ];
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // Lock & unlock blocks - using Periodic sync method (limit 3)
        {
            let mut all_guards = Vec::new();

            // Try to acquire the block locks for authorities 1, 2, and 3.
            for i in 1..=3 {
                let authority = AuthorityIndex::new_for_test(i);

                let guard =
                    map.lock_headers(missing_block_refs.clone(), authority, SyncMethod::Periodic);
                let guard = guard.expect("Guard should be created");
                assert_eq!(guard.block_refs.len(), 4);

                all_guards.push(guard);

                // trying to acquire any of them again will not succeed
                let guard =
                    map.lock_headers(missing_block_refs.clone(), authority, SyncMethod::Periodic);
                assert!(guard.is_none());
            }

            // Trying to acquire for authority 4 will fail - as we have maxed out the
            // number of allowed peers (Periodic limit is 3)
            let authority_4 = AuthorityIndex::new_for_test(4);

            let guard = map.lock_headers(
                missing_block_refs.clone(),
                authority_4,
                SyncMethod::Periodic,
            );
            assert!(guard.is_none());

            // Explicitly drop the guard of authority 1 and try for authority 4 again - it
            // will now succeed
            drop(all_guards.remove(0));

            let guard = map.lock_headers(
                missing_block_refs.clone(),
                authority_4,
                SyncMethod::Periodic,
            );
            let guard = guard.expect("Guard should be successfully acquired");

            assert_eq!(guard.block_refs, missing_block_refs);

            // Dropping all guards should unlock on the block refs
            drop(guard);
            drop(all_guards);

            assert_eq!(map.num_of_locked_headers(), 0);
        }

        // Swap locks
        {
            // acquire a lock for authority 1
            let authority_1 = AuthorityIndex::new_for_test(1);
            let guard = map
                .lock_headers(
                    missing_block_refs.clone(),
                    authority_1,
                    SyncMethod::Periodic,
                )
                .unwrap();

            // Now swap the locks for authority 2
            let authority_2 = AuthorityIndex::new_for_test(2);
            let guard = map.swap_locks(guard, authority_2);
            let guard = guard.expect("Guard should be created");
            assert_eq!(guard.block_refs, missing_block_refs);
            let mut all_guards = Vec::new();
            all_guards.push(guard);
            // authority 1 should now be unlocked, so now we can lock the same refs with
            // authorities 3 and 4, but not 5 (limit of 3)
            let authority_3 = AuthorityIndex::new_for_test(3);
            let guard = map.lock_headers(
                missing_block_refs.clone(),
                authority_3,
                SyncMethod::Periodic,
            );
            let guard = guard.expect("Guard should be created");
            assert_eq!(guard.block_refs.len(), 4);
            all_guards.push(guard);

            let authority_4 = AuthorityIndex::new_for_test(4);
            let guard = map.lock_headers(
                missing_block_refs.clone(),
                authority_4,
                SyncMethod::Periodic,
            );
            let guard = guard.expect("Guard should be created");
            assert_eq!(guard.block_refs.len(), 4);
            all_guards.push(guard);

            let authority_5 = AuthorityIndex::new_for_test(5);
            let guard = map.lock_headers(missing_block_refs, authority_5, SyncMethod::Periodic);
            assert!(guard.is_none());
        }
    }

    #[test]
    fn test_inflight_blocks_map_with_sync_methods() {
        // GIVEN
        let map = InflightBlockHeadersMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
        ];
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // Test 1: Live sync limit (1 authority)
        {
            let authority_1 = AuthorityIndex::new_for_test(1);
            let guard_1 = map
                .lock_headers(missing_block_refs.clone(), authority_1, SyncMethod::Live)
                .expect("Should successfully lock with Live sync");

            assert_eq!(guard_1.block_refs.len(), 2);

            // Authority 2 cannot lock with Live sync (limit of 1 reached)
            let authority_2 = AuthorityIndex::new_for_test(2);
            let guard_2 =
                map.lock_headers(missing_block_refs.clone(), authority_2, SyncMethod::Live);

            assert!(
                guard_2.is_none(),
                "Should fail to lock - Live limit of 1 reached"
            );

            // Release the lock
            drop(guard_1);

            // Now authority 2 can lock with Live sync
            let guard_2 = map
                .lock_headers(missing_block_refs.clone(), authority_2, SyncMethod::Live)
                .expect("Should successfully lock after authority 1 released");

            assert_eq!(guard_2.block_refs.len(), 2);
            drop(guard_2);
        }

        // Test 2: Periodic sync allows more concurrency (3 authorities)
        {
            let authority_1 = AuthorityIndex::new_for_test(1);
            let guard_1 = map
                .lock_headers(
                    missing_block_refs.clone(),
                    authority_1,
                    SyncMethod::Periodic,
                )
                .expect("Should successfully lock with Periodic sync");

            assert_eq!(guard_1.block_refs.len(), 2);

            // Authority 2 can also lock with Periodic sync (limit is 3)
            let authority_2 = AuthorityIndex::new_for_test(2);
            let guard_2 = map
                .lock_headers(
                    missing_block_refs.clone(),
                    authority_2,
                    SyncMethod::Periodic,
                )
                .expect("Should successfully lock - Periodic allows 3 authorities");

            assert_eq!(guard_2.block_refs.len(), 2);

            // Authority 3 can also lock with Periodic sync (limit is 3)
            let authority_3 = AuthorityIndex::new_for_test(3);
            let guard_3 = map
                .lock_headers(
                    missing_block_refs.clone(),
                    authority_3,
                    SyncMethod::Periodic,
                )
                .expect("Should successfully lock - Periodic allows 3 authorities");

            assert_eq!(guard_3.block_refs.len(), 2);

            // But authority 4 cannot lock with Periodic sync (limit of 3 reached)
            let authority_4 = AuthorityIndex::new_for_test(4);
            let guard_4 = map.lock_headers(
                missing_block_refs.clone(),
                authority_4,
                SyncMethod::Periodic,
            );

            assert!(
                guard_4.is_none(),
                "Should fail to lock - Periodic limit of 3 reached"
            );

            // Release locks
            drop(guard_1);
            drop(guard_2);
            drop(guard_3);
        }

        // Test 3: Periodic blocks Live when at Live's limit
        {
            // Authority 1 locks with Periodic sync (total=1, at Live's limit)
            let authority_1 = AuthorityIndex::new_for_test(1);
            let guard_1 = map
                .lock_headers(
                    missing_block_refs.clone(),
                    authority_1,
                    SyncMethod::Periodic,
                )
                .expect("Should successfully lock with Periodic sync");

            assert_eq!(guard_1.block_refs.len(), 2);

            // Authority 2 cannot lock with Live sync (total already at Live limit of 1)
            let authority_2 = AuthorityIndex::new_for_test(2);
            let guard_2_live =
                map.lock_headers(missing_block_refs.clone(), authority_2, SyncMethod::Live);

            assert!(
                guard_2_live.is_none(),
                "Should fail to lock with Live - total already at Live limit of 1"
            );

            // But authority 2 CAN lock with Periodic sync (total would be 2, under the
            // Periodic limit)
            let guard_2_periodic = map
                .lock_headers(
                    missing_block_refs.clone(),
                    authority_2,
                    SyncMethod::Periodic,
                )
                .expect("Should successfully lock with Periodic - under Periodic limit of 3");

            assert_eq!(guard_2_periodic.block_refs.len(), 2);

            drop(guard_1);
            drop(guard_2_periodic);
        }

        // Test 4: Live then Periodic interaction
        {
            // Authority 1 locks with Live sync (total=1, at Live limit)
            let authority_1 = AuthorityIndex::new_for_test(1);
            let guard_1 = map
                .lock_headers(missing_block_refs.clone(), authority_1, SyncMethod::Live)
                .expect("Should successfully lock with Live sync");

            assert_eq!(guard_1.block_refs.len(), 2);

            // Authority 2 cannot lock with Live sync (would exceed Live limit of 1)
            let authority_2 = AuthorityIndex::new_for_test(2);
            let guard_2_live =
                map.lock_headers(missing_block_refs.clone(), authority_2, SyncMethod::Live);

            assert!(
                guard_2_live.is_none(),
                "Should fail to lock with Live - would exceed Live limit of 1"
            );

            // But authority 2 CAN lock with Periodic sync (total=2, still under the
            // Periodic limit)
            let guard_2 = map
                .lock_headers(
                    missing_block_refs.clone(),
                    authority_2,
                    SyncMethod::Periodic,
                )
                .expect("Should successfully lock with Periodic - total 2 is under Periodic limit");

            assert_eq!(guard_2.block_refs.len(), 2);

            // And authority 3 can still lock with Periodic sync (reaching the Periodic
            // limit)
            let authority_3 = AuthorityIndex::new_for_test(3);
            let guard_3 = map
                .lock_headers(
                    missing_block_refs.clone(),
                    authority_3,
                    SyncMethod::Periodic,
                )
                .expect("Should successfully lock with Periodic - total 3 reaches Periodic limit");

            assert_eq!(guard_3.block_refs.len(), 2);

            // Authority 4 would exceed the Periodic limit.
            let authority_4 = AuthorityIndex::new_for_test(4);
            let guard_4 = map.lock_headers(missing_block_refs, authority_4, SyncMethod::Periodic);

            assert!(
                guard_4.is_none(),
                "Should fail to lock with Periodic - would exceed Periodic limit of 3"
            );

            drop(guard_1);
            drop(guard_2);
            drop(guard_3);
        }

        // Test 5: Partial locks with mixed methods
        {
            let block_a = BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN);
            let block_b = BlockRef::new(2, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN);

            // Lock block A with authority 1 using Live (A at limit for Live)
            let guard_a = map
                .lock_headers(
                    [block_a].into(),
                    AuthorityIndex::new_for_test(1),
                    SyncMethod::Live,
                )
                .expect("Should lock block A");
            assert_eq!(guard_a.block_refs.len(), 1);

            // Lock block B with authorities 1, 2, and 3 using Periodic (B at limit for
            // Periodic)
            let guard_b1 = map
                .lock_headers(
                    [block_b].into(),
                    AuthorityIndex::new_for_test(1),
                    SyncMethod::Periodic,
                )
                .expect("Should lock block B with authority 1");
            assert_eq!(guard_b1.block_refs.len(), 1);

            let guard_b2 = map
                .lock_headers(
                    [block_b].into(),
                    AuthorityIndex::new_for_test(2),
                    SyncMethod::Periodic,
                )
                .expect("Should lock block B with authority 2");
            assert_eq!(guard_b2.block_refs.len(), 1);

            let guard_b3 = map
                .lock_headers(
                    [block_b].into(),
                    AuthorityIndex::new_for_test(3),
                    SyncMethod::Periodic,
                )
                .expect("Should lock block B with authority 3");
            assert_eq!(guard_b3.block_refs.len(), 1);

            // Cannot lock block A with authority 2 using Live (A already at Live limit)
            let guard_a2 = map.lock_headers(
                [block_a].into(),
                AuthorityIndex::new_for_test(2),
                SyncMethod::Live,
            );
            assert!(guard_a2.is_none());

            // Cannot lock block B with authority 4 using Periodic (B already at Periodic
            // limit)
            let guard_b4 = map.lock_headers(
                [block_b].into(),
                AuthorityIndex::new_for_test(4),
                SyncMethod::Periodic,
            );
            assert!(guard_b4.is_none());

            drop(guard_a);
            drop(guard_b1);
            drop(guard_b2);
            drop(guard_b3);
        }
    }

    #[tokio::test]
    async fn successful_fetch_blocks_from_peer() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let network_client = Arc::new(MockNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let handle = HeaderSynchronizer::start(
            network_client.clone(),
            context,
            core_dispatcher.clone(),
            commit_vote_monitor,
            transactions_synchronizer,
            block_verifier,
            dag_state,
            false,
        );

        // Create some test block headers
        let expected_block_headers = (0..10)
            .map(|round| VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 0).build()))
            .collect::<Vec<_>>();
        let missing_block_headers = expected_block_headers
            .iter()
            .map(|block| block.reference())
            .collect::<BTreeSet<_>>();

        // AND stub the fetch_block_headers request from peer 1
        let peer = AuthorityIndex::new_for_test(1);
        network_client
            .stub_fetch_headers_response(expected_block_headers.clone(), peer, None)
            .await;

        // WHEN request missing blocks from peer 1
        assert!(
            handle
                .fetch_headers(missing_block_headers, peer)
                .await
                .is_ok()
        );

        // Wait a little bit until those have been added in core
        sleep(Duration::from_millis(1_000)).await;

        // THEN ensure those ended up in Core
        let added_blocks = core_dispatcher.get_and_drain_block_headers().await;
        assert_eq!(added_blocks, expected_block_headers);

        // Stop synchronizer and ensure that no panic occurred
        if let Err(err) = handle.stop().await {
            if err.is_panic() {
                std::panic::resume_unwind(err.into_panic());
            }
        }
    }

    #[tokio::test]
    async fn saturate_fetch_block_headers_from_peer() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let network_client = Arc::new(MockNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let handle = HeaderSynchronizer::start(
            network_client.clone(),
            context,
            core_dispatcher.clone(),
            commit_vote_monitor,
            transactions_synchronizer,
            block_verifier,
            dag_state,
            false,
        );

        // Create some test block headers
        // We need FETCH_BLOCK_HEADERS_CONCURRENCY to saturate vector of requests,
        // FETCH_BLOCK_HEADERS_CONCURRENCY to saturate the channel, and the 2 *
        // FETCH_BLOCK_HEADERS_CONCURRENCY + 1 headers will cause saturation error.
        let expected_block_headers = (0..=2 * FETCH_BLOCK_HEADERS_CONCURRENCY)
            .map(|round| {
                VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round as Round, 0).build())
            })
            .collect::<Vec<_>>();

        // Now start sending requests to fetch blocks by trying to saturate peer 1 task
        let peer = AuthorityIndex::new_for_test(1);
        let mut iter = expected_block_headers.iter().peekable();
        while let Some(block_header) = iter.next() {
            // stub the fetch_block_headers request from peer 1 and give some high response
            // latency so requests can start blocking the peer task.
            network_client
                .stub_fetch_headers_response(
                    vec![block_header.clone()],
                    peer,
                    Some(Duration::from_millis(5_000)),
                )
                .await;

            let mut missing_blocks_refs = BTreeSet::new();
            missing_blocks_refs.insert(block_header.reference());

            // WHEN requesting to fetch the block headers, it should not succeed for the
            // last request and get an error with "saturated" synchronizer
            if iter.peek().is_none() {
                match handle.fetch_headers(missing_blocks_refs, peer).await {
                    Err(ConsensusError::SynchronizerSaturated(index)) => {
                        assert_eq!(index, peer);
                    }
                    _ => panic!("A saturated synchronizer error was expected"),
                }
            } else {
                assert!(
                    handle
                        .fetch_headers(missing_blocks_refs, peer)
                        .await
                        .is_ok()
                );
            }
        }
        // Stop synchronizer and ensure that no panic occurred
        if let Err(err) = handle.stop().await {
            if err.is_panic() {
                std::panic::resume_unwind(err.into_panic());
            }
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn synchronizer_periodic_task_fetch_blocks() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let network_client = Arc::new(MockNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        // Create some test block headers
        let expected_block_headers_1 = (0..10)
            .map(|round| VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 1).build()))
            .collect::<Vec<_>>();
        let expected_block_headers_2 = (10..20)
            .map(|round| VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 2).build()))
            .collect::<Vec<_>>();
        let missing_blocks_refs_1 = expected_block_headers_1
            .iter()
            .map(|block| block.reference())
            .collect::<BTreeSet<_>>();
        let missing_blocks_refs_2 = expected_block_headers_2
            .iter()
            .map(|block| block.reference())
            .collect::<BTreeSet<_>>();

        // AND stub the missing blocks refs
        core_dispatcher
            .stub_missing_block_headers(missing_blocks_refs_1.clone())
            .await;
        core_dispatcher
            .stub_missing_block_headers(missing_blocks_refs_2.clone())
            .await;

        // AND stub the request responses for authority 1 & 2. They will be picked as
        // peers that know block_refs. Make the first authority timeout, so the
        // second will be called. "We" are authority = 0, so we are skipped
        // anyway.
        network_client
            .stub_fetch_headers_response(
                expected_block_headers_1.clone(),
                AuthorityIndex::new_for_test(1),
                Some(FETCH_REQUEST_TIMEOUT * 2),
            )
            .await;
        network_client
            .stub_fetch_headers_response(
                expected_block_headers_2.clone(),
                AuthorityIndex::new_for_test(2),
                None,
            )
            .await;
        // Stub all headers to the third peer that will be chosen (quasi)-randomly as an
        // additional peer.
        let mut all_expected_headers = expected_block_headers_1.clone();
        all_expected_headers.extend(expected_block_headers_2.clone());
        network_client
            .stub_fetch_headers_response(
                all_expected_headers.clone(),
                AuthorityIndex::new_for_test(3),
                Some(FETCH_REQUEST_TIMEOUT * 2),
            )
            .await;

        // WHEN start the synchronizer and wait for a couple of seconds
        let handle = HeaderSynchronizer::start(
            network_client.clone(),
            context,
            core_dispatcher.clone(),
            commit_vote_monitor,
            transactions_synchronizer,
            block_verifier,
            dag_state,
            false,
        );

        sleep(8 * FETCH_REQUEST_TIMEOUT).await;

        // THEN the missing block headers from peer 2 should now be fetched and added to
        // core
        let added_block_headers = core_dispatcher.get_and_drain_block_headers().await;
        assert_eq!(added_block_headers, expected_block_headers_2);

        // AND missing blocks should contain header from peer 1
        assert_eq!(
            core_dispatcher
                .get_missing_block_headers()
                .await
                .unwrap()
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            missing_blocks_refs_1
        );

        // Stop synchronizer and ensure that no panic occurred
        if let Err(err) = handle.stop().await {
            if err.is_panic() {
                std::panic::resume_unwind(err.into_panic());
            }
        }
    }
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn synchronizer_periodic_task_when_commit_lagging_with_missing_blocks_in_acceptable_thresholds()
     {
        // GIVEN
        let (context, _) = Context::new_for_test(4);

        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let network_client = Arc::new(MockNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        // AND stub some missing blocks. The highest accepted round is 0.
        // Create some blocks that are below and above the threshold sync.
        let sync_missing_block_round_threshold = context.parameters.commit_sync_batch_size;
        let expected_block_headers = (1..=sync_missing_block_round_threshold * 2)
            .flat_map(|round| {
                vec![
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 1).build()),
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 2).build()),
                    VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 3).build()),
                ]
                .into_iter()
            })
            .collect::<Vec<_>>();

        let missing_blocks_refs = expected_block_headers
            .iter()
            .map(|block| block.reference())
            .collect::<BTreeSet<_>>();
        core_dispatcher
            .stub_missing_block_headers(missing_blocks_refs)
            .await;

        // Stub the requests for authority 1 & 2 & 3
        let stub_block_headers = expected_block_headers
            .iter()
            .map(|block_header| (block_header.reference(), block_header.clone()))
            .collect::<BTreeMap<_, _>>();

        // Authority 1 and 2 will be requested about their blocks
        let stub_block_author_1 = stub_block_headers
            .iter()
            .filter(|(block, _)| block.author == AuthorityIndex::new_for_test(1))
            .take(context.parameters.max_headers_per_regular_sync_fetch)
            .map(|(_, block)| block.clone())
            .collect::<Vec<_>>();

        let stub_block_author_2 = stub_block_headers
            .iter()
            .filter(|(block, _)| block.author == AuthorityIndex::new_for_test(2))
            .take(context.parameters.max_headers_per_regular_sync_fetch)
            .map(|(_, block)| block.clone())
            .collect::<Vec<_>>();

        // Authority 3 will be requested about the first block headers in the missing
        // blocks
        let stub_block_author_3 = stub_block_headers
            .iter()
            .take(context.parameters.max_headers_per_regular_sync_fetch)
            .map(|(_, block)| block.clone())
            .collect::<Vec<_>>();

        let mut expected_blocks: Vec<_> = Vec::new();
        expected_blocks.extend(stub_block_author_1.clone());
        expected_blocks.extend(stub_block_author_2.clone());
        expected_blocks.extend(stub_block_author_3.clone());

        network_client
            .stub_fetch_headers_response(stub_block_author_1, AuthorityIndex::new_for_test(1), None)
            .await;
        network_client
            .stub_fetch_headers_response(stub_block_author_2, AuthorityIndex::new_for_test(2), None)
            .await;
        network_client
            .stub_fetch_headers_response(stub_block_author_3, AuthorityIndex::new_for_test(3), None)
            .await;

        // Now create some blocks to simulate a commit lag
        let round = context.parameters.commit_sync_batch_size * COMMIT_LAG_MULTIPLIER * 2;
        let commit_index: CommitIndex = round - 1;
        let blocks = (0..4)
            .map(|authority| {
                let commit_votes = vec![CommitVote::new(commit_index, CommitDigest::MIN)];
                let block = TestBlockHeader::new(round, authority)
                    .set_commit_votes(commit_votes)
                    .build();

                VerifiedBlockHeader::new_for_test(block)
            })
            .collect::<Vec<_>>();

        // Pass them through the commit vote monitor - so now there will be a big commit
        // lag to prevent the scheduled synchronizer from running
        for block in blocks {
            commit_vote_monitor.observe_block(&block);
        }

        // Start the synchronizer and wait for a couple of seconds where normally
        // the synchronizer should have kicked in.
        let handle = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer,
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        sleep(4 * FETCH_REQUEST_TIMEOUT).await;

        // Stop synchronizer and ensure that no panic occurred
        if let Err(err) = handle.stop().await {
            if err.is_panic() {
                std::panic::resume_unwind(err.into_panic());
            }
        }

        // We should be in commit lag mode, but since there are missing blocks within
        // the acceptable round thresholds those ones should be fetched. Nothing above.
        let mut added_block_headers = core_dispatcher.get_and_drain_block_headers().await;

        added_block_headers.sort_by_key(|block| block.reference());
        expected_blocks.sort_by_key(|block| block.reference());
        expected_blocks.dedup_by_key(|block| block.reference());

        assert_eq!(added_block_headers, expected_blocks);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn synchronizer_periodic_task_when_commit_lagging_gets_disabled() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let network_client = Arc::new(MockNetworkClient::default());
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        // AND stub some missing blocks. The highest accepted round is 0. Create blocks
        // that are above the threshold sync.
        let sync_missing_block_round_threshold = context.parameters.commit_sync_batch_size;
        let stub_headers = (sync_missing_block_round_threshold * 2
            ..sync_missing_block_round_threshold * 2
                + context.parameters.max_headers_per_regular_sync_fetch as u32)
            .map(|round| VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 0).build()))
            .collect::<Vec<_>>();
        let missing_blocks_refs = stub_headers
            .iter()
            .map(|block| block.reference())
            .collect::<BTreeSet<_>>();
        core_dispatcher
            .stub_missing_block_headers(missing_blocks_refs.clone())
            .await;

        // AND stub the requests for authority 1 & 2
        // Make the first authority timeout, so the second will be called. "We" are
        // authority = 0, so we are skipped anyway.
        let mut expected_headers = stub_headers
            .iter()
            .take(context.parameters.max_headers_per_regular_sync_fetch)
            .cloned()
            .collect::<Vec<_>>();
        network_client
            .stub_fetch_headers_response(
                expected_headers.clone(),
                AuthorityIndex::new_for_test(1),
                Some(FETCH_REQUEST_TIMEOUT),
            )
            .await;
        network_client
            .stub_fetch_headers_response(
                expected_headers.clone(),
                AuthorityIndex::new_for_test(2),
                None,
            )
            .await;

        // Now create some blocks to simulate a commit lag
        let round = context.parameters.commit_sync_batch_size * COMMIT_LAG_MULTIPLIER * 2;
        let commit_index: CommitIndex = round - 1;
        let blocks = (0..4)
            .map(|authority| {
                let commit_votes = vec![CommitVote::new(commit_index, CommitDigest::MIN)];
                let block = TestBlockHeader::new(round, authority)
                    .set_commit_votes(commit_votes)
                    .build();

                VerifiedBlockHeader::new_for_test(block)
            })
            .collect::<Vec<_>>();

        // Pass them through the commit vote monitor - so now there will be a big commit
        // lag to prevent the scheduled synchronizer from running
        for block in blocks {
            commit_vote_monitor.observe_block(&block);
        }

        // Start the synchronizer and wait for a couple of seconds where normally
        // the synchronizer should have kicked in.
        let handle = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer,
            block_verifier,
            dag_state.clone(),
            false,
        );

        sleep(4 * FETCH_REQUEST_TIMEOUT).await;

        // Since we should be in commit lag mode none of the missed blocks should have
        // been fetched - hence nothing should be sent to core for processing.
        let added_blocks = core_dispatcher.get_and_drain_blocks().await;
        assert_eq!(added_blocks, vec![]);

        // AND advance now the local commit index by adding a new commit that matches
        // the commit index of quorum
        {
            let mut d = dag_state.write();
            for index in 1..=commit_index {
                let commit = TrustedCommit::new_for_test(
                    &context,
                    index,
                    CommitDigest::MIN,
                    0,
                    BlockRef::new(
                        index,
                        AuthorityIndex::new_for_test(0),
                        BlockHeaderDigest::MIN,
                    ),
                    vec![],
                    vec![],
                );

                d.add_commit(commit);
            }

            assert_eq!(
                d.last_commit_index(),
                commit_vote_monitor.quorum_commit_index()
            );
        }

        // Now stub again the missing blocks to fetch the exact same ones.
        core_dispatcher
            .stub_missing_block_headers(missing_blocks_refs.clone())
            .await;

        sleep(2 * FETCH_REQUEST_TIMEOUT).await;

        // THEN the missing blocks should now be fetched and added to core
        let mut added_blocks = core_dispatcher.get_and_drain_block_headers().await;

        added_blocks.sort_by_key(|block| block.reference());
        expected_headers.sort_by_key(|block| block.reference());

        assert_eq!(added_blocks, expected_headers);

        // Stop synchronizer and ensure that no panic occurred
        if let Err(err) = handle.stop().await {
            if err.is_panic() {
                std::panic::resume_unwind(err.into_panic());
            }
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn synchronizer_fetch_own_last_block_header() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context.with_parameters(Parameters {
            sync_last_known_own_block_timeout: Duration::from_millis(2_000),
            ..Default::default()
        }));
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let network_client = Arc::new(MockNetworkClient::default());
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let our_index = AuthorityIndex::new_for_test(0);
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        // Create some test block headers
        let mut expected_block_headers = (8..=10)
            .map(|round| VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 0).build()))
            .collect::<Vec<_>>();

        // Now set different latest block headers for the peers
        // For peer 1 we give the block header of round 10 (highest)
        let block_header_1 = expected_block_headers.pop().unwrap();
        network_client
            .stub_fetch_latest_block_headers_response(
                vec![block_header_1.clone()],
                AuthorityIndex::new_for_test(1),
                vec![our_index],
                Some(Duration::from_secs(10)),
            )
            .await;
        network_client
            .stub_fetch_latest_block_headers_response(
                vec![block_header_1],
                AuthorityIndex::new_for_test(1),
                vec![our_index],
                None,
            )
            .await;

        // For peer 2 we give the block header of round 9
        let block_header_2 = expected_block_headers.pop().unwrap();
        network_client
            .stub_fetch_latest_block_headers_response(
                vec![block_header_2.clone()],
                AuthorityIndex::new_for_test(2),
                vec![our_index],
                Some(Duration::from_secs(10)),
            )
            .await;
        network_client
            .stub_fetch_latest_block_headers_response(
                vec![block_header_2],
                AuthorityIndex::new_for_test(2),
                vec![our_index],
                None,
            )
            .await;

        // For peer 3 we give a block header with the lowest round
        let block_header_3 = expected_block_headers.pop().unwrap();
        network_client
            .stub_fetch_latest_block_headers_response(
                vec![block_header_3.clone()],
                AuthorityIndex::new_for_test(3),
                vec![our_index],
                Some(Duration::from_secs(10)),
            )
            .await;
        network_client
            .stub_fetch_latest_block_headers_response(
                vec![block_header_3],
                AuthorityIndex::new_for_test(3),
                vec![our_index],
                None,
            )
            .await;

        // WHEN start the synchronizer and wait for a couple of seconds
        let handle = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor,
            transactions_synchronizer,
            block_verifier,
            dag_state,
            true,
        );

        // Wait at least for the timeout time
        sleep(context.parameters.sync_last_known_own_block_timeout * 2).await;

        // Assert that core has been called to set the min proposed round
        assert_eq!(
            core_dispatcher.get_last_own_proposed_round().await,
            vec![10]
        );

        // Ensure that all the requests have been called
        assert_eq!(
            network_client
                .fetch_latest_block_headers_pending_calls()
                .await,
            0
        );

        // And we got one retry
        assert_eq!(
            context
                .metrics
                .node_metrics
                .sync_last_known_own_block_header_retries
                .get(),
            1
        );

        // Check that we restored our last know block header correctly
        assert_eq!(
            context
                .metrics
                .node_metrics
                .last_known_own_block_header_round
                .get(),
            10
        );

        // Stop synchronizer and ensure that no panic occurred
        if let Err(err) = handle.stop().await {
            if err.is_panic() {
                std::panic::resume_unwind(err.into_panic());
            }
        }
    }
    #[derive(Default)]
    struct SyncMockDispatcher {
        missing_block_headers: Mutex<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>,
        added_blocks: Mutex<Vec<VerifiedBlock>>,
    }

    #[async_trait::async_trait]
    impl CoreThreadDispatcher for SyncMockDispatcher {
        async fn add_blocks(
            &self,
            blocks: Vec<VerifiedBlock>,
            _source: DataSource,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            let mut guard = self.added_blocks.lock().await;
            guard.extend(blocks.clone());
            Ok((
                blocks.iter().map(|b| b.reference()).collect(),
                BTreeMap::new(),
            ))
        }
        async fn add_block_headers(
            &self,
            _blocks: Vec<VerifiedBlockHeader>,
            _source: DataSource,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            unimplemented!("Unimplemented")
        }

        async fn add_transactions(
            &self,
            _transactions: Vec<VerifiedTransactions>,
            _source: DataSource,
        ) -> Result<(), CoreError> {
            unimplemented!("Unimplemented")
        }

        async fn add_shards(&self, _shards: Vec<VerifiedOwnShard>) -> Result<(), CoreError> {
            unimplemented!("Unimplemented")
        }

        async fn get_missing_transaction_data(
            &self,
        ) -> Result<BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>, CoreError> {
            unimplemented!("Unimplemented")
        }

        // Stub out the remaining CoreThreadDispatcher methods with defaults:
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
            // No additional certified-commit logic in tests
            Ok((BTreeSet::new(), BTreeMap::new()))
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
            Ok(BTreeMap::new())
        }

        async fn get_missing_block_headers(
            &self,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            Ok(self.missing_block_headers.lock().await.clone())
        }

        fn set_quorum_subscribers_exists(&self, _exists: bool) -> Result<(), CoreError> {
            Ok(())
        }

        fn set_last_known_proposed_round(&self, _round: Round) -> Result<(), CoreError> {
            Ok(())
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn known_before_random_peer_fetch() {
        {
            // 1) Setup 10‐node context and in‐mem DAG
            let (ctx, _) = Context::new_for_test(10);
            let context = Arc::new(ctx);
            let store = Arc::new(MemStore::new(context.clone()));
            let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
            let inflight = InflightBlockHeadersMap::new();

            // 2) One missing block header
            let missing_vbh =
                VerifiedBlockHeader::new_for_test(TestBlockHeader::new(100, 3).build());
            let missing_ref = missing_vbh.reference();
            let missing_blocks = BTreeMap::from([(
                missing_ref,
                BTreeSet::from([
                    AuthorityIndex::new_for_test(2),
                    AuthorityIndex::new_for_test(3),
                    AuthorityIndex::new_for_test(5),
                ]),
            )]);

            // 3) Prepare mocks and stubs
            let network_client = Arc::new(MockNetworkClient::default());
            // Stub *all* authorities so none panic:
            for i in 1..=9 {
                let peer = AuthorityIndex::new_for_test(i);
                let latency = if i == 1 {
                    Some(Duration::from_millis(2))
                } else if i == 3 {
                    Some(Duration::from_millis(1))
                } else {
                    None
                };
                network_client
                    .stub_fetch_headers_response(vec![missing_vbh.clone()], peer, latency)
                    .await;
            }

            // 4) Invoke knowledge-based fetch and random fallback selection
            //    deterministically
            let results = HeaderSynchronizer::<
                MockNetworkClient,
                NoopBlockVerifier,
                SyncMockDispatcher,
            >::fetch_block_headers_from_authorities_periodic(
                context.clone(),
                inflight.clone(),
                network_client.clone(),
                missing_blocks,
                dag_state.clone(),
            )
            .await;

            // 5) With MAX_PERIODIC_SYNC_PEERS=4 and MAX_PERIODIC_SYNC_RANDOM_PEERS=2:
            // - 2 known peers are selected first: 2 and 3
            // - 2 random peers chosen: 1 and 4, but only peer 1 gets a chunk (all refs fit
            //   in one chunk), so peer 4 has nothing to request
            assert_eq!(results.len(), 3);

            // 6) Results in order: peers 2 and 3 (known), then peer 1 (random)
            let peers: Vec<_> = results.iter().map(|(_, _, peer)| *peer).collect();
            assert_eq!(
                peers,
                vec![
                    AuthorityIndex::new_for_test(2),
                    AuthorityIndex::new_for_test(3),
                    AuthorityIndex::new_for_test(1),
                ]
            );

            // 7) Verify the returned bytes correspond to that block
            for (_, bytes, _) in &results {
                let expected = missing_vbh.serialized().clone();
                assert_eq!(bytes, &vec![expected]);
            }
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn known_before_periodic_peer_fetch_larger_scenario() {
        use std::{
            collections::{BTreeMap, BTreeSet},
            sync::Arc,
        };

        use parking_lot::RwLock;

        use crate::{
            block_header::{Round, TestBlockHeader},
            context::Context,
        };

        // 1) Setup a 10-node context, in-memory DAG, and inflight map
        let (ctx, _) = Context::new_for_test(10);
        let context = Arc::new(ctx);
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let inflight = InflightBlockHeadersMap::new();
        let network_client = Arc::new(MockNetworkClient::default());

        // 2) Create 1000 missing blocks known by authorities 0, 2, and 3
        let mut missing_block_headers = BTreeMap::new();
        let mut all_verified_block_headers = Vec::new();
        let known_number_block_headers = 10;
        for i in 0..1000 {
            let verified_block_header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(1000 + i as Round, 0).build(),
            );
            let block_ref = verified_block_header.reference();
            if i < known_number_block_headers {
                // First 10 blocks are known by authorities 0, 2
                missing_block_headers.insert(
                    block_ref,
                    BTreeSet::from([
                        AuthorityIndex::new_for_test(0),
                        AuthorityIndex::new_for_test(2),
                    ]),
                );
            } else if i >= known_number_block_headers && i < 2 * known_number_block_headers {
                // Second 10 blocks are known by authorities 0, 3
                missing_block_headers.insert(
                    block_ref,
                    BTreeSet::from([
                        AuthorityIndex::new_for_test(0),
                        AuthorityIndex::new_for_test(3),
                    ]),
                );
            } else {
                // The rest are only known by authority 0
                missing_block_headers
                    .insert(block_ref, BTreeSet::from([AuthorityIndex::new_for_test(0)]));
            }
            all_verified_block_headers.push(verified_block_header);
        }

        // 3) Stub fetches for knowledge-based peers (2 and 3)
        let known_peers = [2, 3].map(AuthorityIndex::new_for_test);
        let known_vbhs_by_peer: Vec<(AuthorityIndex, Vec<VerifiedBlockHeader>)> = known_peers
            .iter()
            .map(|&peer| {
                let verified_block_headers = all_verified_block_headers
                    .iter()
                    .filter(|verified_block_header| {
                        missing_block_headers
                            .get(&verified_block_header.reference())
                            .unwrap()
                            .contains(&peer)
                    })
                    .take(context.parameters.max_headers_per_regular_sync_fetch)
                    .cloned()
                    .collect::<Vec<_>>();
                (peer, verified_block_headers)
            })
            .collect();

        for (peer, verified_block_headers) in known_vbhs_by_peer {
            if peer == AuthorityIndex::new_for_test(2) {
                // Simulate timeout for peer 2, then fall back to peer 5
                network_client
                    .stub_fetch_headers_response(
                        verified_block_headers.clone(),
                        peer,
                        Some(2 * FETCH_REQUEST_TIMEOUT),
                    )
                    .await;
                network_client
                    .stub_fetch_headers_response(
                        verified_block_headers.clone(),
                        AuthorityIndex::new_for_test(5),
                        None,
                    )
                    .await;
            } else {
                network_client
                    .stub_fetch_headers_response(verified_block_headers.clone(), peer, None)
                    .await;
            }
        }

        // 4) Stub responses for fetches from additional random peers (1 and 4 in tests)
        network_client
            .stub_fetch_headers_response(
                all_verified_block_headers
                    [0..context.parameters.max_headers_per_regular_sync_fetch]
                    .to_vec(),
                AuthorityIndex::new_for_test(1),
                None,
            )
            .await;

        network_client
            .stub_fetch_headers_response(
                all_verified_block_headers[context.parameters.max_headers_per_regular_sync_fetch
                    ..2 * context.parameters.max_headers_per_regular_sync_fetch]
                    .to_vec(),
                AuthorityIndex::new_for_test(4),
                None,
            )
            .await;

        // 5) Execute the fetch logic
        let results = HeaderSynchronizer::<
            MockNetworkClient,
            NoopBlockVerifier,
            SyncMockDispatcher,
        >::fetch_block_headers_from_authorities_periodic(
            context.clone(),
            inflight.clone(),
            network_client.clone(),
            missing_block_headers,
            dag_state.clone(),
        )
        .await;

        // 6) Assert we got 4 fetches: peer 2 (timed out) and fallback to 5 (first of
        //    the remaining peers), peer 3, and from 'random' 1 and 4
        assert_eq!(results.len(), 4, "Expected 2 known + 2 random fetches");

        // 7) First fetch from peer 3 (knowledge-based)
        let (_guard3, bytes3, peer3) = &results[0];
        assert_eq!(*peer3, AuthorityIndex::new_for_test(3));
        let expected2 = all_verified_block_headers
            [known_number_block_headers..2 * known_number_block_headers]
            .iter()
            .map(|vb| vb.serialized().clone())
            .collect::<Vec<_>>();
        assert_eq!(bytes3, &expected2);

        // 8) Second fetch from peer 1 (additional random)
        let (_guard1, bytes1, peer1) = &results[1];
        assert_eq!(*peer1, AuthorityIndex::new_for_test(1));
        let expected1 = all_verified_block_headers
            [0..context.parameters.max_headers_per_regular_sync_fetch]
            .iter()
            .map(|vb| vb.serialized().clone())
            .collect::<Vec<_>>();
        assert_eq!(bytes1, &expected1);

        // 9) Third fetch from peer 4 (additional random)
        let (_guard4, bytes4, peer4) = &results[2];
        assert_eq!(*peer4, AuthorityIndex::new_for_test(4));
        let expected4 =
            all_verified_block_headers[context.parameters.max_headers_per_regular_sync_fetch
                ..2 * context.parameters.max_headers_per_regular_sync_fetch]
                .iter()
                .map(|vb| vb.serialized().clone())
                .collect::<Vec<_>>();
        assert_eq!(bytes4, &expected4);

        // 10) Fourth fetch from peer 5 (fallback after peer 2 timeout)
        let (_guard5, bytes5, peer5) = &results[3];
        assert_eq!(*peer5, AuthorityIndex::new_for_test(5));
        let expected5 = all_verified_block_headers[0..known_number_block_headers]
            .iter()
            .map(|vb| vb.serialized().clone())
            .collect::<Vec<_>>();
        assert_eq!(bytes5, &expected5);
    }

    #[tokio::test]
    async fn test_process_fetched_headers_with_future_timestamp() {
        let validators = 4;
        let (context, _) = Context::new_for_test(validators);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));

        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());

        let network_client = Arc::new(MockNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );

        let handle = HeaderSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );

        // Create two block headers - one with a normal timestamp, one with a future
        // timestamp
        let normal_block_header = TestBlockHeader::new(1, 0)
            .set_timestamp_ms(context.clock.timestamp_utc_ms())
            .build();
        let future_block_header = TestBlockHeader::new(2, 1)
            .set_timestamp_ms(
                context.clock.timestamp_utc_ms() + Duration::from_secs(3600).as_millis() as u64,
            )
            .build();

        let normal_block_header = VerifiedBlockHeader::new_for_test(normal_block_header);
        let future_block_header = VerifiedBlockHeader::new_for_test(future_block_header);
        let headers_refs = [
            normal_block_header.reference(),
            future_block_header.reference(),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let peer = AuthorityIndex::new_for_test(1);
        network_client
            .stub_fetch_headers_response(
                [normal_block_header.clone(), future_block_header.clone()].to_vec(),
                peer,
                None,
            )
            .await;
        let _ = handle.fetch_headers(headers_refs, peer).await.is_ok();
        // Wait a little bit until synchronizer tries to add them into core
        sleep(Duration::from_millis(1_000)).await;

        // THEN ensure that the normal block header was added and block header with
        // future timestamp was ignored
        let added_block_headers = core_dispatcher.get_and_drain_block_headers().await;
        assert_eq!(added_block_headers.len(), 1);
        assert_eq!(added_block_headers[0], normal_block_header);

        // Stop synchronizer and ensure that no panic occurred
        if let Err(err) = handle.stop().await {
            if err.is_panic() {
                std::panic::resume_unwind(err.into_panic());
            }
        }
    }

    #[tokio::test]
    async fn test_process_fetched_blocks() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let store = Arc::new(MemStore::new(context.clone()));
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let (commands_sender, _commands_receiver) =
            monitored_mpsc::channel("consensus_synchronizer_commands", 1000);
        let network_client = Arc::new(MockNetworkClient::default());

        // Set up synchronizers
        let transactions_synchronizer = TransactionsSynchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            dag_state.clone(),
        );
        // Create input test blocks:
        // - Authority 0 block at round 60.
        // - Authority 1 blocks from round 30 to 60.
        let mut expected_block_headers = vec![VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(60, 0).build(),
        )];
        expected_block_headers.extend((30..=60).map(|round| {
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, 1).build())
        }));
        assert_eq!(expected_block_headers.len(), 32);

        let expected_serialized_block_headers = expected_block_headers
            .iter()
            .map(|b| b.serialized().clone())
            .collect::<Vec<_>>();

        let expected_block_refs = expected_block_headers
            .iter()
            .map(|b| b.reference())
            .collect::<BTreeSet<_>>();

        // GIVEN peer to fetch blocks from
        let peer_index = AuthorityIndex::new_for_test(2);

        // Create blocks_guard
        let inflight_blocks_map = InflightBlockHeadersMap::new();
        let blocks_guard = inflight_blocks_map
            .lock_headers(expected_block_refs.clone(), peer_index, SyncMethod::Live)
            .expect("Failed to lock blocks");

        assert_eq!(
            inflight_blocks_map.num_of_locked_headers(),
            expected_block_refs.len()
        );

        // Create a shared LruCache that will be reused to verify duplicate prevention
        let verified_cache = Arc::new(parking_lot::Mutex::new(lru::LruCache::new(
            NonZero::new(1000).unwrap(),
        )));

        // Create a Synchronizer
        let result = HeaderSynchronizer::<
            MockNetworkClient,
            NoopBlockVerifier,
            MockCoreThreadDispatcher,
        >::process_fetched_headers_from_authority(
            expected_serialized_block_headers.clone(),
            peer_index,
            blocks_guard, // The guard is consumed here
            core_dispatcher.clone(),
            block_verifier.clone(),
            verified_cache.clone(),
            commit_vote_monitor.clone(),
            transactions_synchronizer.clone(),
            context.clone(),
            commands_sender.clone(),
            "live",
        )
        .await;

        // THEN
        assert!(result.is_ok());

        // Check blocks were sent to core
        let added_block_headers = core_dispatcher.get_and_drain_block_headers().await;
        assert_eq!(
            added_block_headers
                .iter()
                .map(|b| b.reference())
                .collect::<BTreeSet<_>>(),
            expected_block_refs,
        );

        // Check blocks were unlocked
        assert_eq!(inflight_blocks_map.num_of_locked_headers(), 0);

        // PART 2: Verify LruCache prevents duplicate processing
        // Try to process the same block headers again (simulating duplicate fetch)
        let blocks_guard_second = inflight_blocks_map
            .lock_headers(expected_block_refs.clone(), peer_index, SyncMethod::Live)
            .expect("Failed to lock blocks for second call");

        let result_second = HeaderSynchronizer::<
            MockNetworkClient,
            NoopBlockVerifier,
            MockCoreThreadDispatcher,
        >::process_fetched_headers_from_authority(
            expected_serialized_block_headers,
            peer_index,
            blocks_guard_second,
            core_dispatcher.clone(),
            block_verifier,
            verified_cache.clone(),
            commit_vote_monitor,
            transactions_synchronizer,
            context.clone(),
            commands_sender,
            "live",
        )
        .await;

        assert!(result_second.is_ok());

        // Verify NO block headers were sent to core on the second call
        // because they were already in the LruCache
        let added_block_headers_second_call = core_dispatcher.get_and_drain_block_headers().await;
        assert!(
            added_block_headers_second_call.is_empty(),
            "Expected no block headers to be added on second call due to LruCache, but got {} headers",
            added_block_headers_second_call.len()
        );

        // Verify the cache contains all the block header digests
        let cache_size = verified_cache.lock().len();
        assert_eq!(
            cache_size,
            expected_block_refs.len(),
            "Expected {} entries in the LruCache, but got {}",
            expected_block_refs.len(),
            cache_size
        );
    }
}
