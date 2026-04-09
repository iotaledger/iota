// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use arc_swap::ArcSwapOption;
use iota_metrics::monitored_mpsc::{UnboundedReceiver, unbounded_channel};
use iota_protocol_config::{ConsensusNetwork, ProtocolConfig};
use parking_lot::Mutex;
use prometheus::Registry;
use starfish_config::{AuthorityIndex, Committee, NetworkKeyPair, Parameters, ProtocolKeyPair};
use starfish_core::{
    BlockTimestampMs, Clock, CommitConsumer, CommitConsumerMonitor, CommitDigest, CommitIndex,
    CommittedSubDag, ConsensusAuthority, TransactionClient, network::tonic_network::to_socket_addr,
    transaction::NoopTransactionVerifier,
};
use tempfile::TempDir;
use tokio::sync::RwLock;
use tracing::{info, trace};

/// Restart mode for authority nodes during testing
#[derive(Clone, Copy, Debug)]
pub(crate) enum RestartMode {
    /// Erase both consensus DB and node tracking state (fresh start)
    CleanAll,
    /// Keep both consensus DB and node tracking state (crash recovery)
    PersistAll,
    /// Keep consensus DB but reset last_processed_commit to 0.
    /// Tests recovery when consensus state is intact but node tracking is lost.
    ResetLastProcessed,
    /// Erase all transactions from the DB, preserving commits and block
    /// headers. Tests transaction recovery via sync from peers.
    EraseAllTransactions,
}

#[derive(Clone)]
pub(crate) struct Config {
    pub authority_index: AuthorityIndex,
    pub db_dir: Arc<TempDir>,
    pub committee: Committee,
    pub keypairs: Vec<(NetworkKeyPair, ProtocolKeyPair)>,
    #[expect(dead_code)]
    pub network_type: ConsensusNetwork,
    pub boot_counter: u64,
    pub clock_drift: BlockTimestampMs,
    pub protocol_config: ProtocolConfig,
    /// Last processed commit index for persistent DB restarts
    pub last_processed_commit: CommitIndex,
}

pub(crate) struct AuthorityNode {
    inner: Mutex<Option<AuthorityNodeInner>>,
    config: Config,
    db_dir: Mutex<Arc<TempDir>>,
    boot_counter: AtomicU64,
    /// Tracks the last processed commit index for persistent DB restarts
    last_processed_commit: Arc<AtomicU32>,
    /// Stores commit digests for consistency verification across authorities
    commit_digests: Arc<RwLock<HashMap<CommitIndex, CommitDigest>>>,
    /// Stores committed transactions for verification of sequencing
    /// completeness
    committed_transactions: Arc<RwLock<HashSet<Vec<u8>>>>,
}

