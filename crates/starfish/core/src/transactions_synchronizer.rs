// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
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
use rand::{prelude::SliceRandom, rngs::ThreadRng};
use serde::{Deserialize, Serialize};
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
    BlockHeaderAPI, CommitIndex, Round,
    block_header::{BlockRef, VerifiedTransactions},
    commit::CommitRange,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
    network::{BlockStream, NetworkClient},
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
        missing_block_refs: BTreeSet<BlockRef>,
        peer_index: AuthorityIndex,
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
        missing_block_refs: BTreeSet<BlockRef>,
        peer_index: AuthorityIndex,
    ) -> ConsensusResult<()> {
        let (sender, receiver) = oneshot::channel();
        self.commands_sender
            .send(Command::FetchTransactions {
                missing_block_refs,
                peer_index,
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
/// a node when data from only a few rounds is absent.
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
pub(crate) struct TransactionsSynchronizer<C: NetworkClient, D: CoreThreadDispatcher> {
    context: Arc<Context>,
    commands_receiver: Receiver<Command>,
    fetch_transaction_senders: BTreeMap<AuthorityIndex, Sender<TransactionsGuard>>,
    core_dispatcher: Arc<D>,
    dag_state: Arc<RwLock<DagState>>,
    fetch_transactions_scheduler_task: JoinSet<()>,
    network_client: Arc<C>,
    inflight_transactions_map: Arc<InflightTransactionsMap>,
    commands_sender: Sender<Command>,
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
                        Command::FetchTransactions{ missing_block_refs, peer_index, result } => {
                            if peer_index == self.context.own_index {
                                error!("We should never attempt to fetch transactions from our own node");
                                continue;
                            }

                            // Keep only the max allowed transactions to request. It is ok to reduce here as the scheduler
                            // task will take care syncing whatever is leftover.
                            let missing_block_refs = missing_block_refs
                                .into_iter()
                                .take(MAX_TRANSACTIONS_PER_FETCH)
                                .collect();

                            let transactions_guard = self.inflight_transactions_map.lock_transactions(missing_block_refs, peer_index);
                            let Some(transactions_guard) = transactions_guard else {
                                result.send(Ok(())).ok();
                                continue;
                            };

                            // We don't block if the corresponding peer task is saturated - but we rather drop the request. That's ok as the periodic
                            // synchronization task will handle any still missing transactions in next run.
                            let r = self
                                .fetch_transaction_senders
                                .get(&peer_index)
                                .expect("Fatal error, sender should be present")
                                .try_send(transactions_guard)
                                .map_err(|err| {
                                    match err {
                                        TrySendError::Full(_) => ConsensusError::TransactionsSynchronizerSaturated(peer_index),
                                        TrySendError::Closed(_) => ConsensusError::Shutdown
                                    }
                                });

                            result.send(r).ok();
                        }
                        Command::KickOffScheduler => {
                            // just reset the scheduler timeout timer to run immediately if not already running.
                            // If the scheduler is already running then just reduce the remaining time to run.
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
                                "live"
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
        sync_method: &str,
    ) -> ConsensusResult<()> {
        // Ensure that all the returned transactions do not go over the total max
        // allowed returned transactions
        if serialized_transactions.len() > requested_transactions_guard.block_refs.len() {
            return Err(ConsensusError::TooManyFetchedTransactionsReturned(
                peer_index,
            ));
        }

        // Deserialize the transactions
        let transactions = Handle::current()
            .spawn_blocking({
                move || {
                    serialized_transactions
                        .into_iter()
                        .map(|serialized| {
                            bcs::from_bytes::<VerifiedTransactions>(&serialized)
                                .map_err(ConsensusError::MalformedTransactions)
                        })
                        .collect::<ConsensusResult<Vec<_>>>()
                }
            })
            .await
            .expect("Spawn blocking should not fail")?;

        // Add the transactions to the core
        core_dispatcher.add_transactions(transactions).await?;

        // Kick off the scheduler to fetch any remaining missing transactions
        commands_sender
            .try_send(Command::KickOffScheduler)
            .map_err(|_| ConsensusError::Shutdown)?;

        // Report metrics
        // TODO: Add metrics for transactions synchronizer
        // context
        //     .metrics
        //     .node_metrics
        //     .transactions_synchronizer_fetched_transactions
        //     .with_label_values(&[sync_method])
        //     .inc_by(requested_transactions_guard.block_refs.len() as u64);

        Ok(())
    }

    async fn fetch_transactions_request(
        network_client: Arc<C>,
        peer: AuthorityIndex,
        transactions_guard: TransactionsGuard,
        request_timeout: Duration,
        retries: u32,
    ) -> (
        ConsensusResult<Vec<Bytes>>,
        TransactionsGuard,
        u32,
        AuthorityIndex,
    ) {
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
        .await
        .map_err(|_| ConsensusError::FetchTransactionsTimeout(peer))
        .and_then(|r| r);

        (result, transactions_guard, retries + 1, peer)
    }

    /// Starts a task to fetch missing transactions from other authorities.
    async fn start_fetch_missing_transactions_task(&mut self) -> ConsensusResult<()> {
        // Get missing transactions from the core
        let missing_transactions = self.core_dispatcher.get_missing_transaction_data().await?;

        if missing_transactions.is_empty() {
            return Ok(());
        }

        // Fetch the missing transactions from other authorities
        let fetch_transactions_from_authorities_async = Self::fetch_transactions_from_authorities(
            self.context.clone(),
            self.inflight_transactions_map.clone(),
            self.network_client.clone(),
            missing_transactions,
        );

        self.fetch_transactions_scheduler_task
            .spawn(monitored_future!(async move {
                let _scope = monitored_scope("TransactionsSynchronizer::fetch_transactions_from_authorities");
                let results = fetch_transactions_from_authorities_async.await;

                for (transactions_guard, serialized_transactions, peer_index) in results {
                    // Process the fetched transactions
                    if let Err(err) = Self::process_fetched_transactions(
                        serialized_transactions,
                        peer_index,
                        transactions_guard,
                        self.core_dispatcher.clone(),
                        self.context.clone(),
                        self.commands_sender.clone(),
                        "periodic",
                    )
                    .await
                    {
                        warn!("Error while processing fetched transactions from peer {peer_index}: {err}");
                    }
                }
            }));

        Ok(())
    }

    /// Fetches missing transactions from other authorities.
    async fn fetch_transactions_from_authorities(
        context: Arc<Context>,
        inflight_transactions_map: Arc<InflightTransactionsMap>,
        network_client: Arc<C>,
        missing_transactions: BTreeSet<BlockRef>,
    ) -> Vec<(TransactionsGuard, Vec<Bytes>, AuthorityIndex)> {
        let _scope =
            monitored_scope("TransactionsSynchronizer::fetch_transactions_from_authorities");

        if missing_transactions.is_empty() {
            return Vec::new();
        }

        // Report metrics
        // TODO: Add metrics for transactions synchronizer
        // context
        //     .metrics
        //     .node_metrics
        //     .transactions_synchronizer_missing_transactions
        //     .set(missing_transactions.len() as i64);

        // Get the list of authorities to fetch from
        let mut authorities: Vec<AuthorityIndex> = context
            .committee
            .authorities()
            .filter_map(|(index, _)| {
                if index != context.own_index {
                    Some(index)
                } else {
                    None
                }
            })
            .collect();

        // Shuffle the authorities to avoid always fetching from the same ones
        #[cfg(not(test))]
        {
            let mut rng = rand::thread_rng();
            authorities.shuffle(&mut rng);
        }

        // Try to lock the transactions for each authority
        let mut results = Vec::new();
        let mut futures = FuturesUnordered::new();

        for authority_index in authorities {
            // Try to lock the transactions for this authority
            let transactions_guard = inflight_transactions_map
                .lock_transactions(missing_transactions.clone(), authority_index);
            let Some(transactions_guard) = transactions_guard else {
                continue;
            };

            // Fetch the transactions from this authority
            let block_refs = transactions_guard
                .block_refs
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            futures.push(async move {
                let result = timeout(
                    FETCH_FROM_PEERS_TIMEOUT,
                    network_client.fetch_transactions(
                        authority_index,
                        block_refs,
                        FETCH_FROM_PEERS_TIMEOUT,
                    ),
                )
                .await
                .map_err(|_| ConsensusError::FetchTransactionsTimeout(authority_index))
                .and_then(|r| r);

                (result, transactions_guard, authority_index)
            });
        }

        // Wait for all the futures to complete
        while let Some((result, transactions_guard, authority_index)) = futures.next().await {
            match result {
                Ok(serialized_transactions) => {
                    results.push((transactions_guard, serialized_transactions, authority_index));
                }
                Err(err) => {
                    warn!("Error while fetching transactions from peer {authority_index}: {err}");
                    drop(transactions_guard);
                }
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc};

    use bytes::Bytes;
    use tokio::sync::Mutex;

    use super::*;
    use crate::{
        block_header::{BlockRef, Transaction, VerifiedTransactions},
        error::ConsensusError,
        network::NetworkClient,
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
            let mut transactions_map = self.transactions.lock().await;
            for transaction in transactions {
                let block_ref = transaction.block_ref();
                let serialized = bcs::to_bytes(&transaction).unwrap();
                transactions_map.insert((peer, block_ref), serialized.into());
            }
        }
    }

    #[async_trait]
    impl NetworkClient for MockNetworkClient {
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

        async fn subscribe_blocks(
            &self,
            _peer: AuthorityIndex,
            _last_received: Round,
            _timeout: Duration,
        ) -> ConsensusResult<BlockStream> {
            // Not needed for transactions synchronizer tests
            unimplemented!("subscribe_blocks not implemented in mock")
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
