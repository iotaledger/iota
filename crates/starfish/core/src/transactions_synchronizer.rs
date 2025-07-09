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
#[cfg(not(test))]
use rand::prelude::SliceRandom;
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

    /// Drops the provided `transactions_guard` which will force to unlock the
    /// transactions, and lock now again the referenced block refs. The swap
    /// is best effort and there is no guarantee that the `peer` will be
    /// able to acquire the new locks.
    fn swap_locks(
        self: &Arc<Self>,
        transactions_guard: TransactionsGuard,
        peer: AuthorityIndex,
    ) -> Option<TransactionsGuard> {
        let block_refs = transactions_guard.block_refs.clone();

        // Explicitly drop the guard
        drop(transactions_guard);

        // Now create new guard
        self.lock_transactions(block_refs, peer)
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
            result?
        }
        Ok(())
    }
}

/// `TransactionsSynchronizer` oversees live transaction synchronization,
/// crucial for node progress. Live synchronization refers to the process of
/// retrieving missing transactions, particularly those essential for advancing
/// a node when data from only a few most recent rounds is absent.
/// `TransactionsSynchronizer` aims for swift catch-up employing two mechanisms:
///
/// 1. Explicitly requesting missing transactions from designated authorities
///    via the "transaction send" path. A locking mechanism allows concurrent
///    requests for missing transactions from up to two authorities
///    simultaneously, enhancing the chances of timely retrieval.
///
/// 2. Periodically requesting missing transactions via a scheduler. This
///    primarily serves to retrieve missing transactions that were not fetched
///    via the "transaction send" path. The scheduler operates on either a fixed
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
    fetch_transaction_senders: BTreeMap<AuthorityIndex, Sender<TransactionsGuard>>,
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

        // Spawn the tasks to fetch the transactions from the others
        let mut fetch_transaction_senders = BTreeMap::new();
        let mut tasks = JoinSet::new();
        for (index, _) in context.committee.authorities() {
            if index == context.own_index {
                continue;
            }
            let (sender, receiver) = channel(
                "consensus_transactions_synchronizer_fetches",
                FETCH_TRANSACTIONS_CONCURRENCY,
            );
            let fetch_transactions_from_authority_async = Self::fetch_transactions_from_authority(
                index,
                network_client.clone(),
                context.clone(),
                core_dispatcher.clone(),
                dag_state.clone(),
                receiver,
                commands_sender.clone(),
                block_verifier.clone(),
            );
            tasks.spawn(monitored_future!(fetch_transactions_from_authority_async));
            fetch_transaction_senders.insert(index, sender);
        }

        let commands_sender_clone = commands_sender.clone();

        // Spawn the task to listen to the requests & periodic runs
        tasks.spawn(monitored_future!(async move {
            let mut s = Self {
                context,
                commands_receiver,
                fetch_transaction_senders,
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
        let mut rng = StdRng::from_rng(OsRng).expect("OsRng should be available");

        loop {
            tokio::select! {
                Some(command) = self.commands_receiver.recv() => {
                    match command {
                        Command::FetchTransactions{ missing_block_refs, result } => {
                            // // Keep only the max allowed transactions to request. It is ok to reduce here as the scheduler
                            // // task will take care of syncing whatever is leftover.
                            // let missing_block_refs = missing_block_refs
                            //     .into_iter()
                            //     .take(MAX_TRANSACTIONS_PER_FETCH)
                            //     .collect::<BTreeMap<_, _>>();

                            // Reorganize by authority - map from authority to the blocks they acknowledged
                            let mut blocks_by_authority: BTreeMap<AuthorityIndex, BTreeSet<BlockRef>> = BTreeMap::new();
                            for (block_ref, authorities) in &missing_block_refs {
                                for authority in authorities {
                                    blocks_by_authority
                                        .entry(*authority)
                                        .or_default()
                                        .insert(*block_ref);
                                }
                            }

                            let mut success = false;
                            for (peer_index, authority_block_refs) in blocks_by_authority {
                                if peer_index == self.context.own_index {
                                    continue;
                                }
                                // Keep only the max allowed transactions to request. It is ok to
                                // reduce here as the scheduler
                                // task will take care of syncing whatever is leftover.
                                // We randomize the selected transactions to avoid asking different
                                // nodes for exactly the same transactions.
                                // Once sharding is implemented, we will need to always ask multiple
                                // authorities for their shards of the same transactions, so probably
                                // the randomization step will not be necessary.
                                let authority_block_refs = authority_block_refs
                                    .into_iter()
                                    .choose_multiple(&mut rng, MAX_TRANSACTIONS_PER_FETCH)
                                    .into_iter()
                                    .collect::<BTreeSet<_>>();


                                // Transaction locking guarantees that we are not fetching the same
                                // transactions from too many authorities.
                                let transactions_guard = self.inflight_transactions_map.lock_transactions(authority_block_refs, peer_index);
                                let Some(transactions_guard) = transactions_guard else {
                                    // TODO: if there are more missing transactions that we can
                                    //  fetch from the authority then we could use them instead of
                                    //  not fetching from the authority at all.
                                    continue;
                                };

                                match self
                                    .fetch_transaction_senders
                                    .get(&peer_index)
                                    .expect("Fatal error, sender should be present")
                                    .try_send(transactions_guard)
                                {
                                    Ok(_) => {
                                        success = true;
                                    },
                                    Err(TrySendError::Full(_)) => {
                                        warn!("Failed to send transactions to fetch from authority \
                                            {peer_index}, the channel is full. This can happen if the transactions \
                                            synchronizer is overloaded or the authority is not responding in time.");
                                        continue;
                                    },
                                    Err(TrySendError::Closed(_)) => {
                                        result.send(Err(ConsensusError::Shutdown)).ok();
                                        return;
                                    }
                                }
                            }

                                result.send(Ok(())).ok();
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

    async fn fetch_transactions_from_authority(
        peer_index: AuthorityIndex,
        network_client: Arc<C>,
        context: Arc<Context>,
        core_dispatcher: Arc<D>,
        dag_state: Arc<RwLock<DagState>>,
        mut receiver: Receiver<TransactionsGuard>,
        commands_sender: Sender<Command>,
        block_verifier: Arc<V>,
    ) {
        const MAX_RETRIES: u32 = 5;
        let peer_hostname = &context.committee.authority(peer_index).hostname;

        let mut requests = FuturesUnordered::new();

        loop {
            tokio::select! {
                Some(transactions_guard) = receiver.recv(), if requests.len() < FETCH_TRANSACTIONS_CONCURRENCY => {
                    requests.push(Self::fetch_transactions_request(network_client.clone(), peer_index, transactions_guard, FETCH_REQUEST_TIMEOUT, 1))
                },
                Some((response, transactions_guard, retries, _peer)) = requests.next() => {
                    match response {
                        Ok(transactions) => {
                            if let Err(err) = Self::process_fetched_transactions(transactions,
                                peer_index,
                                transactions_guard,
                                core_dispatcher.clone(),
                                context.clone(),
                                commands_sender.clone(),
                                block_verifier.clone(),
                                dag_state.clone(),
                                "live",
                            ).await {
                                warn!("Error while processing fetched transactions from peer {peer_index} {peer_hostname}: {err}");
                            }
                        },
                        Err(_) => {
                            if retries <= MAX_RETRIES {
                                requests.push(Self::fetch_transactions_request(network_client.clone(), peer_index, transactions_guard, FETCH_REQUEST_TIMEOUT, retries))
                            } else {
                                warn!("Max retries {retries} reached while trying to fetch transactions from peer {peer_index} {peer_hostname}.");
                                // we don't necessarily need to do, but dropping the guard here to unlock the transactions
                                drop(transactions_guard);
                            }
                        }
                    }
                },
                else => {
                    info!("Fetching transactions from authority {peer_index} task will now abort.");
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

        // Deserialize and verify the transactions
        let transactions = Handle::current()
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
                let context = context.clone();
                move || {
                    Self::verify_transactions(
                        serialized_transactions,
                        block_verifier,
                        &context,
                        peer_index,
                        block_headers_map,
                    )
                }
            })
            .await
            .expect("Spawn blocking should not fail")?;

        let metrics = &context.metrics.node_metrics;
        let peer_hostname = &context.committee.authority(peer_index).hostname;
        metrics
            .transaction_synchronizer_fetched_transactions_by_peer
            .with_label_values(&[peer_hostname.as_str(), sync_method])
            .inc_by(transactions.len() as u64);
        for transactions in &transactions {
            let block_hostname = &context
                .committee
                .authority(transactions.block_ref().author)
                .hostname;
            metrics
                .transaction_synchronizer_fetched_transactions_by_authority
                .with_label_values(&[block_hostname.as_str(), sync_method])
                .inc();
        }

        debug!(
            "Synced {} missing transactions from peer {peer_index} {peer_hostname}: {}",
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
        context: &Context,
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
                bcs::from_bytes(&serialized_transaction_bytes).map_err(|e| {
                    let hostname = context.committee.authority(peer_index).hostname.clone();
                    let err = ConsensusError::MalformedTransactions(e);
                    context
                        .metrics
                        .node_metrics
                        .invalid_transactions
                        .with_label_values(&[
                            hostname.as_str(),
                            "transaction_synchronizer",
                            err.name(),
                        ])
                        .inc();
                    err
                })?;

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
                let err = ConsensusError::TransactionCommitmentFailure {
                    round: serialized_transactions.block_ref.round,
                    author: serialized_transactions.block_ref.author,
                    peer: peer_index,
                };

                let hostname = context.committee.authority(peer_index).hostname.clone();
                context
                    .metrics
                    .node_metrics
                    .invalid_transactions
                    .with_label_values(&[hostname.as_str(), "transaction_synchronizer", err.name()])
                    .inc();
                return Err(err);
            }

            // Step 3: Deserialize and verify the actual transactions vector.
            let transactions: Vec<Transaction> =
                bcs::from_bytes(&serialized_transactions.serialized_transactions).map_err(|e| {
                    let err = ConsensusError::MalformedTransactions(e);
                    let hostname = context.committee.authority(peer_index).hostname.clone();
                    context
                        .metrics
                        .node_metrics
                        .invalid_transactions
                        .with_label_values(&[
                            hostname.as_str(),
                            "transaction_synchronizer",
                            err.name(),
                        ])
                        .inc();
                    err
                })?;

            if let Err(e) = block_verifier.check_and_verify_transactions(&transactions) {
                let hostname = context.committee.authority(peer_index).hostname.clone();
                context
                    .metrics
                    .node_metrics
                    .invalid_transactions
                    .with_label_values(&[hostname.as_str(), "transaction_synchronizer", e.name()])
                    .inc();
                return Err(e);
            }

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
        mut retries: u32,
    ) -> (
        ConsensusResult<Vec<Bytes>>,
        TransactionsGuard,
        u32,
        AuthorityIndex,
    ) {
        let start = Instant::now();
        let block_refs = transactions_guard
            .block_refs
            .iter()
            .cloned()
            .collect::<Vec<_>>();

        // Fetch the transactions from the peer
        let result = timeout(
            request_timeout,
            network_client.fetch_transactions(peer, block_refs, request_timeout),
        )
        .await;

        fail_point_async!("consensus-delay");

        let resp = match result {
            Ok(Err(err)) => {
                // Add a delay before retrying - if that is needed. If the request has timed
                // out, then eventually this will be a no-op.
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
        (resp, transactions_guard, retries, peer)
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
                context
                    .metrics
                    .node_metrics
                    .fetch_transactions_scheduler_inflight
                    .inc();
                let total_requested = missing_transactions.len();

                fail_point_async!("consensus-delay");
                // Fetch the missing transactions from other authorities
                let results = Self::fetch_transactions_from_authorities(
                    context.clone(),
                    inflight_transactions_map,
                    network_client,
                    missing_transactions,
                )
                .await;
                context
                    .metrics
                    .node_metrics
                    .fetch_transactions_scheduler_inflight
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
    ) -> Vec<(TransactionsGuard, Vec<Bytes>, AuthorityIndex)> {
        const MAX_PEERS: usize = 3;

        // Attempt to fetch only up to a max of blocks
        let missing_transactions = missing_transactions
            .into_iter()
            .take(MAX_PEERS * MAX_TRANSACTIONS_PER_FETCH)
            .collect::<Vec<_>>();

        let mut missing_transactions_per_authority = vec![0; context.committee.size()];
        for block in &missing_transactions {
            missing_transactions_per_authority[block.0.author] += 1;
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

        // TODO: only use authorities that have acknowledged the transactions
        // Get the list of authorities to fetch from
        #[cfg_attr(test, expect(unused_mut))]
        let mut authorities: Vec<AuthorityIndex> = context
            .committee
            .authorities()
            .filter_map(|(peer_index, _)| (peer_index != context.own_index).then_some(peer_index))
            .collect::<Vec<_>>();

        // In test, the order is not randomized
        #[cfg(not(test))]
        authorities.shuffle(&mut OsRng);

        let mut authorities = authorities.into_iter();
        let mut request_futures = FuturesUnordered::new();

        // Send the initial requests
        for transactions in missing_transactions.chunks(MAX_TRANSACTIONS_PER_FETCH) {
            let authority = authorities
                .next()
                .expect("Possible misconfiguration as a peer should be found");
            let peer_hostname = &context.committee.authority(authority).hostname;
            let block_refs = transactions
                .iter()
                .cloned()
                .map(|(block_ref, _acknowledging_authorities)| block_ref)
                .collect::<BTreeSet<BlockRef>>();
            // lock the blocks to be fetched. If no lock can be acquired for any of the
            // blocks then don't bother
            if let Some(transactions_guard) =
                inflight_transactions_map.lock_transactions(block_refs.clone(), authority)
            {
                info!(
                    "Periodic sync of {} missing committed transactions from authority {} {}: {}",
                    block_refs.len(),
                    authority,
                    peer_hostname,
                    block_refs
                        .iter()
                        .map(|b| b.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );

                request_futures.push(Self::fetch_transactions_request(
                    network_client.clone(),
                    authority,
                    transactions_guard,
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
                Some((response, transactions_guard, _retries, peer_index)) = request_futures.next() => {
                    let peer_hostname = &context.committee.authority(peer_index).hostname;
                    match response {
                        Ok(fetched_blocks) => {
                            info!("Fetched {} blocks from peer {}", fetched_blocks.len(), peer_hostname);
                            results.push((transactions_guard, fetched_blocks, peer_index));

                            // no more pending requests are left, just break the loop
                            if request_futures.is_empty() {
                                break;
                            }
                        },
                        Err(_) => {
                            // try again if there is any peer left
                            if let Some(next_peer) = authorities.next() {
                                // Do best effort to lock guards. If we can't lock then don't bother at this run.
                                if let Some(blocks_guard) = inflight_transactions_map.swap_locks(transactions_guard, next_peer) {
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
                                    request_futures.push(Self::fetch_transactions_request(
                                        network_client.clone(),
                                        next_peer,
                                        blocks_guard,
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
    use std::{collections::HashMap, sync::Arc};

    use async_trait::async_trait;
    use bytes::Bytes;
    use tokio::sync::Mutex;

    use super::*;
    use crate::{
        Round,
        block_header::{BlockRef, VerifiedTransactions},
        commit::CommitRange,
        network::{BlockBundleStream, BlockStream, NetworkClient},
    };

    struct MockNetworkClient {
        transactions: Arc<Mutex<HashMap<(AuthorityIndex, BlockRef), Bytes>>>,
    }

    impl MockNetworkClient {
        fn new() -> Self {
            Self {
                transactions: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        async fn stub_fetch_transactions(
            &self,
            transactions: Vec<VerifiedTransactions>,
            peer: AuthorityIndex,
        ) {
            // let mut transactions_map = self.transactions.lock().await;
            // // FIXME:
            // for transaction in transactions {
            //     let block_ref = transaction.block_ref();
            //     let serialized = bcs::to_bytes(&transaction).unwrap();
            //     transactions_map.insert((peer, block_ref),
            // serialized.into()); }
        }
    }

    #[async_trait]
    impl NetworkClient for MockNetworkClient {
        async fn subscribe_blocks(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockStream> {
            // Not needed for transactions synchronizer tests
            unimplemented!("subscribe_blocks not implemented in mock")
        }

        async fn subscribe_block_bundles(
            &self,
            peer: AuthorityIndex,
            last_received: Round,
            timeout: Duration,
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
            let transactions_map = self.transactions.lock().await;
            let mut result = Vec::new();
            for block_ref in block_refs {
                if let Some(serialized) = transactions_map.get(&(peer, block_ref)) {
                    result.push(serialized.clone());
                }
            }
            Ok(result)
        }

        async fn fetch_blocks(
            &self,
            _peer: AuthorityIndex,
            _block_refs: Vec<BlockRef>,
            _highest_accepted_rounds: Vec<Round>,
            _timeout: Duration,
        ) -> ConsensusResult<Vec<Bytes>> {
            // Not needed for transactions synchronizer tests
            unimplemented!("fetch_blocks not implemented in mock")
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