impl AuthorityNode {
    pub fn new(config: Config) -> Self {
        let initial_boot_counter = config.boot_counter;
        let db_dir = config.db_dir.clone();
        Self {
            inner: Default::default(),
            config,
            db_dir: Mutex::new(db_dir),
            boot_counter: AtomicU64::new(initial_boot_counter),
            last_processed_commit: Arc::new(AtomicU32::new(0)),
            commit_digests: Arc::new(RwLock::new(HashMap::new())),
            committed_transactions: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Start this Node
    pub async fn start(&self) -> Result<()> {
        let current_boot_counter = self.boot_counter.fetch_add(1, Ordering::SeqCst);
        let last_processed = self.last_processed_commit.load(Ordering::SeqCst);
        info!(
            index =% self.config.authority_index,
            boot_counter =% current_boot_counter,
            last_processed,
            "starting in-memory node"
        );
        let mut config = self.config.clone();
        config.boot_counter = current_boot_counter;
        config.db_dir = self.db_dir.lock().clone();
        config.last_processed_commit = last_processed;
        *self.inner.lock() = Some(AuthorityNodeInner::spawn(config).await);
        Ok(())
    }

    /// Restart the node with the specified mode
    pub async fn restart(&self, mode: RestartMode) -> Result<()> {
        match mode {
            RestartMode::CleanAll => {
                self.stop().await;
                // Erase consensus DB and all node tracking
                *self.db_dir.lock() = Arc::new(TempDir::new()?);
                self.last_processed_commit.store(0, Ordering::SeqCst);
                // Treat clean DB as a fresh node: reset boot counter to enable
                // sync_last_known_own_block, and clear tracking state (commit
                // digests and committed transactions).
                self.boot_counter.store(0, Ordering::SeqCst);
                self.commit_digests.write().await.clear();
                self.committed_transactions.write().await.clear();
            }
            RestartMode::ResetLastProcessed => {
                self.stop().await;
                // Keep consensus DB, reset node tracking state
                self.last_processed_commit.store(0, Ordering::SeqCst);
                self.commit_digests.write().await.clear();
                self.committed_transactions.write().await.clear();
                // Keep boot_counter incrementing (not a fresh node)
            }
            RestartMode::PersistAll => {
                self.stop().await;
                // Keep both consensus DB and node tracking (no changes)
            }
            RestartMode::EraseAllTransactions => {
                self.stop_and_clear_transactions().await;

                // Reset tracking state (transactions will be re-synced)
                self.last_processed_commit.store(0, Ordering::SeqCst);
                self.commit_digests.write().await.clear();
                self.committed_transactions.write().await.clear();
            }
        }
        self.start().await
    }

    /// Spawns a background task to consume committed subdags from consensus.
    ///
    /// This task:
    /// - Tracks commit digests for consistency verification across authorities
    /// - Records all committed transactions for verification
    /// - Updates the last processed commit index for persistent DB restarts
    /// - Notifies the commit consumer monitor of progress
    ///
    /// The spawned task runs until the commit receiver is closed (when the node
    /// stops), so the task handle is intentionally not stored.
    ///
    /// Must be called after `start()` to begin tracking commits.
    pub fn spawn_committed_subdag_consumer(&self) -> Result<()> {
        let inner = self.inner.lock();
        if let Some(inner) = inner.as_ref() {
            let mut commit_receiver = inner.take_commit_receiver();
            let commit_consumer_monitor = inner.commit_consumer_monitor();
            let commit_digests = self.commit_digests.clone();
            let last_processed = self.last_processed_commit.clone();
            let committed_transactions = self.committed_transactions.clone();
            let _handle = tokio::spawn(async move {
                while let Some(subdag) = commit_receiver.recv().await {
                    // Store commit digest for consistency verification
                    commit_digests
                        .write()
                        .await
                        .insert(subdag.commit_ref.index, subdag.commit_ref.digest);
                    // Track committed transactions
                    for verified_txns in &subdag.transactions {
                        for txn in verified_txns.transactions() {
                            committed_transactions
                                .write()
                                .await
                                .insert(txn.data().to_vec());
                        }
                    }
                    // Track last processed for persistent DB restarts
                    last_processed.store(subdag.commit_ref.index, Ordering::SeqCst);
                    commit_consumer_monitor.set_highest_handled_commit(subdag.commit_ref.index);
                }
            });
        }
        Ok(())
    }

    pub fn commit_consumer_monitor(&self) -> Arc<CommitConsumerMonitor> {
        let inner = self.inner.lock();
        if let Some(inner) = inner.as_ref() {
            inner.commit_consumer_monitor()
        } else {
            panic!("Node not initialised");
        }
    }

    pub fn transaction_client(&self) -> Arc<TransactionClient> {
        let inner = self.inner.lock();
        if let Some(inner) = inner.as_ref() {
            inner.transaction_client()
        } else {
            panic!("Node not initialised");
        }
    }

    /// Stop this Node
    pub async fn stop(&self) {
        info!(index =% self.config.authority_index, "stopping in-memory node");
        let inner = self.inner.lock().take();
        if let Some(mut inner) = inner {
            if let Some(consensus_authority) = inner.consensus_authority.take() {
                consensus_authority.stop().await;
            }

            if let Some(handle) = inner.handle.take() {
                tracing::info!("shutting down {}", handle.node_id);
                iota_simulator::runtime::Handle::try_current()
                    .map(|h| h.delete_node(handle.node_id));
            }
        }
        info!(index =% self.config.authority_index, "node stopped");
    }

    /// Stop this Node and clear all transactions from the consensus store.
    /// Only used by simtests.
    pub async fn stop_and_clear_transactions(&self) {
        info!(index =% self.config.authority_index, "stopping in-memory node");
        let inner = self.inner.lock().take();
        if let Some(mut inner) = inner {
            if let Some(consensus_authority) = inner.consensus_authority.take() {
                consensus_authority
                    .stop_and_clear_transactions()
                    .await
                    .expect("Failed to delete transactions");
            }

            if let Some(handle) = inner.handle.take() {
                tracing::info!("shutting down {}", handle.node_id);
                iota_simulator::runtime::Handle::try_current()
                    .map(|h| h.delete_node(handle.node_id));
            }
        }
        info!(index =% self.config.authority_index, "node stopped");
    }

    /// If this Node is currently running
    pub fn is_running(&self) -> bool {
        self.inner.lock().as_ref().map_or(false, |c| c.is_alive())
    }

    /// Get the commit digest for a specific commit index
    pub async fn get_commit_digest(&self, index: CommitIndex) -> Option<CommitDigest> {
        self.commit_digests.read().await.get(&index).copied()
    }

    /// Get all committed transactions
    pub async fn get_committed_transactions(&self) -> HashSet<Vec<u8>> {
        self.committed_transactions.read().await.clone()
    }
}

pub(crate) struct AuthorityNodeInner {
    handle: Option<NodeHandle>,
    cancel_sender: Option<tokio::sync::watch::Sender<bool>>,
    consensus_authority: Option<ConsensusAuthority>,
    commit_receiver: ArcSwapOption<UnboundedReceiver<CommittedSubDag>>,
    commit_consumer_monitor: Arc<CommitConsumerMonitor>,
}

#[derive(Debug)]
struct NodeHandle {
    node_id: iota_simulator::task::NodeId,
}

/// When dropped, stop and wait for the node running in this node to completely
/// shutdown.
impl Drop for AuthorityNodeInner {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            tracing::info!("shutting down {}", handle.node_id);
            iota_simulator::runtime::Handle::try_current().map(|h| h.delete_node(handle.node_id));
        }
    }
}

