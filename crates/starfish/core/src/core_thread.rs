// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use async_trait::async_trait;
use iota_metrics::{
    monitored_mpsc::{Receiver, Sender, WeakSender, channel},
    monitored_scope, spawn_logged_monitored_task,
};
use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use thiserror::Error;
use tokio::sync::{oneshot, watch};
use tracing::warn;

use crate::{
    BlockHeaderAPI as _, VerifiedBlockHeader,
    block_header::{BlockRef, Round, VerifiedBlock, VerifiedTransactions},
    commit::CertifiedCommits,
    context::Context,
    core::Core,
    core_thread::CoreError::Shutdown,
    dag_state::DagState,
    error::{ConsensusError, ConsensusResult},
};

const CORE_THREAD_COMMANDS_CHANNEL_SIZE: usize = 2000;

enum CoreThreadCommand {
    /// Add blocks to be processed and accepted
    AddBlocks(
        Vec<VerifiedBlock>,
        oneshot::Sender<(
            BTreeSet<BlockRef>,
            BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        )>,
    ),
    /// Add block headers to be processed and accepted
    AddBlockHeaders(
        Vec<VerifiedBlockHeader>,
        oneshot::Sender<(
            BTreeSet<BlockRef>,
            BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        )>,
    ),
    /// Add committed sub dag blocks for processing and acceptance.
    AddCertifiedCommits(
        CertifiedCommits,
        oneshot::Sender<(
            BTreeSet<BlockRef>,
            BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        )>,
    ),
    /// Called when the min round has passed or the leader timeout occurred and
    /// a block should be produced. When the command is called with `force =
    /// true`, then the block will be created for `round` skipping
    /// any checks (ex leader existence of previous round). More information can
    /// be found on the `Core` component.
    NewBlock(
        Round,
        oneshot::Sender<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>,
        bool,
    ),
    /// Request missing blocks that need to be synced together with authorities
    /// that have these blocks.
    GetMissingBlocks(oneshot::Sender<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>),
    /// Add transactions to be processed and accepted
    AddTransactions(Vec<VerifiedTransactions>, oneshot::Sender<()>),
    /// Get missing transaction data that need to be synced
    GetMissingTransactionData(oneshot::Sender<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>),
}

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Core thread shutdown: {0}")]
    Shutdown(String),
}

