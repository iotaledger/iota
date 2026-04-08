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
use consensus_config::AuthorityIndex;
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
use rand::prelude::{IteratorRandom, SeedableRng, SliceRandom, StdRng};
use tap::TapFallible;
use tokio::{
    runtime::Handle,
    sync::{mpsc::error::TrySendError, oneshot},
    task::{JoinError, JoinSet},
    time::{Instant, sleep, sleep_until, timeout},
};
use tracing::{debug, error, info, instrument, trace, warn};

use crate::{
    BlockAPI, CommitIndex, Round,
    authority_service::COMMIT_LAG_MULTIPLIER,
    block::{BlockDigest, BlockRef, GENESIS_ROUND, SignedBlock, VerifiedBlock},
    block_verifier::BlockVerifier,
    commit_vote_monitor::CommitVoteMonitor,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
    network::NetworkClient,
    scoring_metrics_store::ErrorSource,
};

/// The number of concurrent fetch blocks requests per authority
const FETCH_BLOCKS_CONCURRENCY: usize = 5;

/// The maximal additional blocks (parents) that can be fetched.
// TODO: This is a temporary value, and should be removed once the protocol
// version is updated to support batching
pub(crate) const MAX_ADDITIONAL_BLOCKS: usize = 10;

/// The maximum number of verified block references to cache for deduplication.
const VERIFIED_BLOCKS_CACHE_CAP: usize = 200_000;

/// The timeout for synchronizer to fetch blocks from a given peer authority.
const FETCH_REQUEST_TIMEOUT: Duration = Duration::from_millis(2_000);

/// The timeout for periodic synchronizer to fetch blocks from the peers.
const FETCH_FROM_PEERS_TIMEOUT: Duration = Duration::from_millis(4_000);

/// The maximum number of authorities from which we will try to periodically
/// fetch blocks at the same moment. The guard will protect that we will not ask
/// from more than this number of authorities at the same time.
const MAX_AUTHORITIES_TO_FETCH_PER_BLOCK: usize = 2;

/// The maximum number of authorities from which the live synchronizer will try
/// to fetch blocks at the same moment. This is lower than the periodic sync
/// limit to prioritize periodic sync.
const MAX_AUTHORITIES_TO_LIVE_FETCH_PER_BLOCK: usize = 1;

/// The maximum number of peers from which the periodic synchronizer will
/// request blocks
const MAX_PERIODIC_SYNC_PEERS: usize = 4;

/// The maximum number of peers in the periodic synchronizer which are chosen
/// totally random to fetch blocks from. The other peers will be chosen based on
/// their knowledge of the DAG.
const MAX_PERIODIC_SYNC_RANDOM_PEERS: usize = 2;

/// Represents the different methods used for synchronization
#[derive(Clone)]
enum SyncMethod {
    Live,
    Periodic,
}

struct BlocksGuard {
    map: Arc<InflightBlocksMap>,
    block_refs: BTreeSet<BlockRef>,
    peer: AuthorityIndex,
    method: SyncMethod,
}

impl Drop for BlocksGuard {
    fn drop(&mut self) {
        self.map.unlock_blocks(&self.block_refs, self.peer);
    }
}

// Keeps a mapping between the missing blocks that have been instructed to be
// fetched and the authorities that are currently fetching them. For a block ref
// there is a maximum number of authorities that can concurrently fetch it. The
// authority ids that are currently fetching a block are set on the
// corresponding `BTreeSet` and basically they act as "locks".
struct InflightBlocksMap {
    inner: Mutex<HashMap<BlockRef, BTreeSet<AuthorityIndex>>>,
}