impl AuthorityNodeInner {
    /// Spawn a new Node.
    pub async fn spawn(config: Config) -> Self {
        let (startup_sender, mut startup_receiver) = tokio::sync::watch::channel(false);
        let (cancel_sender, cancel_receiver) = tokio::sync::watch::channel(false);

        let handle = iota_simulator::runtime::Handle::current();
        let builder = handle.create_node();

        let authority = config.committee.authority(config.authority_index);
        let socket_addr = to_socket_addr(&authority.address).unwrap();
        let ip = match socket_addr {
            SocketAddr::V4(v4) => IpAddr::V4(*v4.ip()),
            _ => panic!("unsupported protocol"),
        };
        let init_receiver_swap = Arc::new(ArcSwapOption::empty());
        let int_receiver_swap_clone = init_receiver_swap.clone();

        let node = builder
            .ip(ip)
            .name(format!("{}", config.authority_index))
            .init(move || {
                info!("Node restarted");
                let config = config.clone();
                let mut cancel_receiver = cancel_receiver.clone();
                let init_receiver_swap_clone = int_receiver_swap_clone.clone();
                let startup_sender_clone = startup_sender.clone();

                async move {
                    let (consensus_authority, commit_receiver, commit_consumer_monitor) =
                        super::node::make_authority(config).await;

                    startup_sender_clone.send(true).ok();
                    init_receiver_swap_clone.store(Some(Arc::new((
                        consensus_authority,
                        commit_receiver,
                        commit_consumer_monitor,
                    ))));

                    // run until canceled
                    loop {
                        if cancel_receiver.changed().await.is_err() || *cancel_receiver.borrow() {
                            break;
                        }
                    }
                    trace!("cancellation received; shutting down thread");
                }
            })
            .build();

        startup_receiver.changed().await.unwrap();

        let Some(init_tuple) = init_receiver_swap.swap(None) else {
            panic!("Components should be initialised by now");
        };

        let Ok((consensus_authority, commit_receiver, commit_consumer_monitor)) =
            Arc::try_unwrap(init_tuple)
        else {
            panic!("commit receiver still in use");
        };

        Self {
            handle: Some(NodeHandle { node_id: node.id() }),
            cancel_sender: Some(cancel_sender),
            consensus_authority: Some(consensus_authority),
            commit_receiver: ArcSwapOption::new(Some(Arc::new(commit_receiver))),
            commit_consumer_monitor,
        }
    }