/// The interface to dispatch commands to CoreThread and Core.
/// Also this allows the easier mocking during unit tests.
#[async_trait]
pub trait CoreThreadDispatcher: Sync + Send + 'static {
    async fn add_blocks(
        &self,
        blocks: Vec<VerifiedBlock>,
    ) -> Result<
        (
            BTreeSet<BlockRef>,
            BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        ),
        CoreError,
    >;

    async fn add_block_headers(
        &self,
        blocks: Vec<VerifiedBlockHeader>,
    ) -> Result<
        (
            BTreeSet<BlockRef>,
            BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        ),
        CoreError,
    >;

    async fn add_transactions(
        &self,
        transactions: Vec<VerifiedTransactions>,
    ) -> Result<(), CoreError>;

    async fn get_missing_transaction_data(
        &self,
    ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError>;

    async fn add_certified_commits(
        &self,
        commits: CertifiedCommits,
    ) -> Result<
        (
            BTreeSet<BlockRef>,
            BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        ),
        CoreError,
    >;

    async fn new_block(
        &self,
        round: Round,
        force: bool,
    ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError>;

    async fn get_missing_blocks(
        &self,
    ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError>;

    /// Informs the core whether consumer of produced blocks exists.
    /// This is only used by core to decide if it should propose new blocks.
    /// It is not a guarantee that produced blocks will be accepted by peers.
    fn set_quorum_subscribers_exists(&self, exists: bool) -> Result<(), CoreError>;

    fn set_last_known_proposed_round(&self, round: Round) -> Result<(), CoreError>;

    /// Returns the highest round received for each authority by Core.
    fn highest_received_rounds(&self) -> Vec<Round>;
}

pub(crate) struct CoreThreadHandle {
    sender: Sender<CoreThreadCommand>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl CoreThreadHandle {
    pub async fn stop(self) {
        // drop the sender, that will force all the other weak senders to not able to
        // upgrade.
        drop(self.sender);
        self.join_handle.await.ok();
    }
}

struct CoreThread {
    core: Core,
    receiver: Receiver<CoreThreadCommand>,
    rx_quorum_subscribers_exists: watch::Receiver<bool>,
    rx_last_known_proposed_round: watch::Receiver<Round>,
    context: Arc<Context>,
}

impl CoreThread {
    #[cfg_attr(test,tracing::instrument(skip_all, name ="",fields(authority = %self.context.own_index)))]
    pub async fn run(mut self) -> ConsensusResult<()> {
        tracing::debug!("Started core thread");

        loop {
            tokio::select! {
                command = self.receiver.recv() => {
                    let Some(command) = command else {
                        break;
                    };
                    self.context.metrics.node_metrics.core_lock_dequeued.inc();
                    match command {
                        CoreThreadCommand::AddBlocks(blocks, sender) => {
                            let _scope = monitored_scope("CoreThread::loop::add_blocks");
                            let (missing_block_refs, missing_committed_txns) = self.core.add_blocks(blocks)?;
                            sender.send((missing_block_refs, missing_committed_txns)).ok();
                        }
                        CoreThreadCommand::AddBlockHeaders(block_headers, sender) => {
                            let _scope = monitored_scope("CoreThread::loop::add_block_headers");
                            let (missing_block_refs, missing_committed_txns) = self.core.add_block_headers(block_headers)?;
                            sender.send((missing_block_refs, missing_committed_txns)).ok();
                        }
                        CoreThreadCommand::AddCertifiedCommits(commits, sender) => {
                            let _scope = monitored_scope("CoreThread::loop::add_certified_commits");
                            let (missing_block_refs, missing_committed_txns) = self.core.add_certified_commits(commits)?;
                            sender.send((missing_block_refs, missing_committed_txns)).ok();
                        }
                        CoreThreadCommand::NewBlock(round, sender, force) => {
                            let _scope = monitored_scope("CoreThread::loop::new_block");
                            let (_new_block_opt, missing_committed_txns) = self.core.new_block(round, force)?;
                            sender.send(missing_committed_txns).ok();
                        }
                        CoreThreadCommand::GetMissingBlocks(sender) => {
                            let _scope = monitored_scope("CoreThread::loop::get_missing_blocks");
                            sender.send(self.core.get_missing_blocks()).ok();
                        }
                        CoreThreadCommand::AddTransactions(transactions, sender) => {
                            let _scope = monitored_scope("CoreThread::loop::add_transactions");
                            self.core.add_transactions(transactions)?;
                            sender.send(()).ok();
                        }
                        CoreThreadCommand::GetMissingTransactionData(sender) => {
                            let _scope = monitored_scope("CoreThread::loop::get_missing_transaction_data");
                            sender.send(self.core.get_missing_transaction_data()).ok();
                        }
                    }
                }
                _ = self.rx_last_known_proposed_round.changed() => {
                    let _scope = monitored_scope("CoreThread::loop::set_last_known_proposed_round");
                    let round = *self.rx_last_known_proposed_round.borrow();
                    self.core.set_last_known_proposed_round(round);
                    self.core.new_block(round + 1, true)?;
                }
                _ = self.rx_quorum_subscribers_exists.changed() => {
                    let _scope = monitored_scope("CoreThread::loop::set_quorum_subscribers_exists");
                    let should_propose_before = self.core.should_propose();
                    let exists = *self.rx_quorum_subscribers_exists.borrow();
                    self.core.set_quorum_subscribers_exists(exists);
                    if !should_propose_before && self.core.should_propose() {
                        // If core cannot propose before but can propose now, try to produce a new block to ensure liveness,
                        // because block proposal could have been skipped.
                        self.core.new_block(Round::MAX, true)?;
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct ChannelCoreThreadDispatcher {
    context: Arc<Context>,
    sender: WeakSender<CoreThreadCommand>,
    tx_quorum_subscribers_exists: Arc<watch::Sender<bool>>,
    tx_last_known_proposed_round: Arc<watch::Sender<Round>>,
    highest_received_rounds: Arc<Vec<AtomicU32>>,
}

impl ChannelCoreThreadDispatcher {
    /// Starts the core thread for the consensus authority and returns a
    /// dispatcher and handle for managing the core thread.
    pub(crate) fn start(
        context: Arc<Context>,
        dag_state: &RwLock<DagState>,
        core: Core,
    ) -> (Self, CoreThreadHandle) {
        // Initialize highest received rounds.
        let highest_received_rounds = {
            let dag_state = dag_state.read();
            let highest_received_rounds = context
                .committee
                .authorities()
                .map(|(index, _)| {
                    AtomicU32::new(dag_state.get_last_block_header_for_authority(index).round())
                })
                .collect();

            highest_received_rounds
        };
        let (sender, receiver) =
            channel("consensus_core_commands", CORE_THREAD_COMMANDS_CHANNEL_SIZE);
        let (tx_quorum_subscribers_exists, mut rx_quorum_subscribers_exists) =
            watch::channel(false);
        let (tx_last_known_proposed_round, mut rx_last_known_proposed_round) = watch::channel(0);
        rx_quorum_subscribers_exists.mark_unchanged();
        rx_last_known_proposed_round.mark_unchanged();
        let core_thread = CoreThread {
            core,
            receiver,
            rx_quorum_subscribers_exists,
            rx_last_known_proposed_round,
            context: context.clone(),
        };

        let join_handle = spawn_logged_monitored_task!(
            async move {
                if let Err(err) = core_thread.run().await {
                    if !matches!(err, ConsensusError::Shutdown) {
                        panic!("Fatal error occurred: {err}");
                    }
                }
            },
            "ConsensusCoreThread"
        );

        // Explicitly using downgraded sender in order to allow sharing the
        // CoreThreadDispatcher but able to shutdown the CoreThread by dropping
        // the original sender.
        let dispatcher = ChannelCoreThreadDispatcher {
            context,
            sender: sender.downgrade(),
            tx_quorum_subscribers_exists: Arc::new(tx_quorum_subscribers_exists),
            tx_last_known_proposed_round: Arc::new(tx_last_known_proposed_round),
            highest_received_rounds: Arc::new(highest_received_rounds),
        };
        let handle = CoreThreadHandle {
            join_handle,
            sender,
        };
        (dispatcher, handle)
    }

    async fn send(&self, command: CoreThreadCommand) {
        self.context.metrics.node_metrics.core_lock_enqueued.inc();
        if let Some(sender) = self.sender.upgrade() {
            if let Err(err) = sender.send(command).await {
                warn!(
                    "Couldn't send command to core thread, probably is shutting down: {}",
                    err
                );
            }
        }
    }
}

#[async_trait]
impl CoreThreadDispatcher for ChannelCoreThreadDispatcher {
    async fn add_blocks(
        &self,
        blocks: Vec<VerifiedBlock>,
    ) -> Result<
        (
            BTreeSet<BlockRef>,
            BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        ),
        CoreError,
    > {
        for block in &blocks {
            self.highest_received_rounds[block.author()].fetch_max(block.round(), Ordering::AcqRel);
        }
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::AddBlocks(blocks, sender))
            .await;
        Ok(receiver.await.map_err(|e| Shutdown(e.to_string()))?)
    }

    async fn add_block_headers(
        &self,
        block_headers: Vec<VerifiedBlockHeader>,
    ) -> Result<
        (
            BTreeSet<BlockRef>,
            BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        ),
        CoreError,
    > {
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::AddBlockHeaders(block_headers, sender))
            .await;
        Ok(receiver.await.map_err(|e| Shutdown(e.to_string()))?)
    }

    async fn add_transactions(
        &self,
        transactions: Vec<VerifiedTransactions>,
    ) -> Result<(), CoreError> {
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::AddTransactions(transactions, sender))
            .await;
        receiver.await.map_err(|e| Shutdown(e.to_string()))
    }

    async fn get_missing_transaction_data(
        &self,
    ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::GetMissingTransactionData(sender))
            .await;
        receiver.await.map_err(|e| Shutdown(e.to_string()))
    }

    async fn add_certified_commits(
        &self,
        commits: CertifiedCommits,
    ) -> Result<
        (
            BTreeSet<BlockRef>,
            BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
        ),
        CoreError,
    > {
        for commit in commits.commits() {
            for block in commit.blocks() {
                self.highest_received_rounds[block.author()]
                    .fetch_max(block.round(), Ordering::AcqRel);
            }
        }
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::AddCertifiedCommits(commits, sender))
            .await;
        Ok(receiver.await.map_err(|e| Shutdown(e.to_string()))?)
    }

    async fn new_block(
        &self,
        round: Round,
        force: bool,
    ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::NewBlock(round, sender, force))
            .await;
        receiver.await.map_err(|e| Shutdown(e.to_string()))
    }

    async fn get_missing_blocks(
        &self,
    ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
        let (sender, receiver) = oneshot::channel();
        self.send(CoreThreadCommand::GetMissingBlocks(sender)).await;
        receiver.await.map_err(|e| Shutdown(e.to_string()))
    }

    fn set_quorum_subscribers_exists(&self, exists: bool) -> Result<(), CoreError> {
        self.tx_quorum_subscribers_exists
            .send(exists)
            .map_err(|e| Shutdown(e.to_string()))
    }

    fn set_last_known_proposed_round(&self, round: Round) -> Result<(), CoreError> {
        self.tx_last_known_proposed_round
            .send(round)
            .map_err(|e| Shutdown(e.to_string()))
    }

    fn highest_received_rounds(&self) -> Vec<Round> {
        self.highest_received_rounds
            .iter()
            .map(|round| round.load(Ordering::Relaxed))
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use iota_metrics::monitored_mpsc::unbounded_channel;
    use parking_lot::{Mutex, RwLock};
    use tokio::time::Instant;

    use super::*;
    use crate::{
        CommitConsumer, VerifiedBlockHeader,
        block_manager::BlockManager,
        commit_observer::CommitObserver,
        context::Context,
        core::CoreSignals,
        dag_state::DagState,
        leader_schedule::LeaderSchedule,
        storage::mem_store::MemStore,
        transaction::{TransactionClient, TransactionConsumer},
    };

    // TODO: complete the Mock for thread dispatcher to be used from several tests
    #[derive(Default)]
    pub(crate) struct MockCoreThreadDispatcher {
        blocks: Mutex<Vec<VerifiedBlock>>,
        block_headers: Mutex<Vec<VerifiedBlockHeader>>,
        missing_blocks: parking_lot::Mutex<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>>,
        last_known_proposed_round: Mutex<Vec<Round>>,
        new_block_calls: Arc<Mutex<Vec<(Round, bool, Instant)>>>,
        quorum_subscribers_exists: Mutex<bool>,
    }

    impl MockCoreThreadDispatcher {
        pub(crate) async fn get_and_drain_blocks(&self) -> Vec<VerifiedBlock> {
            let mut blocks = self.blocks.lock();
            blocks.drain(0..).collect()
        }

        pub(crate) async fn get_and_drain_block_headers(&self) -> Vec<VerifiedBlockHeader> {
            let mut block_headers = self.block_headers.lock();
            block_headers.drain(0..).collect()
        }

        pub(crate) fn get_blocks(&self) -> Vec<VerifiedBlock> {
            self.blocks.lock().clone()
        }

        pub(crate) fn get_block_headers(&self) -> Vec<VerifiedBlockHeader> {
            self.block_headers.lock().clone()
        }

        pub(crate) async fn stub_missing_blocks(&self, block_refs: BTreeSet<BlockRef>) {
            let mut missing_blocks = self.missing_blocks.lock();
            for block_ref in &block_refs {
                missing_blocks.insert(*block_ref, BTreeSet::from([block_ref.author]));
            }
        }

        pub(crate) async fn get_last_own_proposed_round(&self) -> Vec<Round> {
            let last_known_proposed_round = self.last_known_proposed_round.lock();
            last_known_proposed_round.clone()
        }

        pub(crate) async fn get_new_block_calls(&self) -> Vec<(Round, bool, Instant)> {
            let mut binding = self.new_block_calls.lock();
            let all_calls = binding.drain(0..);
            all_calls.into_iter().collect()
        }
    }

    #[async_trait]
    impl CoreThreadDispatcher for MockCoreThreadDispatcher {
        async fn add_blocks(
            &self,
            blocks: Vec<VerifiedBlock>,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            let block_refs = blocks.iter().map(|b| b.reference()).collect();
            self.blocks.lock().extend(blocks);
            Ok((block_refs, BTreeMap::new()))
        }

        async fn add_block_headers(
            &self,
            block_headers: Vec<VerifiedBlockHeader>,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
            ),
            CoreError,
        > {
            let block_refs = block_headers.iter().map(|b| b.reference()).collect();
            self.block_headers.lock().extend(block_headers);
            Ok((block_refs, BTreeMap::new()))
        }

        async fn add_transactions(
            &self,
            _transactions: Vec<VerifiedTransactions>,
        ) -> Result<(), CoreError> {
            unimplemented!()
        }

        async fn get_missing_transaction_data(
            &self,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            Ok(BTreeMap::new())
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
            round: Round,
            force: bool,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            self.new_block_calls
                .lock()
                .push((round, force, Instant::now()));
            Ok(BTreeMap::new())
        }

        async fn get_missing_blocks(
            &self,
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            let mut missing_blocks = self.missing_blocks.lock();
            let result = missing_blocks.clone();
            missing_blocks.clear();
            Ok(result)
        }

        fn set_quorum_subscribers_exists(&self, exists: bool) -> Result<(), CoreError> {
            *self.quorum_subscribers_exists.lock() = exists;
            Ok(())
        }

        fn set_last_known_proposed_round(&self, round: Round) -> Result<(), CoreError> {
            self.last_known_proposed_round.lock().push(round);
            Ok(())
        }

        fn highest_received_rounds(&self) -> Vec<Round> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_core_thread() {
        telemetry_subscribers::init_for_testing();
        let (context, mut key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let block_manager = BlockManager::new(context.clone(), dag_state.clone());
        let (_transaction_client, tx_receiver) = TransactionClient::new(context.clone());
        let transaction_consumer = TransactionConsumer::new(tx_receiver, context.clone());
        let (signals, signal_receivers) = CoreSignals::new(context.clone());
        let _block_receiver = signal_receivers.block_broadcast_receiver();
        let (sender, _receiver) = unbounded_channel("consensus_output");
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let commit_observer = CommitObserver::new(
            context.clone(),
            CommitConsumer::new(sender.clone(), 0),
            dag_state.clone(),
            store,
            leader_schedule.clone(),
        );
        let leader_schedule = Arc::new(LeaderSchedule::from_store(
            context.clone(),
            dag_state.clone(),
        ));
        let core = Core::new(
            context.clone(),
            leader_schedule,
            transaction_consumer,
            block_manager,
            true,
            commit_observer,
            signals,
            key_pairs.remove(context.own_index.value()).1,
            dag_state.clone(),
            false,
        );

        let (core_dispatcher, handle) =
            ChannelCoreThreadDispatcher::start(context, &dag_state, core);

        // Now create some clones of the dispatcher
        let dispatcher_1 = core_dispatcher.clone();
        let dispatcher_2 = core_dispatcher.clone();

        // Try to send some commands
        assert!(dispatcher_1.add_blocks(vec![]).await.is_ok());
        assert!(dispatcher_2.add_blocks(vec![]).await.is_ok());

        assert!(dispatcher_1.add_block_headers(vec![]).await.is_ok());
        assert!(dispatcher_2.add_block_headers(vec![]).await.is_ok());

        // Now shutdown the dispatcher
        handle.stop().await;

        // Try to send some commands
        assert!(dispatcher_1.add_blocks(vec![]).await.is_err());
        assert!(dispatcher_2.add_blocks(vec![]).await.is_err());
        assert!(dispatcher_1.add_block_headers(vec![]).await.is_err());
        assert!(dispatcher_2.add_block_headers(vec![]).await.is_err());
    }
}