impl InflightBlocksMap {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(HashMap::new()),
        })
    }

    /// Locks the blocks to be fetched for the assigned `peer_index`. We want to
    /// avoid re-fetching the missing blocks from too many authorities at
    /// the same time, thus we limit the concurrency per block by attempting
    /// to lock per block. If a block is already fetched by the maximum allowed
    /// number of authorities, then the block ref will not be included in the
    /// returned set. The method returns all the block refs that have been
    /// successfully locked and allowed to be fetched.
    ///
    /// Different limits apply based on the sync method:
    /// - Periodic sync: Can lock if total authorities <
    ///   MAX_AUTHORITIES_TO_FETCH_PER_BLOCK (3)
    /// - Live sync: Can lock if total authorities <
    ///   MAX_AUTHORITIES_TO_LIVE_FETCH_PER_BLOCK (2)
    fn lock_blocks(
        self: &Arc<Self>,
        missing_block_refs: BTreeSet<BlockRef>,
        peer: AuthorityIndex,
        method: SyncMethod,
    ) -> Option<BlocksGuard> {
        let mut blocks = BTreeSet::new();
        let mut inner = self.inner.lock();

        for block_ref in missing_block_refs {
            let authorities = inner.entry(block_ref).or_default();

            // Check if this peer is already fetching this block
            if authorities.contains(&peer) {
                continue;
            }

            // Count total authorities currently fetching this block
            let total_count = authorities.len();

            // Determine the limit based on the sync method
            let max_limit = match method {
                SyncMethod::Live => MAX_AUTHORITIES_TO_LIVE_FETCH_PER_BLOCK,
                SyncMethod::Periodic => MAX_AUTHORITIES_TO_FETCH_PER_BLOCK,
            };

            // Check if we can acquire the lock
            if total_count < max_limit {
                assert!(authorities.insert(peer));
                blocks.insert(block_ref);
            }
        }

        if blocks.is_empty() {
            None
        } else {
            Some(BlocksGuard {
                map: self.clone(),
                block_refs: blocks,
                peer,
                method,
            })
        }
    }

    /// Unlocks the provided block references for the given `peer`. The
    /// unlocking is strict, meaning that if this method is called for a
    /// specific block ref and peer more times than the corresponding lock
    /// has been called, it will panic.
    fn unlock_blocks(self: &Arc<Self>, block_refs: &BTreeSet<BlockRef>, peer: AuthorityIndex) {
        // Now mark all the blocks as fetched from the map
        let mut blocks_to_fetch = self.inner.lock();
        for block_ref in block_refs {
            let authorities = blocks_to_fetch
                .get_mut(block_ref)
                .expect("Should have found a non empty map");

            assert!(authorities.remove(&peer), "Peer index should be present!");

            // if the last one then just clean up
            if authorities.is_empty() {
                blocks_to_fetch.remove(block_ref);
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
        let method = blocks_guard.method.clone();

        // Explicitly drop the guard
        drop(blocks_guard);

        // Now create a new guard with the same sync method
        self.lock_blocks(block_refs, peer, method)
    }

    #[cfg(test)]
    fn num_of_locked_blocks(self: &Arc<Self>) -> usize {
        let inner = self.inner.lock();
        inner.len()
    }
}

enum Command {
    FetchBlocks {
        missing_block_refs: BTreeSet<BlockRef>,
        peer_index: AuthorityIndex,
        result: oneshot::Sender<Result<(), ConsensusError>>,
    },
    FetchOwnLastBlock,
    KickOffScheduler,
}

pub(crate) struct SynchronizerHandle {
    commands_sender: Sender<Command>,
    tasks: tokio::sync::Mutex<JoinSet<()>>,
}

impl SynchronizerHandle {
    /// Explicitly asks from the synchronizer to fetch the blocks - provided the
    /// block_refs set - from the peer authority.
    pub(crate) async fn fetch_blocks(
        &self,
        missing_block_refs: BTreeSet<BlockRef>,
        peer_index: AuthorityIndex,
    ) -> ConsensusResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.commands_sender
            .send(Command::FetchBlocks {
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
            result?
        }
        Ok(())
    }
}

/// `Synchronizer` oversees live block synchronization, crucial for node
/// progress. Live synchronization refers to the process of retrieving missing
/// blocks, particularly those essential for advancing a node when data from
/// only a few rounds is absent. If a node significantly lags behind the
/// network, `commit_syncer` handles fetching missing blocks via a more
/// efficient approach. `Synchronizer` aims for swift catch-up employing two
/// mechanisms:
///
/// 1. Explicitly requesting missing blocks from designated authorities via the
///    "block send" path. This includes attempting to fetch any missing
///    ancestors necessary for processing a received block. Such requests
///    prioritize the block author, maximizing the chance of prompt retrieval. A
///    locking mechanism allows concurrent requests for missing blocks from up
///    to two authorities simultaneously, enhancing the chances of timely
///    retrieval. Notably, if additional missing blocks arise during block
///    processing, requests to the same authority are deferred to the scheduler.
///
/// 2. Periodically requesting missing blocks via a scheduler. This primarily
///    serves to retrieve missing blocks that were not ancestors of a received
///    block via the "block send" path. The scheduler operates on either a fixed
///    periodic basis or is triggered immediately after explicit fetches
///    described in (1), ensuring continued block retrieval if gaps persist.
///
/// Additionally to the above, the synchronizer can synchronize and fetch the
/// last own proposed block from the network peers as best effort approach to
/// recover node from amnesia and avoid making the node equivocate.
pub(crate) struct Synchronizer<C: NetworkClient, V: BlockVerifier, D: CoreThreadDispatcher> {
    context: Arc<Context>,
    commands_receiver: Receiver<Command>,
    fetch_block_senders: BTreeMap<AuthorityIndex, Sender<BlocksGuard>>,
    core_dispatcher: Arc<D>,
    commit_vote_monitor: Arc<CommitVoteMonitor>,
    dag_state: Arc<RwLock<DagState>>,
    fetch_blocks_scheduler_task: JoinSet<()>,
    fetch_own_last_block_task: JoinSet<()>,
    network_client: Arc<C>,
    block_verifier: Arc<V>,
    inflight_blocks_map: Arc<InflightBlocksMap>,
    verified_blocks_cache: Arc<Mutex<LruCache<BlockDigest, ()>>>,
    commands_sender: Sender<Command>,
}

impl<C: NetworkClient, V: BlockVerifier, D: CoreThreadDispatcher> Synchronizer<C, V, D> {
    /// Starts the synchronizer, which is responsible for fetching blocks from
    /// other authorities and managing block synchronization tasks.
    pub fn start(
        network_client: Arc<C>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        commit_vote_monitor: Arc<CommitVoteMonitor>,
        block_verifier: Arc<V>,
        dag_state: Arc<RwLock<DagState>>,
        sync_last_known_own_block: bool,
    ) -> Arc<SynchronizerHandle> {
        let (commands_sender, commands_receiver) =
            channel("consensus_synchronizer_commands", 1_000);
        let inflight_blocks_map = InflightBlocksMap::new();
        let verified_blocks_cache = Arc::new(Mutex::new(LruCache::new(
            NonZeroUsize::new(VERIFIED_BLOCKS_CACHE_CAP).unwrap(),
        )));

        // Spawn the tasks to fetch the blocks from the others
        let mut fetch_block_senders = BTreeMap::new();
        let mut tasks = JoinSet::new();
        for (index, _) in context.committee.authorities() {
            if index == context.own_index {
                continue;
            }
            let (sender, receiver) =
                channel("consensus_synchronizer_fetches", FETCH_BLOCKS_CONCURRENCY);
            let fetch_blocks_from_authority_async = Self::fetch_blocks_from_authority(
                index,
                network_client.clone(),
                block_verifier.clone(),
                verified_blocks_cache.clone(),
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
                .try_send(Command::FetchOwnLastBlock)
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
                fetch_blocks_scheduler_task: JoinSet::new(),
                fetch_own_last_block_task: JoinSet::new(),
                network_client,
                block_verifier,
                inflight_blocks_map,
                verified_blocks_cache,
                commands_sender: commands_sender_clone,
                dag_state,
            };
            s.run().await;
        }));

        Arc::new(SynchronizerHandle {
            commands_sender,
            tasks: tokio::sync::Mutex::new(tasks),
        })
    }

    // The main loop to listen for the submitted commands.
    async fn run(&mut self) {
        // We want the synchronizer to run periodically every 200ms to fetch any missing
        // blocks.
        const PERIODIC_FETCH_TIMEOUT: Duration = Duration::from_millis(200);
        let scheduler_timeout = sleep_until(Instant::now() + PERIODIC_FETCH_TIMEOUT);

        tokio::pin!(scheduler_timeout);

        loop {
            tokio::select! {
                Some(command) = self.commands_receiver.recv() => {
                    match command {
                        Command::FetchBlocks{ missing_block_refs, peer_index, result } => {
                            if peer_index == self.context.own_index {
                                error!("We should never attempt to fetch blocks from our own node");
                                continue;
                            }

                            let peer_hostname = self.context.committee.authority(peer_index).hostname.clone();

                            // Keep only the max allowed blocks to request. It is ok to reduce here as the scheduler
                            // task will take care syncing whatever is leftover.
                            let missing_block_refs = missing_block_refs
                                .into_iter()
                                .take(self.context.parameters.max_blocks_per_sync)
                                .collect();

                            let blocks_guard = self.inflight_blocks_map.lock_blocks(missing_block_refs, peer_index, SyncMethod::Live);
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
                                        TrySendError::Full(_) => ConsensusError::SynchronizerSaturated(peer_index,peer_hostname),
                                        TrySendError::Closed(_) => ConsensusError::Shutdown
                                    }
                                });

                            result.send(r).ok();
                        }
                        Command::FetchOwnLastBlock => {
                            if self.fetch_own_last_block_task.is_empty() {
                                self.start_fetch_own_last_block_task();
                            }
                        }
                        Command::KickOffScheduler => {
                            // just reset the scheduler timeout timer to run immediately if not already running.
                            // If the scheduler is already running then just reduce the remaining time to run.
                            let timeout = if self.fetch_blocks_scheduler_task.is_empty() {
                                Instant::now()
                            } else {
                                Instant::now() + PERIODIC_FETCH_TIMEOUT.checked_div(2).unwrap()
                            };

                            // only reset if it is earlier than the next deadline
                            if timeout < scheduler_timeout.deadline() {
                                scheduler_timeout.as_mut().reset(timeout);
                            }
                        }
                    }
                },
                Some(result) = self.fetch_own_last_block_task.join_next(), if !self.fetch_own_last_block_task.is_empty() => {
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
                Some(result) = self.fetch_blocks_scheduler_task.join_next(), if !self.fetch_blocks_scheduler_task.is_empty() => {
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
                    if self.fetch_blocks_scheduler_task.is_empty() {
                        if let Err(err) = self.start_fetch_missing_blocks_task().await {
                            debug!("Core is shutting down, synchronizer is shutting down: {err:?}");
                            return;
                        };
                    }

                    scheduler_timeout
                        .as_mut()
                        .reset(Instant::now() + PERIODIC_FETCH_TIMEOUT);
                }
            }
        }
    }

    async fn fetch_blocks_from_authority(
        peer_index: AuthorityIndex,
        network_client: Arc<C>,
        block_verifier: Arc<V>,
        verified_cache: Arc<Mutex<LruCache<BlockDigest, ()>>>,
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
                Some(blocks_guard) = receiver.recv(), if requests.len() < FETCH_BLOCKS_CONCURRENCY => {
                    // get the highest accepted rounds
                    let highest_rounds = Self::get_highest_accepted_rounds(dag_state.clone(), &context);

                    // Record metrics for live synchronizer requests
                    let metrics = &context.metrics.node_metrics;
                    metrics
                        .synchronizer_requested_blocks_by_peer
                        .with_label_values(&[peer_hostname.as_str(), "live"])
                        .inc_by(blocks_guard.block_refs.len() as u64);
                    // Count requested blocks per authority and increment metric by one per authority
                    let mut authors = HashSet::new();
                    for block_ref in &blocks_guard.block_refs {
                        authors.insert(block_ref.author);
                    }
                    for author in authors {
                        let host = &context.committee.authority(author).hostname;
                        metrics
                            .synchronizer_requested_blocks_by_authority
                            .with_label_values(&[host.as_str(), "live"])
                            .inc();
                    }

                    requests.push(Self::fetch_blocks_request(
                        network_client.clone(),
                        peer_index,
                        blocks_guard,
                        highest_rounds,
                        FETCH_REQUEST_TIMEOUT,
                        1,
                    ))
                },
                Some((response, blocks_guard, retries, _peer, highest_rounds)) = requests.next() => {
                    match response {
                        Ok(blocks) => {
                            if let Err(err) = Self::process_fetched_blocks(blocks,
                                peer_index,
                                blocks_guard,
                                core_dispatcher.clone(),
                                block_verifier.clone(),
                                verified_cache.clone(),
                                commit_vote_monitor.clone(),
                                context.clone(),
                                commands_sender.clone(),
                                "live"
                            ).await {
                                context.scoring_metrics_store.update_scoring_metrics_on_block_receival(
                                    peer_index,
                                    peer_hostname,
                                    err.clone(),
                                    ErrorSource::Synchronizer,
                                    &context.metrics.node_metrics,
                                );
                                warn!("Error while processing fetched blocks from peer {peer_index} {peer_hostname}: {err}");
                                context.metrics.node_metrics.synchronizer_process_fetched_failures_by_peer.with_label_values(&[peer_hostname.as_str(), "live"]).inc();
                            }
                        },
                        Err(_) => {
                            context.metrics.node_metrics.synchronizer_fetch_failures_by_peer.with_label_values(&[peer_hostname.as_str(), "live"]).inc();
                            if retries <= MAX_RETRIES {
                                requests.push(Self::fetch_blocks_request(network_client.clone(), peer_index, blocks_guard, highest_rounds, FETCH_REQUEST_TIMEOUT, retries))
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

    /// Processes the requested raw fetched blocks from peer `peer_index`. If no
    /// error is returned then the verified blocks are immediately sent to
    /// Core for processing.
    async fn process_fetched_blocks(
        mut serialized_blocks: Vec<Bytes>,
        peer_index: AuthorityIndex,
        requested_blocks_guard: BlocksGuard,
        core_dispatcher: Arc<D>,
        block_verifier: Arc<V>,
        verified_cache: Arc<Mutex<LruCache<BlockDigest, ()>>>,
        commit_vote_monitor: Arc<CommitVoteMonitor>,
        context: Arc<Context>,
        commands_sender: Sender<Command>,
        sync_method: &str,
    ) -> ConsensusResult<()> {
        if serialized_blocks.is_empty() {
            return Ok(());
        }
        let _s = context
            .metrics
            .node_metrics
            .scope_processing_time
            .with_label_values(&["Synchronizer::process_fetched_blocks"])
            .start_timer();

        // Limit the number of the returned blocks processed.
        if context.protocol_config.consensus_batched_block_sync() {
            serialized_blocks.truncate(context.parameters.max_blocks_per_sync);
        } else {
            // Ensure that all the returned blocks do not go over the total max allowed
            // returned blocks
            if serialized_blocks.len()
                > requested_blocks_guard.block_refs.len() + MAX_ADDITIONAL_BLOCKS
            {
                return Err(ConsensusError::TooManyFetchedBlocksReturned(peer_index));
            }
        }

        // Verify all the fetched blocks
        let blocks = Handle::current()
            .spawn_blocking({
                let block_verifier = block_verifier.clone();
                let verified_cache = verified_cache.clone();
                let context = context.clone();
                let sync_method = sync_method.to_string();
                move || {
                    Self::verify_blocks(
                        serialized_blocks,
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

        if !context.protocol_config.consensus_batched_block_sync() {
            // Get all the ancestors of the requested blocks only
            let ancestors = blocks
                .iter()
                .filter(|b| requested_blocks_guard.block_refs.contains(&b.reference()))
                .flat_map(|b| b.ancestors().to_vec())
                .collect::<BTreeSet<BlockRef>>();

            // Now confirm that the blocks are either between the ones requested, or they
            // are parents of the requested blocks
            for block in &blocks {
                if !requested_blocks_guard
                    .block_refs
                    .contains(&block.reference())
                    && !ancestors.contains(&block.reference())
                {
                    return Err(ConsensusError::UnexpectedFetchedBlock {
                        index: peer_index,
                        block_ref: block.reference(),
                    });
                }
            }
        }

        // Record commit votes from the verified blocks.
        for block in &blocks {
            commit_vote_monitor.observe_block(block);
        }

        let metrics = &context.metrics.node_metrics;
        let peer_hostname = &context.committee.authority(peer_index).hostname;
        metrics
            .synchronizer_fetched_blocks_by_peer
            .with_label_values(&[peer_hostname.as_str(), sync_method])
            .inc_by(blocks.len() as u64);
        for block in &blocks {
            let block_hostname = &context.committee.authority(block.author()).hostname;
            metrics
                .synchronizer_fetched_blocks_by_authority
                .with_label_values(&[block_hostname.as_str(), sync_method])
                .inc();
        }

        debug!(
            "Synced {} missing blocks from peer {peer_index} {peer_hostname}: {}",
            blocks.len(),
            blocks.iter().map(|b| b.reference().to_string()).join(", "),
        );

        // Now send them to core for processing. Ignore the returned missing blocks as
        // we don't want this mechanism to keep feedback looping on fetching
        // more blocks. The periodic synchronization will take care of that.
        let missing_blocks = core_dispatcher
            .add_blocks(blocks)
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
            .missing_blocks_after_fetch_total
            .inc_by(missing_blocks.len() as u64);

        Ok(())
    }

    fn get_highest_accepted_rounds(
        dag_state: Arc<RwLock<DagState>>,
        context: &Arc<Context>,
    ) -> Vec<Round> {
        let blocks = dag_state
            .read()
            .get_last_cached_block_per_authority(Round::MAX);
        assert_eq!(blocks.len(), context.committee.size());

        blocks
            .into_iter()
            .map(|(block, _)| block.round())
            .collect::<Vec<_>>()
    }

    #[instrument(level = "trace", skip_all)]
    fn verify_blocks(
        serialized_blocks: Vec<Bytes>,
        block_verifier: Arc<V>,
        verified_cache: Arc<Mutex<LruCache<BlockDigest, ()>>>,
        context: &Context,
        peer_index: AuthorityIndex,
        sync_method: &str,
    ) -> ConsensusResult<Vec<VerifiedBlock>> {
        let mut verified_blocks = Vec::new();
        let mut skipped_count = 0u64;

        for serialized_block in serialized_blocks {
            let block_digest = VerifiedBlock::compute_digest(&serialized_block);

            // Check if this block has already been verified
            if verified_cache.lock().get(&block_digest).is_some() {
                skipped_count += 1;
                continue; // Skip already verified blocks
            }

            let signed_block: SignedBlock =
                bcs::from_bytes(&serialized_block).map_err(ConsensusError::MalformedBlock)?;

            if let Err(e) = block_verifier.verify(&signed_block) {
                // TODO: we might want to use a different metric to track the invalid "served"
                // blocks from the invalid "proposed" ones.
                let hostname = context.committee.authority(peer_index).hostname.clone();

                context
                    .metrics
                    .node_metrics
                    .invalid_blocks
                    .with_label_values(&[hostname.as_str(), "synchronizer", e.name()])
                    .inc();
                warn!("Invalid block received from {}: {}", peer_index, e);
                return Err(e);
            }

            // Add block to verified cache after successful verification
            verified_cache.lock().put(block_digest, ());

            let verified_block = VerifiedBlock::new_verified_with_digest(
                signed_block,
                serialized_block,
                block_digest,
            );

            // Dropping is ok because the block will be refetched.
            // TODO: improve efficiency, maybe suspend and continue processing the block
            // asynchronously.
            let now = context.clock.timestamp_utc_ms();
            let drift = verified_block.timestamp_ms().saturating_sub(now) as u64;
            if drift > 0 {
                let peer_hostname = &context
                    .committee
                    .authority(verified_block.author())
                    .hostname;
                context
                    .metrics
                    .node_metrics
                    .block_timestamp_drift_ms
                    .with_label_values(&[peer_hostname.as_str(), "synchronizer"])
                    .inc_by(drift);

                if context
                    .protocol_config
                    .consensus_median_timestamp_with_checkpoint_enforcement()
                {
                    trace!(
                        "Synced block {} timestamp {} is in the future (now={}). Will not ignore as median based timestamp is enabled.",
                        verified_block.reference(),
                        verified_block.timestamp_ms(),
                        now
                    );
                } else {
                    warn!(
                        "Synced block {} timestamp {} is in the future (now={}). Ignoring.",
                        verified_block.reference(),
                        verified_block.timestamp_ms(),
                        now
                    );
                    continue;
                }
            }

            verified_blocks.push(verified_block);
        }

        // Record skipped blocks metric
        if skipped_count > 0 {
            let peer_hostname = &context.committee.authority(peer_index).hostname;
            context
                .metrics
                .node_metrics
                .synchronizer_skipped_blocks_by_peer
                .with_label_values(&[peer_hostname.as_str(), sync_method])
                .inc_by(skipped_count);
        }

        Ok(verified_blocks)
    }

    async fn fetch_blocks_request(
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
            network_client.fetch_blocks(
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

    fn start_fetch_own_last_block_task(&mut self) {
        const FETCH_OWN_BLOCK_RETRY_DELAY: Duration = Duration::from_millis(1_000);
        const MAX_RETRY_DELAY_STEP: Duration = Duration::from_millis(4_000);

        let context = self.context.clone();
        let dag_state = self.dag_state.clone();
        let network_client = self.network_client.clone();
        let block_verifier = self.block_verifier.clone();
        let core_dispatcher = self.core_dispatcher.clone();

        self.fetch_own_last_block_task
            .spawn(monitored_future!(async move {
                let _scope = monitored_scope("FetchOwnLastBlockTask");

                let fetch_own_block = |authority_index: AuthorityIndex, fetch_own_block_delay: Duration| {
                    let network_client_cloned = network_client.clone();
                    let own_index = context.own_index;
                    async move {
                        sleep(fetch_own_block_delay).await;
                        let r = network_client_cloned.fetch_latest_blocks(authority_index, vec![own_index], FETCH_REQUEST_TIMEOUT).await;
                        (r, authority_index)
                    }
                };

                let process_blocks = |blocks: Vec<Bytes>, authority_index: AuthorityIndex| -> ConsensusResult<Vec<VerifiedBlock>> {
                                    let mut result = Vec::new();
                                    for serialized_block in blocks {
                                        let signed_block: SignedBlock = bcs::from_bytes(&serialized_block).map_err(ConsensusError::MalformedBlock)?;
                                        block_verifier.verify(&signed_block).tap_err(|err|{
                                            let hostname = context.committee.authority(authority_index).hostname.clone();
                                            context
                                                .metrics
                                                .node_metrics
                                                .invalid_blocks
                                                .with_label_values(&[hostname.as_str(), "synchronizer_own_block", err.clone().name()])
                                                .inc();
                                            warn!("Invalid block received from {}: {}", authority_index, err);
                                        })?;

                                        let verified_block = VerifiedBlock::new_verified(signed_block, serialized_block);
                                        if verified_block.author() != context.own_index {
                                            return Err(ConsensusError::UnexpectedLastOwnBlock { index: authority_index, block_ref: verified_block.reference()});
                                        }
                                        result.push(verified_block);
                                    }
                                    Ok(result)
                };

                // Get the highest of all the results. Retry until at least `f+1` results have been gathered.
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
                        highest_round = dag_state.read().get_last_proposed_block().round();
                        info!("Only one node in the network, will not try fetching own last block from peers.");
                        break 'main;
                    }

                    // Ask all the other peers about our last block
                    let mut results = FuturesUnordered::new();

                    for (authority_index, _authority) in context.committee.authorities() {
                        // Skip our own index and the ones that have already responded
                        if !received_response[authority_index] {
                            results.push(fetch_own_block(authority_index, Duration::from_millis(0)));
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
                                        match process_blocks(result, authority_index) {
                                            Ok(blocks) => {
                                                received_response[authority_index] = true;
                                                let max_round = blocks.into_iter().map(|b|b.round()).max().unwrap_or(0);
                                                highest_round = highest_round.max(max_round);

                                                total_stake += context.committee.stake(authority_index);
                                            },
                                            Err(err) => {
                                                warn!("Invalid result returned from {authority_index} while fetching last own block: {err}");
                                            }
                                        }
                                    },
                                    Err(err) => {
                                        warn!("Error {err} while fetching our own block from peer {authority_index}. Will retry.");
                                        results.push(fetch_own_block(authority_index, FETCH_OWN_BLOCK_RETRY_DELAY));
                                    }
                                }
                            },
                            () = &mut timer => {
                                info!("Timeout while trying to sync our own last block from peers");
                                break 'inner;
                            }
                        }
                    }

                    // Request at least a quorum of 2f+1 stake to have replied back.
                    if context.committee.reached_quorum(total_stake) {
                        info!("A quorum, {} out of {} total stake, returned acceptable results for our own last block with highest round {}, with {retries} retries.", total_stake, context.committee.total_stake(), highest_round);
                        break 'main;
                    } else {
                        info!("Only {} out of {} total stake returned acceptable results for our own last block with highest round {}, with {retries} retries.", total_stake, context.committee.total_stake(), highest_round);
                    }

                    retries += 1;
                    context.metrics.node_metrics.sync_last_known_own_block_retries.inc();
                    warn!("Not enough stake: {} out of {} total stake returned acceptable results for our own last block with highest round {}. Will now retry {retries}.", total_stake, context.committee.total_stake(), highest_round);

                    sleep(retry_delay_step).await;

                    retry_delay_step = Duration::from_secs_f64(retry_delay_step.as_secs_f64() * 1.5);
                    retry_delay_step = retry_delay_step.min(MAX_RETRY_DELAY_STEP);
                }

                // Update the Core with the highest detected round
                context.metrics.node_metrics.last_known_own_block_round.set(highest_round as i64);

                if let Err(err) = core_dispatcher.set_last_known_proposed_round(highest_round) {
                    warn!("Error received while calling dispatcher, probably dispatcher is shutting down, will now exit: {err:?}");
                }
            }));
    }

    async fn start_fetch_missing_blocks_task(&mut self) -> ConsensusResult<()> {
        let mut missing_blocks = self
            .core_dispatcher
            .get_missing_blocks()
            .await
            .map_err(|_err| ConsensusError::Shutdown)?;

        // No reason to kick off the scheduler if there are no missing blocks to fetch
        if missing_blocks.is_empty() {
            return Ok(());
        }

        let context = self.context.clone();
        let network_client = self.network_client.clone();
        let block_verifier = self.block_verifier.clone();
        let verified_cache = self.verified_blocks_cache.clone();
        let commit_vote_monitor = self.commit_vote_monitor.clone();
        let core_dispatcher = self.core_dispatcher.clone();
        let blocks_to_fetch = self.inflight_blocks_map.clone();
        let commands_sender = self.commands_sender.clone();
        let dag_state = self.dag_state.clone();

        let (commit_lagging, last_commit_index, quorum_commit_index) = self.is_commit_lagging();
        trace!(
            "Commit lagging: {commit_lagging}, last commit index: {last_commit_index}, quorum commit index: {quorum_commit_index}"
        );
        if commit_lagging {
            // If gc is enabled and we are commit lagging, then we don't want to enable the
            // scheduler. As the new logic of processing the certified commits
            // takes place we are guaranteed that commits will happen for all the certified
            // commits.
            if dag_state.read().gc_enabled() {
                return Ok(());
            }

            // As node is commit lagging try to sync only the missing blocks that are within
            // the acceptable round thresholds to sync. The rest we don't attempt to
            // sync yet.
            let highest_accepted_round = dag_state.read().highest_accepted_round();
            missing_blocks = missing_blocks
                .into_iter()
                .take_while(|(block_ref, _)| {
                    block_ref.round <= highest_accepted_round + self.missing_block_round_threshold()
                })
                .collect::<BTreeMap<_, _>>();

            // If no missing blocks are within the acceptable thresholds to sync while we
            // commit lag, then we disable the scheduler completely for this run.
            if missing_blocks.is_empty() {
                trace!(
                    "Scheduled synchronizer temporarily disabled as local commit is falling behind from quorum {last_commit_index} << {quorum_commit_index} and missing blocks are too far in the future."
                );
                self.context
                    .metrics
                    .node_metrics
                    .fetch_blocks_scheduler_skipped
                    .with_label_values(&["commit_lagging"])
                    .inc();
                return Ok(());
            }
        }

        self.fetch_blocks_scheduler_task
            .spawn(monitored_future!(async move {
                let _scope = monitored_scope("FetchMissingBlocksScheduler");

                context
                    .metrics
                    .node_metrics
                    .fetch_blocks_scheduler_inflight
                    .inc();
                let total_requested = missing_blocks.len();

                fail_point_async!("consensus-delay");

                // Fetch blocks from peers
                let results = Self::fetch_blocks_from_authorities(
                    context.clone(),
                    blocks_to_fetch.clone(),
                    network_client,
                    missing_blocks,
                    dag_state,
                )
                .await;
                context
                    .metrics
                    .node_metrics
                    .fetch_blocks_scheduler_inflight
                    .dec();
                if results.is_empty() {
                    warn!("No results returned while requesting missing blocks");
                    return;
                }

                // Now process the returned results
                let mut total_fetched = 0;
                for (blocks_guard, fetched_blocks, peer) in results {
                    total_fetched += fetched_blocks.len();

                    if let Err(err) = Self::process_fetched_blocks(
                        fetched_blocks,
                        peer,
                        blocks_guard,
                        core_dispatcher.clone(),
                        block_verifier.clone(),
                        verified_cache.clone(),
                        commit_vote_monitor.clone(),
                        context.clone(),
                        commands_sender.clone(),
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
    /// missing blocks via the periodic synchronization. Any missing blocks
    /// of higher rounds are considered too far in the future to fetch. This
    /// property is used only when it's detected that the node has fallen
    /// behind on its commit compared to the rest of the network,
    /// otherwise scheduler will attempt to fetch any missing block.
    fn missing_block_round_threshold(&self) -> Round {
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
    async fn fetch_blocks_from_authorities(
        context: Arc<Context>,
        inflight_blocks: Arc<InflightBlocksMap>,
        network_client: Arc<C>,
        missing_blocks: BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        dag_state: Arc<RwLock<DagState>>,
    ) -> Vec<(BlocksGuard, Vec<Bytes>, AuthorityIndex)> {
        // Step 1: Map authorities to missing blocks that they are aware of
        let mut authority_to_blocks: HashMap<AuthorityIndex, Vec<BlockRef>> = HashMap::new();
        for (missing_block_ref, authorities) in &missing_blocks {
            for author in authorities {
                if author == &context.own_index {
                    // Skip our own index as we don't want to fetch blocks from ourselves
                    continue;
                }
                authority_to_blocks
                    .entry(*author)
                    .or_default()
                    .push(*missing_block_ref);
            }
        }

        // Step 2: Choose at most MAX_PEERS-MAX_RANDOM_PEERS peers from those who are
        // aware of some missing blocks

        #[cfg(not(test))]
        let mut rng = StdRng::from_entropy();

        // Randomly pick up MAX_PEERS - MAX_RANDOM_PEERS authorities that are aware of
        // missing blocks
        #[cfg(not(test))]
        let mut chosen_peers_with_blocks: Vec<(AuthorityIndex, Vec<BlockRef>, &str)> =
            authority_to_blocks
                .iter()
                .choose_multiple(
                    &mut rng,
                    MAX_PERIODIC_SYNC_PEERS - MAX_PERIODIC_SYNC_RANDOM_PEERS,
                )
                .into_iter()
                .map(|(&peer, blocks)| {
                    let limited_blocks = blocks
                        .iter()
                        .copied()
                        .take(context.parameters.max_blocks_per_sync)
                        .collect();
                    (peer, limited_blocks, "periodic_known")
                })
                .collect();
        #[cfg(test)]
        // Deterministically pick the smallest (MAX_PEERS - MAX_RANDOM_PEERS) authority indices
        let mut chosen_peers_with_blocks: Vec<(AuthorityIndex, Vec<BlockRef>, &str)> = {
            let mut items: Vec<(AuthorityIndex, Vec<BlockRef>, &str)> = authority_to_blocks
                .iter()
                .map(|(&peer, blocks)| {
                    let limited_blocks = blocks
                        .iter()
                        .copied()
                        .take(context.parameters.max_blocks_per_sync)
                        .collect();
                    (peer, limited_blocks, "periodic_known")
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

        // Step 3: Choose at most two random peers not known to be aware of the missing
        // blocks
        let already_chosen: HashSet<AuthorityIndex> = chosen_peers_with_blocks
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
        let mut all_missing_blocks: Vec<BlockRef> = missing_blocks.keys().cloned().collect();
        // Shuffle the missing blocks in case the first ones are blocked by irresponsive
        // peers
        #[cfg(not(test))]
        all_missing_blocks.shuffle(&mut rng);

        let mut block_chunks = all_missing_blocks.chunks(context.parameters.max_blocks_per_sync);

        for peer in random_peers {
            if let Some(chunk) = block_chunks.next() {
                chosen_peers_with_blocks.push((peer, chunk.to_vec(), "periodic_random"));
            } else {
                break;
            }
        }

        let mut request_futures = FuturesUnordered::new();

        let highest_rounds = Self::get_highest_accepted_rounds(dag_state, &context);

        // Record the missing blocks per authority for metrics
        let mut missing_blocks_per_authority = vec![0; context.committee.size()];
        for block in &all_missing_blocks {
            missing_blocks_per_authority[block.author] += 1;
        }
        for (missing, (_, authority)) in missing_blocks_per_authority
            .into_iter()
            .zip(context.committee.authorities())
        {
            context
                .metrics
                .node_metrics
                .synchronizer_missing_blocks_by_authority
                .with_label_values(&[&authority.hostname])
                .inc_by(missing as u64);
            context
                .metrics
                .node_metrics
                .synchronizer_current_missing_blocks_by_authority
                .with_label_values(&[&authority.hostname])
                .set(missing as i64);
        }

        // Look at peers that were not chosen yet and try to fetch blocks from them if
        // needed later
        #[cfg_attr(test, expect(unused_mut))]
        let mut remaining_peers: Vec<_> = context
            .committee
            .authorities()
            .filter_map(|(peer_index, _)| {
                if peer_index != context.own_index
                    && !chosen_peers_with_blocks
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
        for (peer, blocks_to_request, label) in chosen_peers_with_blocks {
            let peer_hostname = &context.committee.authority(peer).hostname;
            let block_refs = blocks_to_request.iter().cloned().collect::<BTreeSet<_>>();

            // Lock the blocks to be fetched. If no lock can be acquired for any of the
            // blocks then don't bother.
            if let Some(blocks_guard) =
                inflight_blocks.lock_blocks(block_refs.clone(), peer, SyncMethod::Periodic)
            {
                info!(
                    "Periodic sync of {} missing blocks from peer {} {}: {}",
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
                    .synchronizer_requested_blocks_by_peer
                    .with_label_values(&[peer_hostname.as_str(), label])
                    .inc_by(block_refs.len() as u64);
                for block_ref in &block_refs {
                    let block_hostname = &context.committee.authority(block_ref.author).hostname;
                    metrics
                        .synchronizer_requested_blocks_by_authority
                        .with_label_values(&[block_hostname.as_str(), label])
                        .inc();
                }
                request_futures.push(Self::fetch_blocks_request(
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
                        Ok(fetched_blocks) => {
                            info!("Fetched {} blocks from peer {}", fetched_blocks.len(), peer_hostname);
                            results.push((blocks_guard, fetched_blocks, peer_index));

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
                                if let Some(blocks_guard) = inflight_blocks.swap_locks(blocks_guard, next_peer) {
                                    info!(
                                        "Retrying syncing {} missing blocks from peer {}: {}",
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
                                        .synchronizer_requested_blocks_by_peer
                                        .with_label_values(&[peer_hostname.as_str(), "periodic_retry"])
                                        .inc_by(block_refs.len() as u64);
                                    for block_ref in &block_refs {
                                        let block_hostname =
                                            &context.committee.authority(block_ref.author).hostname;
                                        metrics
                                            .synchronizer_requested_blocks_by_authority
                                            .with_label_values(&[block_hostname.as_str(), "periodic_retry"])
                                            .inc();
                                    }
                                    request_futures.push(Self::fetch_blocks_request(
                                        network_client.clone(),
                                        next_peer,
                                        blocks_guard,
                                        highest_rounds,
                                        FETCH_REQUEST_TIMEOUT,
                                        1,
                                    ));
                                } else {
                                    debug!("Couldn't acquire locks to fetch blocks from peer {next_peer}.")
                                }
                            } else {
                                debug!("No more peers left to fetch blocks");
                            }
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
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::NonZeroUsize,
        sync::Arc,
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use consensus_config::{AuthorityIndex, Parameters};
    use iota_metrics::monitored_mpsc;
    use lru::LruCache;
    use parking_lot::{Mutex as SyncMutex, RwLock};
    use tokio::{sync::Mutex, time::sleep};

    use crate::{
        CommitDigest, CommitIndex,
        authority_service::COMMIT_LAG_MULTIPLIER,
        block::{BlockDigest, BlockRef, Round, SignedBlock, TestBlock, VerifiedBlock},
        block_verifier::{BlockVerifier, NoopBlockVerifier},
        commit::{CertifiedCommits, CommitRange, CommitVote, TrustedCommit},
        commit_vote_monitor::CommitVoteMonitor,
        context::Context,
        core_thread::{CoreError, CoreThreadDispatcher, tests::MockCoreThreadDispatcher},
        dag_state::DagState,
        error::{ConsensusError, ConsensusResult},
        network::{BlockStream, NetworkClient},
        round_prober::QuorumRound,
        storage::mem_store::MemStore,
        synchronizer::{
            FETCH_BLOCKS_CONCURRENCY, FETCH_REQUEST_TIMEOUT, InflightBlocksMap, SyncMethod,
            Synchronizer, VERIFIED_BLOCKS_CACHE_CAP,
        },
    };

    type FetchRequestKey = (Vec<BlockRef>, AuthorityIndex);
    type FetchRequestResponse = (Vec<VerifiedBlock>, Option<Duration>);
    type FetchLatestBlockKey = (AuthorityIndex, Vec<AuthorityIndex>);
    type FetchLatestBlockResponse = (Vec<VerifiedBlock>, Option<Duration>);

    // Mock verifier that always fails verification
    struct FailingBlockVerifier;

    impl BlockVerifier for FailingBlockVerifier {
        fn verify(&self, _block: &SignedBlock) -> ConsensusResult<()> {
            Err(ConsensusError::WrongEpoch {
                expected: 1,
                actual: 0,
            })
        }

        fn check_ancestors(
            &self,
            _block: &VerifiedBlock,
            _ancestors: &[Option<VerifiedBlock>],
            _gc_enabled: bool,
            _gc_round: Round,
        ) -> ConsensusResult<()> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockNetworkClient {
        fetch_blocks_requests: Mutex<BTreeMap<FetchRequestKey, FetchRequestResponse>>,
        fetch_latest_blocks_requests:
            Mutex<BTreeMap<FetchLatestBlockKey, Vec<FetchLatestBlockResponse>>>,
    }

    impl MockNetworkClient {
        async fn stub_fetch_blocks(
            &self,
            blocks: Vec<VerifiedBlock>,
            peer: AuthorityIndex,
            latency: Option<Duration>,
        ) {
            let mut lock = self.fetch_blocks_requests.lock().await;
            let block_refs = blocks
                .iter()
                .map(|block| block.reference())
                .collect::<Vec<_>>();
            lock.insert((block_refs, peer), (blocks, latency));
        }

        async fn stub_fetch_latest_blocks(
            &self,
            blocks: Vec<VerifiedBlock>,
            peer: AuthorityIndex,
            authorities: Vec<AuthorityIndex>,
            latency: Option<Duration>,
        ) {
            let mut lock = self.fetch_latest_blocks_requests.lock().await;
            lock.entry((peer, authorities))
                .or_default()
                .push((blocks, latency));
        }

        async fn fetch_latest_blocks_pending_calls(&self) -> usize {
            let lock = self.fetch_latest_blocks_requests.lock().await;
            lock.len()
        }
    }

    #[async_trait]
    impl NetworkClient for MockNetworkClient {
        const SUPPORT_STREAMING: bool = false;

        async fn send_block(
            &self,
            _peer: AuthorityIndex,
            _serialized_block: &VerifiedBlock,
            _timeout: Duration,
        ) -> ConsensusResult<()> {
            unimplemented!("Unimplemented")
        }

        async fn subscribe_blocks(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockStream> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_blocks(
            &self,
            peer: AuthorityIndex,
            block_refs: Vec<BlockRef>,
            _highest_accepted_rounds: Vec<Round>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            let mut lock = self.fetch_blocks_requests.lock().await;
            let response = lock
                .remove(&(block_refs, peer))
                .expect("Unexpected fetch blocks request made");

            let serialised = response
                .0
                .into_iter()
                .map(|block| block.serialized().clone())
                .collect::<Vec<_>>();

            drop(lock);

            if let Some(latency) = response.1 {
                sleep(latency).await;
            }

            Ok(serialised)
        }

        async fn fetch_commits(
            &self,
            _peer: AuthorityIndex,
            _commit_range: CommitRange,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Bytes>, Vec<Bytes>)> {
            unimplemented!("Unimplemented")
        }

        async fn fetch_latest_blocks(
            &self,
            peer: AuthorityIndex,
            authorities: Vec<AuthorityIndex>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            let mut lock = self.fetch_latest_blocks_requests.lock().await;
            let mut responses = lock
                .remove(&(peer, authorities.clone()))
                .expect("Unexpected fetch blocks request made");

            let response = responses.remove(0);
            let serialised = response
                .0
                .into_iter()
                .map(|block| block.serialized().clone())
                .collect::<Vec<_>>();

            if !responses.is_empty() {
                lock.insert((peer, authorities), responses);
            }

            drop(lock);

            if let Some(latency) = response.1 {
                sleep(latency).await;
            }

            Ok(serialised)
        }

        async fn get_latest_rounds(
            &self,
            _peer: AuthorityIndex,
            _timeout: Duration,
        ) -> ConsensusResult<(Vec<Round>, Vec<Round>)> {
            unimplemented!("Unimplemented")
        }
    }

    #[test]
    fn test_inflight_blocks_map() {
        // GIVEN
        let map = InflightBlocksMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
            BlockRef::new(12, AuthorityIndex::new_for_test(3), BlockDigest::MIN),
            BlockRef::new(15, AuthorityIndex::new_for_test(2), BlockDigest::MIN),
        ];
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // Lock & unlock blocks
        {
            let mut all_guards = Vec::new();

            // Try to acquire the block locks for authorities 1 & 2 (Periodic limit is 2)
            for i in 1..=2 {
                let authority = AuthorityIndex::new_for_test(i);

                let guard =
                    map.lock_blocks(missing_block_refs.clone(), authority, SyncMethod::Periodic);
                let guard = guard.expect("Guard should be created");
                assert_eq!(guard.block_refs.len(), 4);

                all_guards.push(guard);

                // trying to acquire any of them again will not succeed
                let guard =
                    map.lock_blocks(missing_block_refs.clone(), authority, SyncMethod::Periodic);
                assert!(guard.is_none());
            }

            // Trying to acquire for authority 3 it will fail - as we have maxed out the
            // number of allowed peers (Periodic limit is 2)
            let authority_3 = AuthorityIndex::new_for_test(3);

            let guard = map.lock_blocks(
                missing_block_refs.clone(),
                authority_3,
                SyncMethod::Periodic,
            );
            assert!(guard.is_none());

            // Explicitly drop the guard of authority 1 and try for authority 3 again - it
            // will now succeed
            drop(all_guards.remove(0));

            let guard = map.lock_blocks(
                missing_block_refs.clone(),
                authority_3,
                SyncMethod::Periodic,
            );
            let guard = guard.expect("Guard should be successfully acquired");

            assert_eq!(guard.block_refs, missing_block_refs);

            // Dropping all guards should unlock on the block refs
            drop(guard);
            drop(all_guards);

            assert_eq!(map.num_of_locked_blocks(), 0);
        }

        // Swap locks
        {
            // acquire a lock for authority 1
            let authority_1 = AuthorityIndex::new_for_test(1);
            let guard = map
                .lock_blocks(
                    missing_block_refs.clone(),
                    authority_1,
                    SyncMethod::Periodic,
                )
                .unwrap();

            // Now swap the locks for authority 2
            let authority_2 = AuthorityIndex::new_for_test(2);
            let guard = map.swap_locks(guard, authority_2);

            assert_eq!(guard.unwrap().block_refs, missing_block_refs);
        }
    }

    #[test]
    fn test_inflight_blocks_map_live_sync_limit() {
        // GIVEN
        let map = InflightBlocksMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
        ];
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // WHEN authority 1 locks with Live sync
        let authority_1 = AuthorityIndex::new_for_test(1);
        let guard_1 = map
            .lock_blocks(missing_block_refs.clone(), authority_1, SyncMethod::Live)
            .expect("Should successfully lock with Live sync");

        assert_eq!(guard_1.block_refs.len(), 2);

        // THEN authority 2 cannot lock with Live sync (limit of 1 reached)
        let authority_2 = AuthorityIndex::new_for_test(2);
        let guard_2 = map.lock_blocks(missing_block_refs.clone(), authority_2, SyncMethod::Live);

        assert!(
            guard_2.is_none(),
            "Should fail to lock - Live limit of 1 reached"
        );

        // WHEN authority 1 releases the lock
        drop(guard_1);

        // THEN authority 2 can now lock with Live sync
        let guard_2 = map
            .lock_blocks(missing_block_refs, authority_2, SyncMethod::Live)
            .expect("Should successfully lock after authority 1 released");

        assert_eq!(guard_2.block_refs.len(), 2);
    }

    #[test]
    fn test_inflight_blocks_map_periodic_allows_more_concurrency() {
        // GIVEN
        let map = InflightBlocksMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
        ];
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // WHEN authority 1 locks with Periodic sync
        let authority_1 = AuthorityIndex::new_for_test(1);
        let guard_1 = map
            .lock_blocks(
                missing_block_refs.clone(),
                authority_1,
                SyncMethod::Periodic,
            )
            .expect("Should successfully lock with Periodic sync");

        assert_eq!(guard_1.block_refs.len(), 2);

        // THEN authority 2 can also lock with Periodic sync (limit is 2)
        let authority_2 = AuthorityIndex::new_for_test(2);
        let guard_2 = map
            .lock_blocks(
                missing_block_refs.clone(),
                authority_2,
                SyncMethod::Periodic,
            )
            .expect("Should successfully lock - Periodic allows 2 authorities");

        assert_eq!(guard_2.block_refs.len(), 2);

        // BUT authority 3 cannot lock with Periodic sync (limit of 2 reached)
        let authority_3 = AuthorityIndex::new_for_test(3);
        let guard_3 = map.lock_blocks(
            missing_block_refs.clone(),
            authority_3,
            SyncMethod::Periodic,
        );

        assert!(
            guard_3.is_none(),
            "Should fail to lock - Periodic limit of 2 reached"
        );

        // WHEN authority 1 releases the lock
        drop(guard_1);

        // THEN authority 3 can now lock with Periodic sync
        let guard_3 = map
            .lock_blocks(missing_block_refs, authority_3, SyncMethod::Periodic)
            .expect("Should successfully lock after authority 1 released");

        assert_eq!(guard_3.block_refs.len(), 2);
    }

    #[test]
    fn test_inflight_blocks_map_periodic_blocks_live_when_at_live_limit() {
        // GIVEN
        let map = InflightBlocksMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
        ];
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // WHEN authority 1 locks with Periodic sync (total=1, at Live's limit)
        let authority_1 = AuthorityIndex::new_for_test(1);
        let guard_1 = map
            .lock_blocks(
                missing_block_refs.clone(),
                authority_1,
                SyncMethod::Periodic,
            )
            .expect("Should successfully lock with Periodic sync");

        assert_eq!(guard_1.block_refs.len(), 2);

        // THEN authority 2 cannot lock with Live sync (total already at Live limit of
        // 1)
        let authority_2 = AuthorityIndex::new_for_test(2);
        let guard_2_live =
            map.lock_blocks(missing_block_refs.clone(), authority_2, SyncMethod::Live);

        assert!(
            guard_2_live.is_none(),
            "Should fail to lock with Live - total already at Live limit of 1"
        );

        // BUT authority 2 CAN lock with Periodic sync (total would be 2, at Periodic
        // limit)
        let guard_2_periodic = map
            .lock_blocks(missing_block_refs, authority_2, SyncMethod::Periodic)
            .expect("Should successfully lock with Periodic - under Periodic limit of 2");

        assert_eq!(guard_2_periodic.block_refs.len(), 2);
    }

    #[test]
    fn test_inflight_blocks_map_live_then_periodic_interaction() {
        // GIVEN
        let map = InflightBlocksMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
        ];
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // WHEN authority 1 locks with Live sync (total=1, at Live limit)
        let authority_1 = AuthorityIndex::new_for_test(1);
        let guard_1 = map
            .lock_blocks(missing_block_refs.clone(), authority_1, SyncMethod::Live)
            .expect("Should successfully lock with Live sync");

        assert_eq!(guard_1.block_refs.len(), 2);

        // THEN authority 2 cannot lock with Live sync (would exceed Live limit of 1)
        let authority_2 = AuthorityIndex::new_for_test(2);
        let guard_2_live =
            map.lock_blocks(missing_block_refs.clone(), authority_2, SyncMethod::Live);

        assert!(
            guard_2_live.is_none(),
            "Should fail to lock with Live - would exceed Live limit of 1"
        );

        // BUT authority 2 CAN lock with Periodic sync (total=2, at Periodic limit)
        let guard_2 = map
            .lock_blocks(
                missing_block_refs.clone(),
                authority_2,
                SyncMethod::Periodic,
            )
            .expect("Should successfully lock with Periodic - total 2 is at Periodic limit");

        assert_eq!(guard_2.block_refs.len(), 2);

        // AND authority 3 cannot lock with Periodic sync (would exceed Periodic limit
        // of 2)
        let authority_3 = AuthorityIndex::new_for_test(3);
        let guard_3 = map.lock_blocks(missing_block_refs, authority_3, SyncMethod::Periodic);

        assert!(
            guard_3.is_none(),
            "Should fail to lock with Periodic - would exceed Periodic limit of 2"
        );
    }

    #[test]
    fn test_inflight_blocks_map_partial_locks_mixed_methods() {
        // GIVEN 4 blocks
        let map = InflightBlocksMap::new();
        let block_a = BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockDigest::MIN);
        let block_b = BlockRef::new(2, AuthorityIndex::new_for_test(0), BlockDigest::MIN);
        let block_c = BlockRef::new(3, AuthorityIndex::new_for_test(0), BlockDigest::MIN);
        let block_d = BlockRef::new(4, AuthorityIndex::new_for_test(0), BlockDigest::MIN);

        // Lock block A with authority 1 using Live (A at limit for Live)
        let guard_a = map
            .lock_blocks(
                [block_a].into(),
                AuthorityIndex::new_for_test(1),
                SyncMethod::Live,
            )
            .expect("Should lock block A");
        assert_eq!(guard_a.block_refs.len(), 1);

        // Lock block B with authorities 1 & 2 using Periodic (B at limit for Periodic)
        let guard_b1 = map
            .lock_blocks(
                [block_b].into(),
                AuthorityIndex::new_for_test(1),
                SyncMethod::Periodic,
            )
            .expect("Should lock block B");
        let guard_b2 = map
            .lock_blocks(
                [block_b].into(),
                AuthorityIndex::new_for_test(2),
                SyncMethod::Periodic,
            )
            .expect("Should lock block B again");
        assert_eq!(guard_b1.block_refs.len(), 1);
        assert_eq!(guard_b2.block_refs.len(), 1);

        // Lock block C with authority 1 using Periodic (C has 1 lock)
        let guard_c = map
            .lock_blocks(
                [block_c].into(),
                AuthorityIndex::new_for_test(1),
                SyncMethod::Periodic,
            )
            .expect("Should lock block C");
        assert_eq!(guard_c.block_refs.len(), 1);

        // Block D is unlocked

        // WHEN authority 3 requests all 4 blocks with Periodic
        let all_blocks = [block_a, block_b, block_c, block_d].into();
        let guard_3 = map
            .lock_blocks(
                all_blocks,
                AuthorityIndex::new_for_test(3),
                SyncMethod::Periodic,
            )
            .expect("Should get partial lock");

        // THEN should successfully lock C and D only
        // - A: total=1 (at Live limit), authority 3 can still add since using Periodic
        //   and total < 2
        // - B: total=2 (at Periodic limit), cannot lock
        // - C: total=1, can lock (under limit)
        // - D: total=0, can lock
        assert_eq!(
            guard_3.block_refs.len(),
            3,
            "Should lock blocks A, C, and D"
        );
        assert!(
            guard_3.block_refs.contains(&block_a),
            "Should contain block A"
        );
        assert!(
            !guard_3.block_refs.contains(&block_b),
            "Should NOT contain block B (at limit)"
        );
        assert!(
            guard_3.block_refs.contains(&block_c),
            "Should contain block C"
        );
        assert!(
            guard_3.block_refs.contains(&block_d),
            "Should contain block D"
        );
    }

    #[test]
    fn test_inflight_blocks_map_swap_locks_preserves_method() {
        // GIVEN
        let map = InflightBlocksMap::new();
        let some_block_refs = [
            BlockRef::new(1, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
            BlockRef::new(10, AuthorityIndex::new_for_test(0), BlockDigest::MIN),
        ];
        let missing_block_refs = some_block_refs.iter().cloned().collect::<BTreeSet<_>>();

        // WHEN authority 1 locks with Live sync
        let authority_1 = AuthorityIndex::new_for_test(1);
        let guard_1 = map
            .lock_blocks(missing_block_refs.clone(), authority_1, SyncMethod::Live)
            .expect("Should lock with Live sync");

        assert_eq!(guard_1.block_refs.len(), 2);

        // AND we swap to authority 2
        let authority_2 = AuthorityIndex::new_for_test(2);
        let guard_2 = map
            .swap_locks(guard_1, authority_2)
            .expect("Should swap locks");

        // THEN the new guard should preserve the block refs
        assert_eq!(guard_2.block_refs, missing_block_refs);

        // AND authority 3 cannot lock with Live sync (limit of 1 reached)
        let authority_3 = AuthorityIndex::new_for_test(3);
        let guard_3 = map.lock_blocks(missing_block_refs.clone(), authority_3, SyncMethod::Live);
        assert!(guard_3.is_none(), "Should fail - Live limit reached");

        // BUT authority 3 CAN lock with Periodic sync
        let guard_3_periodic = map
            .lock_blocks(missing_block_refs, authority_3, SyncMethod::Periodic)
            .expect("Should lock with Periodic");
        assert_eq!(guard_3_periodic.block_refs.len(), 2);
    }

    #[tokio::test]
    async fn test_process_fetched_blocks() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let (commands_sender, _commands_receiver) =
            monitored_mpsc::channel("consensus_synchronizer_commands", 1000);

        // Create input test blocks:
        // - Authority 0 block at round 60.
        // - Authority 1 blocks from round 30 to 93.
        let mut expected_blocks = vec![VerifiedBlock::new_for_test(TestBlock::new(60, 0).build())];
        expected_blocks.extend(
            (30..=60).map(|round| VerifiedBlock::new_for_test(TestBlock::new(round, 1).build())),
        );
        assert_eq!(
            expected_blocks.len(),
            context.parameters.max_blocks_per_sync
        );

        let expected_serialized_blocks = expected_blocks
            .iter()
            .map(|b| b.serialized().clone())
            .collect::<Vec<_>>();

        let expected_block_refs = expected_blocks
            .iter()
            .map(|b| b.reference())
            .collect::<BTreeSet<_>>();

        // GIVEN peer to fetch blocks from
        let peer_index = AuthorityIndex::new_for_test(2);

        // Create blocks_guard
        let inflight_blocks_map = InflightBlocksMap::new();
        let blocks_guard = inflight_blocks_map
            .lock_blocks(expected_block_refs.clone(), peer_index, SyncMethod::Live)
            .expect("Failed to lock blocks");

        assert_eq!(
            inflight_blocks_map.num_of_locked_blocks(),
            expected_block_refs.len()
        );

        // Create a Synchronizer
        let verified_cache = Arc::new(SyncMutex::new(LruCache::new(
            NonZeroUsize::new(VERIFIED_BLOCKS_CACHE_CAP).unwrap(),
        )));
        let result = Synchronizer::<
            MockNetworkClient,
            NoopBlockVerifier,
            MockCoreThreadDispatcher,
        >::process_fetched_blocks(
            expected_serialized_blocks,
            peer_index,
            blocks_guard, // The guard is consumed here
            core_dispatcher.clone(),
            block_verifier,
            verified_cache,
            commit_vote_monitor,
            context.clone(),
            commands_sender,
            "test",
        )
            .await;

        // THEN
        assert!(result.is_ok());

        // Check blocks were sent to core
        let added_blocks = core_dispatcher.get_add_blocks().await;
        assert_eq!(
            added_blocks
                .iter()
                .map(|b| b.reference())
                .collect::<BTreeSet<_>>(),
            expected_block_refs,
        );

        // Check blocks were unlocked
        assert_eq!(inflight_blocks_map.num_of_locked_blocks(), 0);
    }

    #[tokio::test]
    async fn test_process_fetched_blocks_duplicates() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let (commands_sender, _commands_receiver) =
            monitored_mpsc::channel("consensus_synchronizer_commands", 1000);

        // Create input test blocks:
        // - Authority 0 block at round 60.
        // - Authority 1 blocks from round 30 to 60.
        let mut expected_blocks = vec![VerifiedBlock::new_for_test(TestBlock::new(60, 0).build())];
        expected_blocks.extend(
            (30..=60).map(|round| VerifiedBlock::new_for_test(TestBlock::new(round, 1).build())),
        );
        assert_eq!(
            expected_blocks.len(),
            context.parameters.max_blocks_per_sync
        );

        let expected_serialized_blocks = expected_blocks
            .iter()
            .map(|b| b.serialized().clone())
            .collect::<Vec<_>>();

        let expected_block_refs = expected_blocks
            .iter()
            .map(|b| b.reference())
            .collect::<BTreeSet<_>>();

        // GIVEN peer to fetch blocks from
        let peer_index = AuthorityIndex::new_for_test(2);

        // Create blocks_guard
        let inflight_blocks_map = InflightBlocksMap::new();
        let blocks_guard = inflight_blocks_map
            .lock_blocks(expected_block_refs.clone(), peer_index, SyncMethod::Live)
            .expect("Failed to lock blocks");

        assert_eq!(
            inflight_blocks_map.num_of_locked_blocks(),
            expected_block_refs.len()
        );

        // Create a shared LruCache that will be reused to verify duplicate prevention
        let verified_cache = Arc::new(SyncMutex::new(LruCache::new(
            NonZeroUsize::new(VERIFIED_BLOCKS_CACHE_CAP).unwrap(),
        )));

        // WHEN process fetched blocks for the first time
        let result = Synchronizer::<
            MockNetworkClient,
            NoopBlockVerifier,
            MockCoreThreadDispatcher,
        >::process_fetched_blocks(
            expected_serialized_blocks.clone(),
            peer_index,
            blocks_guard,
            core_dispatcher.clone(),
            block_verifier.clone(),
            verified_cache.clone(),
            commit_vote_monitor.clone(),
            context.clone(),
            commands_sender.clone(),
            "test",
        )
        .await;

        // THEN
        assert!(result.is_ok());

        // Check blocks were sent to core
        let added_blocks = core_dispatcher.get_add_blocks().await;
        assert_eq!(
            added_blocks
                .iter()
                .map(|b| b.reference())
                .collect::<BTreeSet<_>>(),
            expected_block_refs,
        );

        // Check blocks were unlocked
        assert_eq!(inflight_blocks_map.num_of_locked_blocks(), 0);

        // PART 2: Verify LruCache prevents duplicate processing
        // Try to process the same blocks again (simulating duplicate fetch)
        let blocks_guard_second = inflight_blocks_map
            .lock_blocks(expected_block_refs.clone(), peer_index, SyncMethod::Live)
            .expect("Failed to lock blocks for second call");

        let result_second = Synchronizer::<
            MockNetworkClient,
            NoopBlockVerifier,
            MockCoreThreadDispatcher,
        >::process_fetched_blocks(
            expected_serialized_blocks,
            peer_index,
            blocks_guard_second,
            core_dispatcher.clone(),
            block_verifier,
            verified_cache.clone(),
            commit_vote_monitor,
            context.clone(),
            commands_sender,
            "test",
        )
        .await;

        assert!(result_second.is_ok());

        // Verify NO blocks were sent to core on the second call
        // because they were already in the LruCache
        let added_blocks_second_call = core_dispatcher.get_add_blocks().await;
        assert!(
            added_blocks_second_call.is_empty(),
            "Expected no blocks to be added on second call due to LruCache, but got {} blocks",
            added_blocks_second_call.len()
        );

        // Verify the cache contains all the block digests
        let cache_size = verified_cache.lock().len();
        assert_eq!(
            cache_size,
            expected_block_refs.len(),
            "Expected {} entries in the LruCache, but got {}",
            expected_block_refs.len(),
            cache_size
        );
    }

    #[tokio::test]
    async fn test_successful_fetch_blocks_from_peer() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let network_client = Arc::new(MockNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let handle = Synchronizer::start(
            network_client.clone(),
            context,
            core_dispatcher.clone(),
            commit_vote_monitor,
            block_verifier,
            dag_state,
            false,
        );

        // Create some test blocks
        let expected_blocks = (0..10)
            .map(|round| VerifiedBlock::new_for_test(TestBlock::new(round, 0).build()))
            .collect::<Vec<_>>();
        let missing_blocks = expected_blocks
            .iter()
            .map(|block| block.reference())
            .collect::<BTreeSet<_>>();

        // AND stub the fetch_blocks request from peer 1
        let peer = AuthorityIndex::new_for_test(1);
        network_client
            .stub_fetch_blocks(expected_blocks.clone(), peer, None)
            .await;

        // WHEN request missing blocks from peer 1
        assert!(handle.fetch_blocks(missing_blocks, peer).await.is_ok());

        // Wait a little bit until those have been added in core
        sleep(Duration::from_millis(1_000)).await;

        // THEN ensure those ended up in Core
        let added_blocks = core_dispatcher.get_add_blocks().await;
        assert_eq!(added_blocks, expected_blocks);
    }

    #[tokio::test]
    async fn saturate_fetch_blocks_from_peer() {
        // GIVEN
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let network_client = Arc::new(MockNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let handle = Synchronizer::start(
            network_client.clone(),
            context,
            core_dispatcher.clone(),
            commit_vote_monitor,
            block_verifier,
            dag_state,
            false,
        );

        // Create some test blocks
        let expected_blocks = (0..=2 * FETCH_BLOCKS_CONCURRENCY)
            .map(|round| VerifiedBlock::new_for_test(TestBlock::new(round as Round, 0).build()))
            .collect::<Vec<_>>();

        // Now start sending requests to fetch blocks by trying to saturate peer 1 task
        let peer = AuthorityIndex::new_for_test(1);
        let mut iter = expected_blocks.iter().peekable();
        while let Some(block) = iter.next() {
            // stub the fetch_blocks request from peer 1 and give some high response latency
            // so requests can start blocking the peer task.
            network_client
                .stub_fetch_blocks(
                    vec![block.clone()],
                    peer,
                    Some(Duration::from_millis(5_000)),
                )
                .await;

            let mut missing_blocks = BTreeSet::new();
            missing_blocks.insert(block.reference());

            // WHEN requesting to fetch the blocks, it should not succeed for the last
            // request and get an error with "saturated" synchronizer
            if iter.peek().is_none() {
                match handle.fetch_blocks(missing_blocks, peer).await {
                    Err(ConsensusError::SynchronizerSaturated(index, _)) => {
                        assert_eq!(index, peer);
                    }
                    _ => panic!("A saturated synchronizer error was expected"),
                }
            } else {
                assert!(handle.fetch_blocks(missing_blocks, peer).await.is_ok());
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
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        // Create some test blocks
        let expected_blocks = (0..10)
            .map(|round| VerifiedBlock::new_for_test(TestBlock::new(round, 0).build()))
            .collect::<Vec<_>>();
        let missing_blocks = expected_blocks
            .iter()
            .map(|block| block.reference())
            .collect::<BTreeSet<_>>();

        // AND stub the missing blocks
        core_dispatcher
            .stub_missing_blocks(missing_blocks.clone())
            .await;

        // AND stub the requests for authority 1 & 2
        // Make the first authority timeout, so the second will be called. "We" are
        // authority = 0, so we are skipped anyways.
        network_client
            .stub_fetch_blocks(
                expected_blocks.clone(),
                AuthorityIndex::new_for_test(1),
                Some(FETCH_REQUEST_TIMEOUT),
            )
            .await;
        network_client
            .stub_fetch_blocks(
                expected_blocks.clone(),
                AuthorityIndex::new_for_test(2),
                None,
            )
            .await;

        // WHEN start the synchronizer and wait for a couple of seconds
        let _handle = Synchronizer::start(
            network_client.clone(),
            context,
            core_dispatcher.clone(),
            commit_vote_monitor,
            block_verifier,
            dag_state,
            false,
        );

        sleep(8 * FETCH_REQUEST_TIMEOUT).await;

        // THEN the missing blocks should now be fetched and added to core
        let added_blocks = core_dispatcher.get_add_blocks().await;
        assert_eq!(added_blocks, expected_blocks);

        // AND missing blocks should have been consumed by the stub
        assert!(
            core_dispatcher
                .get_missing_blocks()
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn synchronizer_periodic_task_when_commit_lagging_gets_disabled() {
        // GIVEN
        let (mut context, _) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_batched_block_sync_for_testing(true);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::default());
        let network_client = Arc::new(MockNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));

        // AND stub some missing blocks. The highest accepted round is 0. Create blocks
        // that are above the sync threshold.
        let sync_missing_block_round_threshold = context.parameters.commit_sync_batch_size;
        let stub_blocks = (sync_missing_block_round_threshold * 2
            ..sync_missing_block_round_threshold * 2
                + context.parameters.max_blocks_per_sync as u32)
            .map(|round| VerifiedBlock::new_for_test(TestBlock::new(round, 0).build()))
            .collect::<Vec<_>>();
        let missing_blocks = stub_blocks
            .iter()
            .map(|block| block.reference())
            .collect::<BTreeSet<_>>();
        core_dispatcher
            .stub_missing_blocks(missing_blocks.clone())
            .await;
        // AND stub the requests for authority 1 & 2
        // Make the first authority timeout, so the second will be called. "We" are
        // authority = 0, so we are skipped anyways.
        let mut expected_blocks = stub_blocks
            .iter()
            .take(context.parameters.max_blocks_per_sync)
            .cloned()
            .collect::<Vec<_>>();
        network_client
            .stub_fetch_blocks(
                expected_blocks.clone(),
                AuthorityIndex::new_for_test(1),
                Some(FETCH_REQUEST_TIMEOUT),
            )
            .await;
        network_client
            .stub_fetch_blocks(
                expected_blocks.clone(),
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
                let block = TestBlock::new(round, authority)
                    .set_commit_votes(commit_votes)
                    .build();

                VerifiedBlock::new_for_test(block)
            })
            .collect::<Vec<_>>();

        // Pass them through the commit vote monitor - so now there will be a big commit
        // lag to prevent the scheduled synchronizer from running
        for block in blocks {
            commit_vote_monitor.observe_block(&block);
        }

        // Start the synchronizer and wait for a couple of seconds where normally
        // the synchronizer should have kicked in.
        let _handle = Synchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            block_verifier,
            dag_state.clone(),
            false,
        );

        sleep(4 * FETCH_REQUEST_TIMEOUT).await;

        // Since we should be in commit lag mode none of the missed blocks should have
        // been fetched - hence nothing should be sent to core for processing.
        let added_blocks = core_dispatcher.get_add_blocks().await;
        assert_eq!(added_blocks, vec![]);

        println!("Before advancing");
        // AND advance now the local commit index by adding a new commit that matches
        // the commit index of quorum
        {
            let mut d = dag_state.write();
            for index in 1..=commit_index {
                let commit =
                    TrustedCommit::new_for_test(index, CommitDigest::MIN, 0, BlockRef::MIN, vec![]);

                d.add_commit(commit);
            }

            println!("Once advanced");
            assert_eq!(
                d.last_commit_index(),
                commit_vote_monitor.quorum_commit_index()
            );
        }

        // Now stub again the missing blocks to fetch the exact same ones.
        core_dispatcher
            .stub_missing_blocks(missing_blocks.clone())
            .await;

        println!("Final sleep");
        sleep(2 * FETCH_REQUEST_TIMEOUT).await;

        // THEN the missing blocks should now be fetched and added to core
        let mut added_blocks = core_dispatcher.get_add_blocks().await;
        println!("Final await");
        added_blocks.sort_by_key(|block| block.reference());
        expected_blocks.sort_by_key(|block| block.reference());

        assert_eq!(added_blocks, expected_blocks);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn synchronizer_fetch_own_last_block() {
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
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let our_index = AuthorityIndex::new_for_test(0);

        // Create some test blocks
        let mut expected_blocks = (8..=10)
            .map(|round| VerifiedBlock::new_for_test(TestBlock::new(round, 0).build()))
            .collect::<Vec<_>>();

        // Now set different latest blocks for the peers
        // For peer 1 we give the block of round 10 (highest)
        let block_1 = expected_blocks.pop().unwrap();
        network_client
            .stub_fetch_latest_blocks(
                vec![block_1.clone()],
                AuthorityIndex::new_for_test(1),
                vec![our_index],
                Some(Duration::from_secs(10)),
            )
            .await;
        network_client
            .stub_fetch_latest_blocks(
                vec![block_1],
                AuthorityIndex::new_for_test(1),
                vec![our_index],
                None,
            )
            .await;

        // For peer 2 we give the block of round 9
        let block_2 = expected_blocks.pop().unwrap();
        network_client
            .stub_fetch_latest_blocks(
                vec![block_2.clone()],
                AuthorityIndex::new_for_test(2),
                vec![our_index],
                Some(Duration::from_secs(10)),
            )
            .await;
        network_client
            .stub_fetch_latest_blocks(
                vec![block_2],
                AuthorityIndex::new_for_test(2),
                vec![our_index],
                None,
            )
            .await;

        // For peer 3 we give a block with lowest round
        let block_3 = expected_blocks.pop().unwrap();
        network_client
            .stub_fetch_latest_blocks(
                vec![block_3.clone()],
                AuthorityIndex::new_for_test(3),
                vec![our_index],
                Some(Duration::from_secs(10)),
            )
            .await;
        network_client
            .stub_fetch_latest_blocks(
                vec![block_3],
                AuthorityIndex::new_for_test(3),
                vec![our_index],
                None,
            )
            .await;

        // WHEN start the synchronizer and wait for a couple of seconds
        let handle = Synchronizer::start(
            network_client.clone(),
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor,
            block_verifier,
            dag_state,
            true,
        );

        // Wait at least for the timeout time
        sleep(context.parameters.sync_last_known_own_block_timeout * 2).await;

        // Assert that core has been called to set the min propose round
        assert_eq!(
            core_dispatcher.get_last_own_proposed_round().await,
            vec![10]
        );

        // Ensure that all the requests have been called
        assert_eq!(network_client.fetch_latest_blocks_pending_calls().await, 0);

        // And we got one retry
        assert_eq!(
            context
                .metrics
                .node_metrics
                .sync_last_known_own_block_retries
                .get(),
            1
        );

        // Ensure that no panic occurred
        if let Err(err) = handle.stop().await {
            if err.is_panic() {
                std::panic::resume_unwind(err.into_panic());
            }
        }
    }
    #[derive(Default)]
    struct SyncMockDispatcher {
        missing_blocks: Mutex<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>,
        added_blocks: Mutex<Vec<VerifiedBlock>>,
    }

    #[async_trait::async_trait]
    impl CoreThreadDispatcher for SyncMockDispatcher {
        async fn get_missing_blocks(
            &self,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            Ok(self.missing_blocks.lock().await.clone())
        }
        async fn add_blocks(
            &self,
            blocks: Vec<VerifiedBlock>,
        ) -> Result<BTreeSet<BlockRef>, CoreError> {
            let mut guard = self.added_blocks.lock().await;
            guard.extend(blocks.clone());
            Ok(blocks.iter().map(|b| b.reference()).collect())
        }

        // Stub out the remaining CoreThreadDispatcher methods with defaults:

        async fn check_block_refs(
            &self,
            block_refs: Vec<BlockRef>,
        ) -> Result<BTreeSet<BlockRef>, CoreError> {
            // Echo back the requested refs by default
            Ok(block_refs.into_iter().collect())
        }

        async fn add_certified_commits(
            &self,
            _commits: CertifiedCommits,
        ) -> Result<BTreeSet<BlockRef>, CoreError> {
            // No additional certified-commit logic in tests
            Ok(BTreeSet::new())
        }

        async fn new_block(&self, _round: Round, _force: bool) -> Result<(), CoreError> {
            Ok(())
        }

        fn set_quorum_subscribers_exists(&self, _exists: bool) -> Result<(), CoreError> {
            Ok(())
        }

        fn set_propagation_delay_and_quorum_rounds(
            &self,
            _delay: Round,
            _received_quorum_rounds: Vec<QuorumRound>,
            _accepted_quorum_rounds: Vec<QuorumRound>,
        ) -> Result<(), CoreError> {
            Ok(())
        }

        fn set_last_known_proposed_round(&self, _round: Round) -> Result<(), CoreError> {
            Ok(())
        }

        fn highest_received_rounds(&self) -> Vec<Round> {
            Vec::new()
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn known_before_random_peer_fetch() {
        {
            // 1) Setup 10‐node context and in‐mem DAG
            let (ctx, _) = Context::new_for_test(10);
            let context = Arc::new(ctx);
            let store = Arc::new(MemStore::new());
            let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
            let inflight = InflightBlocksMap::new();

            // 2) One missing block
            let missing_vb = VerifiedBlock::new_for_test(TestBlock::new(100, 3).build());
            let missing_ref = missing_vb.reference();
            let missing_blocks = BTreeMap::from([(
                missing_ref,
                BTreeSet::from([
                    AuthorityIndex::new_for_test(2),
                    AuthorityIndex::new_for_test(3),
                    AuthorityIndex::new_for_test(4),
                ]),
            )]);

            // 3) Prepare mocks and stubs
            let network_client = Arc::new(MockNetworkClient::default());
            // Stub *all*  authorities so none panic:
            for i in 1..=9 {
                let peer = AuthorityIndex::new_for_test(i);
                if i == 1 || i == 4 {
                    network_client
                        .stub_fetch_blocks(
                            vec![missing_vb.clone()],
                            peer,
                            Some(2 * FETCH_REQUEST_TIMEOUT),
                        )
                        .await;
                    continue;
                }
                network_client
                    .stub_fetch_blocks(vec![missing_vb.clone()], peer, None)
                    .await;
            }

            // 4) Invoke knowledge-based fetch and random fallback selection
            //    deterministically
            let results = Synchronizer::<MockNetworkClient, NoopBlockVerifier, SyncMockDispatcher>
        ::fetch_blocks_from_authorities(
            context.clone(),
            inflight.clone(),
            network_client.clone(),
            missing_blocks,
            dag_state.clone(),
        )
            .await;

            // 5) Assert we got exactly two fetches - two from the first two who are aware
            //    of the missing block (authority 2 and 3)
            assert_eq!(results.len(), 2);

            // 6) The  knowledge-based‐fetch went to peer 2 and 3
            let (_hot_guard, hot_bytes, hot_peer) = &results[0];
            assert_eq!(*hot_peer, AuthorityIndex::new_for_test(2));
            let (_periodic_guard, _periodic_bytes, periodic_peer) = &results[1];
            assert_eq!(*periodic_peer, AuthorityIndex::new_for_test(3));
            // 7) Verify the returned bytes correspond to that block
            let expected = missing_vb.serialized().clone();
            assert_eq!(hot_bytes, &vec![expected]);
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
            block::{Round, TestBlock, VerifiedBlock},
            context::Context,
        };

        // 1) Setup a 10-node context, in-memory DAG, and inflight map
        let (ctx, _) = Context::new_for_test(10);
        let context = Arc::new(ctx);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let inflight = InflightBlocksMap::new();
        let network_client = Arc::new(MockNetworkClient::default());

        // 2) Create 1000 missing blocks known by authorities 0, 2, and 3
        let mut missing_blocks = BTreeMap::new();
        let mut missing_vbs = Vec::new();
        let known_number_blocks = 10;
        for i in 0..1000 {
            let vb = VerifiedBlock::new_for_test(TestBlock::new(1000 + i as Round, 0).build());
            let r = vb.reference();
            if i < known_number_blocks {
                // First 10 blocks are known by authorities 0, 2
                missing_blocks.insert(
                    r,
                    BTreeSet::from([
                        AuthorityIndex::new_for_test(0),
                        AuthorityIndex::new_for_test(2),
                    ]),
                );
            } else if i >= known_number_blocks && i < 2 * known_number_blocks {
                // Second 10 blocks are known by authorities 0, 3
                missing_blocks.insert(
                    r,
                    BTreeSet::from([
                        AuthorityIndex::new_for_test(0),
                        AuthorityIndex::new_for_test(3),
                    ]),
                );
            } else {
                // The rest are only known by authority 0
                missing_blocks.insert(r, BTreeSet::from([AuthorityIndex::new_for_test(0)]));
            }
            missing_vbs.push(vb);
        }

        // 3) Stub fetches for knowledge-based peers (2 and 3)
        let known_peers = [2, 3].map(AuthorityIndex::new_for_test);
        let known_vbs_by_peer: Vec<(AuthorityIndex, Vec<VerifiedBlock>)> = known_peers
            .iter()
            .map(|&peer| {
                let vbs = missing_vbs
                    .iter()
                    .filter(|vb| missing_blocks.get(&vb.reference()).unwrap().contains(&peer))
                    .take(context.parameters.max_blocks_per_sync)
                    .cloned()
                    .collect::<Vec<_>>();
                (peer, vbs)
            })
            .collect();

        for (peer, vbs) in known_vbs_by_peer {
            if peer == AuthorityIndex::new_for_test(2) {
                // Simulate timeout for peer 2, then fallback to peer 5
                network_client
                    .stub_fetch_blocks(vbs.clone(), peer, Some(2 * FETCH_REQUEST_TIMEOUT))
                    .await;
                network_client
                    .stub_fetch_blocks(vbs.clone(), AuthorityIndex::new_for_test(5), None)
                    .await;
            } else {
                network_client
                    .stub_fetch_blocks(vbs.clone(), peer, None)
                    .await;
            }
        }

        // 4) Stub fetches from periodic path peers (1 and 4)
        network_client
            .stub_fetch_blocks(
                missing_vbs[0..context.parameters.max_blocks_per_sync].to_vec(),
                AuthorityIndex::new_for_test(1),
                None,
            )
            .await;

        network_client
            .stub_fetch_blocks(
                missing_vbs[context.parameters.max_blocks_per_sync
                    ..2 * context.parameters.max_blocks_per_sync]
                    .to_vec(),
                AuthorityIndex::new_for_test(4),
                None,
            )
            .await;

        // 5) Execute the fetch logic
        let results = Synchronizer::<
            MockNetworkClient,
            NoopBlockVerifier,
            SyncMockDispatcher,
        >::fetch_blocks_from_authorities(
            context.clone(),
            inflight.clone(),
            network_client.clone(),
            missing_blocks,
            dag_state.clone(),
        )
            .await;

        // 6) Assert we got 4 fetches: peer 2 (timed out), fallback to 5, then periodic
        //    from 1 and 4
        assert_eq!(results.len(), 4, "Expected 2 known + 2 random fetches");

        // 7) First fetch from peer 3 (knowledge-based)
        let (_guard3, bytes3, peer3) = &results[0];
        assert_eq!(*peer3, AuthorityIndex::new_for_test(3));
        let expected2 = missing_vbs[known_number_blocks..2 * known_number_blocks]
            .iter()
            .map(|vb| vb.serialized().clone())
            .collect::<Vec<_>>();
        assert_eq!(bytes3, &expected2);

        // 8) Second fetch from peer 1 (periodic)
        let (_guard1, bytes1, peer1) = &results[1];
        assert_eq!(*peer1, AuthorityIndex::new_for_test(1));
        let expected1 = missing_vbs[0..context.parameters.max_blocks_per_sync]
            .iter()
            .map(|vb| vb.serialized().clone())
            .collect::<Vec<_>>();
        assert_eq!(bytes1, &expected1);

        // 9) Third fetch from peer 4 (periodic)
        let (_guard4, bytes4, peer4) = &results[2];
        assert_eq!(*peer4, AuthorityIndex::new_for_test(4));
        let expected4 = missing_vbs
            [context.parameters.max_blocks_per_sync..2 * context.parameters.max_blocks_per_sync]
            .iter()
            .map(|vb| vb.serialized().clone())
            .collect::<Vec<_>>();
        assert_eq!(bytes4, &expected4);

        // 10) Fourth fetch from peer 5 (fallback after peer 2 timeout)
        let (_guard5, bytes5, peer5) = &results[3];
        assert_eq!(*peer5, AuthorityIndex::new_for_test(5));
        let expected5 = missing_vbs[0..known_number_blocks]
            .iter()
            .map(|vb| vb.serialized().clone())
            .collect::<Vec<_>>();
        assert_eq!(bytes5, &expected5);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn test_verify_blocks_deduplication() {
        let (context, _keys) = Context::new_for_test(4);
        let context = Arc::new(context);
        let block_verifier = Arc::new(NoopBlockVerifier {});
        let failing_verifier = Arc::new(FailingBlockVerifier);
        let peer1 = AuthorityIndex::new_for_test(1);
        let peer2 = AuthorityIndex::new_for_test(2);

        // Create cache with capacity of 5 for eviction testing
        let cache = Arc::new(SyncMutex::new(LruCache::new(NonZeroUsize::new(5).unwrap())));

        // Test 1: Per-peer metric tracking
        let block1 = VerifiedBlock::new_for_test(TestBlock::new(10, 0).build());
        let serialized1 = vec![block1.serialized().clone()];

        // Verify from peer1 (cache miss)
        let result = Synchronizer::<MockNetworkClient, NoopBlockVerifier, MockCoreThreadDispatcher>::verify_blocks(
            serialized1.clone(), block_verifier.clone(), cache.clone(), &context, peer1, "live",
        );
        assert_eq!(result.unwrap().len(), 1);

        let peer1_hostname = &context.committee.authority(peer1).hostname;
        assert_eq!(
            context
                .metrics
                .node_metrics
                .synchronizer_skipped_blocks_by_peer
                .with_label_values(&[peer1_hostname.as_str(), "live"])
                .get(),
            0
        );

        // Verify same block from peer2 with different sync method (cache hit)
        let result = Synchronizer::<MockNetworkClient, NoopBlockVerifier, MockCoreThreadDispatcher>::verify_blocks(
            serialized1, block_verifier.clone(), cache.clone(), &context, peer2, "periodic",
        );
        assert_eq!(result.unwrap().len(), 0, "Should skip cached block");

        let peer2_hostname = &context.committee.authority(peer2).hostname;
        assert_eq!(
            context
                .metrics
                .node_metrics
                .synchronizer_skipped_blocks_by_peer
                .with_label_values(&[peer2_hostname.as_str(), "periodic"])
                .get(),
            1
        );

        // Test 2: Invalid blocks not cached
        let invalid_block = VerifiedBlock::new_for_test(TestBlock::new(20, 0).build());
        let invalid_serialized = vec![invalid_block.serialized().clone()];

        assert!(Synchronizer::<MockNetworkClient, FailingBlockVerifier, MockCoreThreadDispatcher>::verify_blocks(
            invalid_serialized.clone(), failing_verifier.clone(), cache.clone(), &context, peer1, "test",
        ).is_err());
        assert_eq!(cache.lock().len(), 1, "Invalid block should not be cached");

        // Verify invalid block fails again (not from cache)
        assert!(Synchronizer::<MockNetworkClient, FailingBlockVerifier, MockCoreThreadDispatcher>::verify_blocks(
            invalid_serialized, failing_verifier, cache.clone(), &context, peer1, "test",
        ).is_err());

        // Test 3: Cache eviction
        let blocks: Vec<_> = (0..5)
            .map(|i| VerifiedBlock::new_for_test(TestBlock::new(30 + i, 0).build()))
            .collect();

        // Fill cache to capacity
        for block in &blocks {
            Synchronizer::<MockNetworkClient, NoopBlockVerifier, MockCoreThreadDispatcher>::verify_blocks(
                vec![block.serialized().clone()], block_verifier.clone(), cache.clone(), &context, peer1, "test",
            ).unwrap();
        }
        assert_eq!(cache.lock().len(), 5);

        // Verify first block is evicted when adding new one
        let new_block = VerifiedBlock::new_for_test(TestBlock::new(99, 0).build());
        Synchronizer::<MockNetworkClient, NoopBlockVerifier, MockCoreThreadDispatcher>::verify_blocks(
            vec![new_block.serialized().clone()], block_verifier.clone(), cache.clone(), &context, peer1, "test",
        ).unwrap();

        // First block (block1) should be evicted, so re-verifying it should not be a
        // cache hit
        let block1_serialized = vec![block1.serialized().clone()];
        let result = Synchronizer::<MockNetworkClient, NoopBlockVerifier, MockCoreThreadDispatcher>::verify_blocks(
            block1_serialized, block_verifier.clone(), cache.clone(), &context, peer1, "test",
        );
        assert_eq!(
            result.unwrap().len(),
            1,
            "Evicted block should be re-verified"
        );

        // New block should still be in cache
        let new_block_serialized = vec![new_block.serialized().clone()];
        let result = Synchronizer::<MockNetworkClient, NoopBlockVerifier, MockCoreThreadDispatcher>::verify_blocks(
            new_block_serialized, block_verifier, cache, &context, peer1, "test",
        );
        assert_eq!(result.unwrap().len(), 0, "New block should be cached");
    }
}