    /// Check to see that the Node is still alive by checking if the receiving
    /// side of the `cancel_sender` has been dropped.
    pub fn is_alive(&self) -> bool {
        if let Some(cancel_sender) = &self.cancel_sender {
            // unless the node is deleted, it keeps a reference to its start up function,
            // which keeps 1 receiver alive. If the node is actually running,
            // the cloned receiver will also be alive, and receiver count will
            // be 2.
            cancel_sender.receiver_count() > 1
        } else {
            false
        }
    }

    pub fn take_commit_receiver(&self) -> UnboundedReceiver<CommittedSubDag> {
        if let Some(commit_receiver) = self.commit_receiver.swap(None) {
            let Ok(commit_receiver) = Arc::try_unwrap(commit_receiver) else {
                panic!("commit receiver still in use");
            };

            commit_receiver
        } else {
            panic!("commit receiver already taken");
        }
    }

    pub fn commit_consumer_monitor(&self) -> Arc<CommitConsumerMonitor> {
        self.commit_consumer_monitor.clone()
    }

    pub fn transaction_client(&self) -> Arc<TransactionClient> {
        self.consensus_authority
            .as_ref()
            .expect("consensus authority should be available")
            .transaction_client()
    }
}

pub(crate) async fn make_authority(
    config: Config,
) -> (
    ConsensusAuthority,
    UnboundedReceiver<CommittedSubDag>,
    Arc<CommitConsumerMonitor>,
) {
    let Config {
        authority_index,
        db_dir,
        committee,
        keypairs,
        network_type: _,
        boot_counter,
        clock_drift,
        protocol_config,
        last_processed_commit,
    } = config;

    let registry = Registry::new();

    // Cache less blocks to exercise commit sync.
    let parameters = Parameters {
        db_path: db_dir.path().to_path_buf(),
        dag_state_cached_rounds: 5,
        commit_sync_parallel_fetches: 2,
        commit_sync_batch_size: 3,
        sync_last_known_own_block_timeout: Duration::from_millis(2_000),
        enable_fast_commit_syncer: protocol_config.consensus_fast_commit_sync(),
        ..Default::default()
    };
    let txn_verifier = NoopTransactionVerifier {};

    let protocol_keypair = keypairs[authority_index].1.clone();
    let network_keypair = keypairs[authority_index].0.clone();

    let (commit_sender, commit_receiver) = unbounded_channel("consensus_output");

    let commit_consumer = CommitConsumer::new(commit_sender, last_processed_commit);
    let commit_consumer_monitor = commit_consumer.monitor();

    let authority = ConsensusAuthority::start(
        0,
        authority_index,
        committee,
        parameters,
        protocol_config,
        protocol_keypair,
        network_keypair,
        Arc::new(Clock::new_for_test(clock_drift)),
        Arc::new(txn_verifier),
        commit_consumer,
        registry,
        boot_counter,
    )
    .await;

    (authority, commit_receiver, commit_consumer_monitor)
}
