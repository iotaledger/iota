// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    sync::Arc,
    time::Duration,
};

use parking_lot::RwLock;
use starfish_config::AuthorityIndex;
use tokio::{
    sync::{
        Mutex, mpsc,
        mpsc::{Receiver, Sender},
    },
    task::{JoinError, JoinHandle},
    time::{Instant, sleep_until},
};
use tracing::{debug, warn};

use crate::{
    Round, Transaction,
    block_header::{
        BlockHeaderDigest, GENESIS_ROUND, Shard, ShardWithProof, ShardWithProofAPI,
        TransactionsCommitment, VerifiedBlock, VerifiedTransactions,
    },
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::{DagState, DataSource},
    decoder::{ShardsDecoder, create_decoder},
    encoder::{ShardEncoder, create_encoder},
    error::{ConsensusError, ConsensusResult},
    transaction_ref::TransactionRef,
};

const EVICTION_TIMEOUT: Duration = Duration::from_secs(1);

const SEND_TO_CORE_RECONSTRUCTED_TXS_TIMEOUT: Duration = Duration::from_millis(20);
const NUMBER_OF_RECONSTRUCTION_WORKERS: usize = 5;

/// Using transaction messages we update the state of shard reconstructor
/// Two types of messages are supported: full transaction and shard
#[derive(Clone, Debug)]
pub(crate) enum TransactionMessage {
    FullTransaction(TransactionRef),
    Shard(ShardMessage),
}

/// Shard message contains shard with index and the reference to the
/// transactions the shard was erasure-coded from, plus an optional block digest
/// (present for V1 shards, absent for V2 shards that use TransactionRef).
#[derive(Clone, Debug)]
pub(crate) struct ShardMessage {
    transaction_ref: TransactionRef,
    block_digest: Option<BlockHeaderDigest>,
    shard: Shard,
    shard_index: usize,
}

impl TransactionMessage {
    pub fn transaction_ref(&self) -> TransactionRef {
        match self {
            TransactionMessage::FullTransaction(tx_ref) => *tx_ref,
            TransactionMessage::Shard(msg) => msg.transaction_ref,
        }
    }

    /// Create transaction messages (full, shards) for a given block
    /// bundle
    pub fn create_transaction_messages(
        block: &VerifiedBlock,
        shards: &[ShardWithProof],
        shard_index: usize,
    ) -> Vec<TransactionMessage> {
        let full = TransactionMessage::FullTransaction(block.transaction_ref());

        let shard_msgs = shards.iter().map(|swp| {
            TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef {
                    round: swp.round(),
                    author: swp.author(),
                    transactions_commitment: swp.transaction_commitment(),
                },
                block_digest: swp.block_digest(),
                shard: swp.shard().clone(),
                shard_index,
            })
        });

        std::iter::once(full).chain(shard_msgs).collect()
    }
}

/// A basic structure that represents the collection of shards for a given
/// transaction reference. We track the number of shards and the shards
/// themselves.
#[derive(Clone)]
pub struct ShardAccumulator {
    /// Reference to the transactions these shards were erasure-coded from
    transaction_ref: TransactionRef,
    /// Block digest of the source block (present for V1, absent for V2)
    block_digest: Option<BlockHeaderDigest>,
    /// Collected shards, indexed by their shard index
    collected_shards: Vec<Option<Shard>>,
    /// Number of collected data shards
    number_shards: usize,
}

impl ShardAccumulator {
    /// Create a new accumulator initialized with the first shard
    fn new_with_shard(msg: ShardMessage, total_length: usize) -> Self {
        let ShardMessage {
            transaction_ref,
            block_digest,
            shard,
            shard_index,
        } = msg;
        let mut collected_shards = vec![None; total_length];
        collected_shards[shard_index] = Some(shard);
        Self {
            transaction_ref,
            block_digest,
            collected_shards,
            number_shards: 1,
        }
    }

    /// Update the accumulator with a new shard
    fn update_with_shard(&mut self, msg: ShardMessage) {
        let ShardMessage {
            shard, shard_index, ..
        } = msg;
        if self.collected_shards[shard_index].is_none() {
            self.collected_shards[shard_index] = Some(shard);
            self.number_shards += 1;
        }
    }

    /// The condition to reconstruct the transaction data is by relying on the
    /// number of shards
    fn is_ready_to_reconstruct(&self, info_length: usize) -> bool {
        self.number_shards >= info_length
    }

    /// We use Codec to decode the transaction data from collected shards. Once
    /// reconstructed, we encode and verify that the transaction commitment
    /// was computed correctly
    fn decode_by_codec(&self, codec: &mut Codec) -> ConsensusResult<VerifiedTransactions> {
        let transactions = codec.decoder.decode_shards(
            codec.info_length,
            codec.parity_length,
            self.collected_shards.clone(),
        )?;

        let serialized =
            Transaction::serialize(&transactions).expect("We should expect serialization to work");

        // Verify the commitment
        let computed_commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized,
            &codec.context.clone(),
            &mut codec.encoder,
        )?;
        if computed_commitment != self.transaction_ref.transactions_commitment {
            return Err(ConsensusError::TransactionCommitmentMismatch {
                transaction_ref: self.transaction_ref,
            });
        }

        Ok(VerifiedTransactions::new(
            transactions,
            self.transaction_ref,
            self.block_digest,
            serialized,
        ))
    }
}

/// Data structure containing both encoder and decoder
pub struct Codec {
    pub encoder: Box<dyn ShardEncoder + Send + Sync>,
    pub decoder: Box<dyn ShardsDecoder + Send + Sync>,
    pub context: Arc<Context>,
    pub info_length: usize,
    pub parity_length: usize,
}

impl Codec {
    pub fn new(context: &Arc<Context>) -> Self {
        Self {
            encoder: create_encoder(context),
            decoder: create_decoder(context),
            context: context.clone(),
            info_length: context.committee.info_length(),
            parity_length: context.committee.parity_length(),
        }
    }
}

/// By keeping this handle, we continue running ShardCollector, responsible for
/// shard collection, and given number of shard reconstructor workers.
/// One field, transaction_message_sender, can be cloned to send transaction
/// messages to the internal ShardReconstructor
pub struct ShardReconstructorHandle {
    transaction_message_sender: Sender<Vec<TransactionMessage>>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
}

impl ShardReconstructorHandle {
    /// Access the transaction sender
    pub fn transaction_message_sender(&self) -> Sender<Vec<TransactionMessage>> {
        self.transaction_message_sender.clone()
    }

    /// Gracefully stop the shard reconstructor.
    pub async fn stop(&self) -> Result<(), JoinError> {
        let mut guard = self.join_handle.lock().await;

        if let Some(handle) = guard.take() {
            handle.abort();
            match handle.await {
                Ok(_) => Ok(()),
                Err(e) if e.is_cancelled() => Ok(()), // expected cancellation
                Err(e) => Err(e),                     // propagate panic or other errors
            }
        } else {
            Ok(()) // already stopped
        }
    }
}

impl<C: CoreThreadDispatcher + 'static> ShardReconstructor<C> {
    /// Start ShardReconstructor and get the respected handle
    pub fn start(
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
        core_dispatcher: Arc<C>,
    ) -> Arc<ShardReconstructorHandle> {
        let (mut reconstructor, transaction_message_sender) =
            ShardReconstructor::new(context, dag_state, core_dispatcher);

        let join_handle = tokio::spawn(async move {
            reconstructor.run().await;
        });

        Arc::new(ShardReconstructorHandle {
            transaction_message_sender,
            join_handle: Mutex::new(Some(join_handle)),
        })
    }
}

/// The main structure responsible for collecting shards and reconstructing
/// transaction data once enough shards are collected. Keeps track of already
/// locally available transaction data. The transaction is reconstructed only
/// when it is still not locally available and enough shards are reconstructed.
/// The structure periodically sends data to the core. In addition, eviction
/// mechanism is implemented by relying on the transaction GC round.
pub struct ShardReconstructor<C: CoreThreadDispatcher> {
    /// Shards below this round will not be collected
    transaction_gc_round: Round,
    /// Upon having this number of shards, the reconstruction is possible
    info_length: usize,
    /// The total number of shards
    total_length: usize,
    context: Arc<Context>,
    /// Already processed transaction either by authority service or by shard
    /// reconstructor
    processed_transactions: BTreeSet<TransactionRef>,
    /// A cache of reconstructed transactions that will be periodically sent in
    /// the core
    reconstructed_transactions: BTreeMap<TransactionRef, VerifiedTransactions>,
    /// A map of all shard accumulators. Periodically evicted. Keyed by
    /// TransactionRef which uniquely identifies transactions via
    /// transactions_commitment
    shard_accumulators: BTreeMap<TransactionRef, ShardAccumulator>,
    /// Use only read access to the dag state to read the transaction GC round
    /// and check whether the respected headers are available
    dag_state: Arc<RwLock<DagState>>,
    /// The receiver for transaction message sent from the authority service
    transaction_message_receiver: Receiver<Vec<TransactionMessage>>,
    /// After full reconstruction and verification, send data to the core
    core_dispatcher: Arc<C>,
    /// Queue is used to not reconstruct the same data twice
    reconstruction_queue: BTreeSet<TransactionRef>,
    /// Once enough shards are collected, they are sent to reconstructor workers
    ready_to_reconstruct_sender: Sender<ShardAccumulator>,
    /// Channel to receive accumulated shard for reconstruction by workers
    ready_to_reconstruct_receiver: Arc<Mutex<Receiver<ShardAccumulator>>>,
    /// Reconstruction workers send the verified data through this channel
    reconstructed_transactions_sender: Sender<VerifiedTransactions>,
    /// Reconstructed data is received by this channel
    reconstructed_transactions_receiver: Receiver<VerifiedTransactions>,
}

impl<C: CoreThreadDispatcher> ShardReconstructor<C> {
    /// Create a new ShardReconstructor and its associated Sender
    pub fn new(
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
        core_dispatcher: Arc<C>,
    ) -> (Self, Sender<Vec<TransactionMessage>>) {
        let info_length = context.committee.info_length();
        let total_length = context.committee.size();

        let (transaction_message_sender, transaction_message_receiver) = mpsc::channel(1000);
        let (ready_sender, ready_receiver) = mpsc::channel(1000);
        let (result_sender, result_receiver) = mpsc::channel(1000);

        let reconstructor = Self {
            info_length,
            total_length,
            context,
            core_dispatcher,
            dag_state,
            transaction_gc_round: GENESIS_ROUND,
            reconstruction_queue: BTreeSet::new(),
            ready_to_reconstruct_sender: ready_sender,
            ready_to_reconstruct_receiver: Arc::new(Mutex::new(ready_receiver)),
            reconstructed_transactions_sender: result_sender,
            reconstructed_transactions_receiver: result_receiver,
            processed_transactions: BTreeSet::new(),
            reconstructed_transactions: BTreeMap::new(),
            shard_accumulators: BTreeMap::new(),
            transaction_message_receiver,
        };

        (reconstructor, transaction_message_sender)
    }

    pub fn start_reconstruction_workers(&self) {
        for _ in 0..NUMBER_OF_RECONSTRUCTION_WORKERS {
            let mut codec = Codec::new(&self.context);
            let ready_rx = Arc::clone(&self.ready_to_reconstruct_receiver);
            let result_tx = self.reconstructed_transactions_sender.clone();
            let metrics = Arc::clone(&self.context.metrics);
            tokio::spawn(async move {
                loop {
                    // Receive a job from the ready to reconstruct channel
                    let job = {
                        let mut rx = ready_rx.lock().await;
                        rx.recv().await
                    };

                    match job {
                        Some(shard_accumulator) => {
                            metrics.node_metrics.reconstruction_jobs_started.inc();
                            match shard_accumulator.decode_by_codec(&mut codec) {
                                Ok(verified_transactions) => {
                                    debug!(
                                        "Successfully reconstructed transactions for {:?}",
                                        shard_accumulator.transaction_ref
                                    );
                                    if let Err(err) = result_tx.send(verified_transactions).await {
                                        warn!(
                                            "Failed to send the result to shard accumulator {err}"
                                        );
                                    }
                                }
                                Err(err) => {
                                    warn!(
                                        "Failed to reconstruct transactions for {:?}: {:?}",
                                        shard_accumulator.transaction_ref, err
                                    );
                                }
                            }
                            metrics.node_metrics.reconstruction_jobs_finished.inc();
                        }
                        None => {
                            debug!("Ready to reconstruct channel closed, workers exiting");
                            break;
                        }
                    }
                }
            });
        }
    }

    /// Run the main loop, consuming TransactionMessages from the channel
    async fn run(&mut self) {
        self.start_reconstruction_workers();

        let send_to_core_timeout =
            sleep_until(Instant::now() + SEND_TO_CORE_RECONSTRUCTED_TXS_TIMEOUT);
        tokio::pin!(send_to_core_timeout);

        let eviction_timeout = sleep_until(Instant::now() + EVICTION_TIMEOUT);
        tokio::pin!(eviction_timeout);

        loop {
            tokio::select! {
                    // Receive new shard/header/full-transaction
                    transaction_msgs = self.transaction_message_receiver.recv() => {
                        match transaction_msgs {
                            Some(msgs) => {
                                for msg in msgs {
                                    // Handle the message and update internal state
                                    if let Err(e) = self.handle_transaction_message(msg.clone()).await {
                                        warn!("Error when handling transaction message{:?}: {:?}", msg, e);
                                    }
                                }
                            }
                            None => {
                                debug!("Transaction channel is closed, shutting down");
                                break;
                            }
                        }
                    }
                    // A transaction is reconstructed in one of the reconstruction workers
                    Some(verified_transactions) = self.reconstructed_transactions_receiver.recv() => {
                        let tx_ref = verified_transactions.transaction_ref();
                        self.processed_transactions.insert(tx_ref);
                        self.reconstruction_queue.remove(&tx_ref);
                        self.reconstructed_transactions.insert(tx_ref, verified_transactions);
                    }

                 () = &mut send_to_core_timeout => {

                    // Grab reconstructed transactions and send them to core to add to the DAG state
                    if let Err(e) = self.send_to_core().await {
                        debug!("Error when sending reconstructed transactions to core: {:?}", e);
                    }

                    send_to_core_timeout
                        .as_mut()
                        .reset(Instant::now() + SEND_TO_CORE_RECONSTRUCTED_TXS_TIMEOUT);
                        }

                 () = &mut eviction_timeout => {

                    // Clean accumulators and processed transaction from memory
                    self.evict_memory();

                    eviction_timeout
                        .as_mut()
                        .reset(Instant::now() + EVICTION_TIMEOUT);
                }

            }
        }
    }

    /// Evict old accumulators and processed transactions to free memory. We
    /// read the dag state to find the transaction garbage collection round
    /// and evict all accumulators and processed transactions below that
    /// round.
    fn evict_memory(&mut self) {
        self.context
            .metrics
            .node_metrics
            .shard_accumulators
            .set(self.shard_accumulators.len() as i64);
        self.context
            .metrics
            .node_metrics
            .reconstruction_queue
            .set(self.reconstruction_queue.len() as i64);
        self.context
            .metrics
            .node_metrics
            .shard_reconstructor_processed_transactions
            .set(self.processed_transactions.len() as i64);

        let transaction_gc_round = self.dag_state.read().gc_round_for_last_solid_commit();

        // Update the internal transaction_gc_round
        self.transaction_gc_round = transaction_gc_round;

        let lower_bound = TransactionRef {
            round: transaction_gc_round,
            author: AuthorityIndex::ZERO,
            transactions_commitment: TransactionsCommitment::MIN,
        };

        self.processed_transactions = self.processed_transactions.split_off(&lower_bound);
        self.reconstructed_transactions = self.reconstructed_transactions.split_off(&lower_bound);
        self.shard_accumulators = self.shard_accumulators.split_off(&lower_bound);
    }

    async fn get_transactions_with_headers_in_dag_state(&mut self) -> Vec<VerifiedTransactions> {
        let transactions_map = std::mem::take(&mut self.reconstructed_transactions);
        // In most cases, all reconstructed transactions will go to the core
        let mut ready_to_be_sent_transactions = Vec::new();

        // We introduce a check about the existence of block headers to ensure that for
        // every transaction, we have the respected header in the dag state
        self.reconstructed_transactions = {
            #[cfg(not(test))]
            {
                let mut to_stay_transactions = BTreeMap::new();
                let block_headers_exist = {
                    let tx_refs: Vec<TransactionRef> = transactions_map.keys().copied().collect();
                    self.dag_state
                        .read()
                        .contains_verified_block_headers_for_transaction_refs(&tx_refs)
                };
                for (exists, (tx_ref, transactions)) in block_headers_exist
                    .into_iter()
                    .zip(transactions_map.into_iter())
                {
                    if exists {
                        ready_to_be_sent_transactions.push(transactions);
                    } else {
                        to_stay_transactions.insert(tx_ref, transactions);
                    }
                }
                to_stay_transactions
            }
            #[cfg(test)]
            {
                for transactions in transactions_map.values() {
                    ready_to_be_sent_transactions.push(transactions.clone());
                }
                BTreeMap::new()
            }
        };
        self.context
            .metrics
            .node_metrics
            .reconstructed_transactions_unknown
            .set(self.reconstructed_transactions.len() as i64);

        ready_to_be_sent_transactions
    }

    /// Send reconstructed transactions to the core
    async fn send_to_core(&mut self) -> ConsensusResult<()> {
        let transactions = self.get_transactions_with_headers_in_dag_state().await;
        if !transactions.is_empty() {
            let highest_accepted_round = self.dag_state.read().highest_accepted_round();
            for transaction in &transactions {
                let difference = highest_accepted_round.saturating_sub(transaction.round());
                self.context
                    .metrics
                    .node_metrics
                    .reconstruction_lag
                    .observe(difference as f64);
            }

            // Add the transactions to the core
            self.core_dispatcher
                .add_transactions(transactions, DataSource::ShardReconstructor)
                .await
                .map_err(|_| ConsensusError::Shutdown)?;
        }
        Ok(())
    }

    /// Handle a message and update internal state
    async fn handle_transaction_message(&mut self, msg: TransactionMessage) -> ConsensusResult<()> {
        let tx_ref = msg.transaction_ref();

        if self.processed_transactions.contains(&tx_ref)
            || self.reconstruction_queue.contains(&tx_ref)
            || tx_ref.round < self.transaction_gc_round
        {
            return Ok(());
        }

        let total_length = self.total_length;

        match msg {
            TransactionMessage::Shard(shard_msg) => match self.shard_accumulators.entry(tx_ref) {
                Entry::Vacant(v) => {
                    v.insert(ShardAccumulator::new_with_shard(shard_msg, total_length));
                }
                Entry::Occupied(mut o) => {
                    o.get_mut().update_with_shard(shard_msg);
                }
            },

            TransactionMessage::FullTransaction(tx_ref) => {
                self.processed_transactions.insert(tx_ref);
                return Ok(());
            }
        }

        // Check if we can reconstruct the block now and enqueue it if so
        Self::enqueue_if_ready(
            &mut self.shard_accumulators,
            &mut self.reconstruction_queue,
            &self.ready_to_reconstruct_sender,
            self.info_length,
            &tx_ref,
        )
        .await?;

        Ok(())
    }

    /// If the accumulator for the given key is ready to reconstruct, remove it
    /// from the map and enqueue it for reconstruction
    async fn enqueue_if_ready(
        accumulators: &mut BTreeMap<TransactionRef, ShardAccumulator>,
        reconstruction_queue: &mut BTreeSet<TransactionRef>,
        sender: &Sender<ShardAccumulator>,
        info_length: usize,
        tx_ref: &TransactionRef,
    ) -> ConsensusResult<()> {
        if let Some(acc) = accumulators.get(tx_ref) {
            if acc.is_ready_to_reconstruct(info_length) {
                // take ownership out of map
                let acc = accumulators
                    .remove(tx_ref)
                    .expect("We should expect the shard accumulator to be present");
                sender
                    .send(acc)
                    .await
                    .map_err(|_| ConsensusError::AccumulatorSenderClosed)?;
                reconstruction_queue.insert(*tx_ref);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
        time::Duration,
    };

    use parking_lot::RwLock;
    use rand::{seq::SliceRandom, thread_rng};
    use starfish_config::AuthorityIndex;
    use tokio::sync::{Mutex, mpsc::Sender};

    use crate::{
        BlockRef, Round, TestBlockHeader, Transaction, VerifiedBlockHeader,
        block_header::{
            Shard, ShardWithProof, TransactionsCommitment, VerifiedBlock, VerifiedOwnShard,
            VerifiedTransactions,
        },
        commit::CertifiedCommits,
        context::Context,
        core::ReasonToCreateBlock,
        core_thread::{CoreError, CoreThreadDispatcher},
        dag_state::{DagState, DataSource},
        encoder::create_encoder,
        shard_reconstructor::{
            ShardMessage, ShardReconstructor, ShardReconstructorHandle, TransactionMessage,
        },
        storage::mem_store::MemStore,
        transaction_ref::{GenericTransactionRef, TransactionRef},
    };

    struct TestHarness {
        context: Arc<Context>,
        core_dispatcher: Arc<MockCoreThreadDispatcher>,
        handle: Arc<ShardReconstructorHandle>,
        tx: Sender<Vec<TransactionMessage>>,
    }

    impl TestHarness {
        fn new(committee_size: usize) -> Self {
            let (context, _) = Context::new_for_test(committee_size);
            let context = Arc::new(context);
            let store = Arc::new(MemStore::new(context.clone()));
            let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
            let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
            let handle =
                ShardReconstructor::start(context.clone(), dag_state, core_dispatcher.clone());
            let tx = handle.transaction_message_sender();
            Self {
                context,
                core_dispatcher,
                handle,
                tx,
            }
        }
    }

    #[derive(Default)]
    struct MockCoreThreadDispatcher {
        transactions: Mutex<Vec<VerifiedTransactions>>,
    }

    impl MockCoreThreadDispatcher {
        fn new() -> Self {
            Self::default()
        }

        async fn get_and_drain_transactions(&self) -> Vec<VerifiedTransactions> {
            let mut guard = self.transactions.lock().await;
            guard.drain(..).collect()
        }
    }

    #[async_trait::async_trait]
    impl CoreThreadDispatcher for MockCoreThreadDispatcher {
        async fn add_transactions(
            &self,
            txs: Vec<VerifiedTransactions>,
            _source: DataSource,
        ) -> Result<(), CoreError> {
            let mut guard = self.transactions.lock().await;
            guard.extend(txs);
            Ok(())
        }
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
            _blocks: Vec<VerifiedBlockHeader>,
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

        async fn add_shards(&self, _shards: Vec<VerifiedOwnShard>) -> Result<(), CoreError> {
            unimplemented!()
        }

        async fn get_missing_transaction_data(
            &self,
        ) -> Result<BTreeMap<GenericTransactionRef, BTreeSet<AuthorityIndex>>, CoreError> {
            unimplemented!()
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

    ///  Prepare a batch of messages simulating the case:
    /// - FullTransaction for round `i` from authority `j`
    /// - The j-th shard of every authority's transaction data from round `i-1`
    ///   This simulates the typical case where authority is streaming its block
    ///   bundles
    fn prepare_bundle_messages(
        authority_j: u8,
        header_cur: VerifiedBlockHeader,
        headers_prev: &[VerifiedBlockHeader],
        shards_prev: &[Vec<Shard>], // one Vec<Shard> per authority
    ) -> Vec<TransactionMessage> {
        let mut msgs = Vec::new();

        // 1. FullTransaction for round i (authority j)
        msgs.push(TransactionMessage::FullTransaction(
            header_cur.transaction_ref(),
        ));

        // 2. The j-th shard of every authority’s transaction data from round i-1
        let j_index = authority_j as usize;
        for (auth_index, shards) in shards_prev.iter().enumerate() {
            if let Some(shard) = shards.get(j_index) {
                msgs.push(TransactionMessage::Shard(ShardMessage {
                    transaction_ref: headers_prev[auth_index].transaction_ref(),
                    block_digest: Some(headers_prev[auth_index].digest()),
                    shard: shard.clone(),
                    shard_index: j_index,
                }));
            }
        }

        msgs
    }

    /// Test that reconstruction only triggers after receiving one header and
    /// info_length shards
    #[tokio::test]
    async fn test_reconstruction_triggers_only_after_info_length_shards() {
        telemetry_subscribers::init_for_testing();

        // GIVEN
        let h = TestHarness::new(10);
        let context = &h.context;
        let transaction_message_sender = h.tx.clone();

        // Create block header & transactions
        let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build());
        let block_ref = header.reference();

        let txs = Transaction::random_transactions(4, 48);
        let serialized = Transaction::serialize(&txs).unwrap();

        let mut encoder = create_encoder(context);
        let commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized,
            context,
            &mut encoder,
        )
        .unwrap();

        let info_length = context.committee.info_length();
        let parity_length = context.committee.parity_length();

        let all_shards = encoder
            .encode_serialized_data(&serialized, info_length, parity_length)
            .unwrap();

        // Shuffle shard indices
        let mut rng = thread_rng();
        let mut indices: Vec<usize> = (0..all_shards.len()).collect();
        indices.shuffle(&mut rng);

        // Take info_length - 1 random shards first
        let first_subset = &indices[..info_length - 1];

        let mut batch = Vec::new();
        for &i in first_subset {
            batch.push(TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef::new(block_ref, commitment),
                block_digest: Some(block_ref.digest),
                shard: all_shards[i].clone(),
                shard_index: i,
            }));
        }

        transaction_message_sender.send(batch).await.unwrap();

        // Wait — should not reconstruct yet
        tokio::time::sleep(Duration::from_millis(400)).await;
        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        assert!(
            fetched.is_empty(),
            "With header + (info_length - 1) shards, no reconstruction should happen"
        );

        // Now send ONE more random shard (the missing one to make total info_length)
        let extra_shard_index = indices[info_length - 1];
        transaction_message_sender
            .send(vec![TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef::new(block_ref, commitment),
                block_digest: Some(block_ref.digest),
                shard: all_shards[extra_shard_index].clone(),
                shard_index: extra_shard_index,
            })])
            .await
            .unwrap();

        // THEN: reconstruction should happen
        tokio::time::sleep(Duration::from_millis(600)).await;
        let fetched = h.core_dispatcher.get_and_drain_transactions().await;

        assert_eq!(
            fetched.len(),
            1,
            "Reconstruction should happen after reaching info_length shards"
        );
        let vt = &fetched[0];
        assert_eq!(
            vt.block_ref().expect("block_ref should be set in test"),
            block_ref
        );
        assert_eq!(vt.transactions(), txs);

        h.handle
            .stop()
            .await
            .expect("We should expect graceful shutdown");
    }

    /// Test that once a FullTransaction message is received, the reconstructor
    /// stops collecting shards and does not reconstruct even if enough shards
    /// arrive
    #[tokio::test]
    async fn test_stop_collecting_shards_when_full_transaction_arrives() {
        telemetry_subscribers::init_for_testing();

        // GIVEN
        let h = TestHarness::new(15);
        let context = &h.context;
        let transaction_message_sender = h.tx.clone();

        // Create block header & transactions
        let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(7, 1).build());
        let block_ref = header.reference();

        let txs = Transaction::random_transactions(5, 64);
        let serialized = Transaction::serialize(&txs).unwrap();

        let mut encoder = create_encoder(context);
        let transactions_commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized,
            context,
            &mut encoder,
        )
        .unwrap();

        let info_length = context.committee.info_length();
        let parity_length = context.committee.parity_length();

        let all_shards = encoder
            .encode_serialized_data(&serialized, info_length, parity_length)
            .unwrap();

        // Shuffle shard indices so it's not always the same missing one
        let mut rng = thread_rng();
        let mut indices: Vec<usize> = (0..all_shards.len()).collect();
        indices.shuffle(&mut rng);

        // Take all but one shard
        let almost_all = &indices[..info_length - 1];
        let missing_index = indices[info_length - 1];

        let mut batch = Vec::new();
        // Add all shards except the missing one
        for &i in almost_all {
            batch.push(TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef::new(block_ref, transactions_commitment),
                block_digest: Some(block_ref.digest),
                shard: all_shards[i].clone(),
                shard_index: i,
            }));
        }

        transaction_message_sender.send(batch).await.unwrap();

        // Wait — should not reconstruct yet
        tokio::time::sleep(Duration::from_millis(600)).await;
        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        assert!(
            fetched.is_empty(),
            "With header + (info_length - 1) shards, no reconstruction should happen"
        );

        // WHEN: send a FullTransaction message. The reconstructor should stop
        // collecting shards
        transaction_message_sender
            .send(vec![TransactionMessage::FullTransaction(
                TransactionRef::new(block_ref, transactions_commitment),
            )])
            .await
            .unwrap();

        // Now send ONE more random shard (the missing one to make total info_length)
        let extra_shard_index = indices[missing_index];
        transaction_message_sender
            .send(vec![TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef::new(block_ref, transactions_commitment),
                block_digest: Some(block_ref.digest),
                shard: all_shards[extra_shard_index].clone(),
                shard_index: extra_shard_index,
            })])
            .await
            .unwrap();

        // Wait and check that no reconstruction happens
        tokio::time::sleep(Duration::from_millis(600)).await;
        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        assert!(
            fetched.is_empty(),
            "Once FullTransaction is received, reconstructor should ignore shards and not reconstruct"
        );

        // Clean up
        h.handle
            .stop()
            .await
            .expect("We should expect graceful shutdown");
    }

    /// Test reconstruction over multiple rounds with one authority that has a
    /// blocked connection
    #[tokio::test]
    async fn test_reconstruction_over_multiple_rounds_with_missing_authority() {
        telemetry_subscribers::init_for_testing();

        // GIVEN
        let committee_size = 4;
        let h = TestHarness::new(committee_size);
        let context = &h.context;
        let tx = h.tx.clone();

        let mut encoder = create_encoder(context);
        let info_len = context.committee.info_length();
        let parity_len = context.committee.parity_length();

        // Authority that never sends bundles
        let blocked_authority: u8 = 1;

        // === Create initial round 0 ===
        let mut headers_prev = Vec::new();
        let mut shards_prev = Vec::new();
        for auth in 0..committee_size as u8 {
            let txs = Transaction::random_transactions(3, 32);
            let serialized = Transaction::serialize(&txs).unwrap();
            let commitment = TransactionsCommitment::compute_transactions_commitment(
                &serialized,
                context,
                &mut encoder,
            )
            .unwrap();

            let header = VerifiedBlockHeader::new_for_test(
                TestBlockHeader::new(0, auth)
                    .set_commitment(commitment)
                    .build(),
            );

            let shards = encoder
                .encode_serialized_data(&serialized, info_len, parity_len)
                .unwrap();

            headers_prev.push(header);
            shards_prev.push(shards);
        }

        // === Simulate rounds 1..=10 ===
        for round in 1..=10 {
            let mut headers_cur = Vec::new();
            let mut shards_cur = Vec::new();

            // Generate data for all authorities in current round
            for auth in 0..committee_size as u8 {
                let txs = Transaction::random_transactions(3, 32);
                let serialized = Transaction::serialize(&txs).unwrap();
                let commitment = TransactionsCommitment::compute_transactions_commitment(
                    &serialized,
                    context,
                    &mut encoder,
                )
                .unwrap();

                let header = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, auth)
                        .set_commitment(commitment)
                        .build(),
                );

                let shards = encoder
                    .encode_serialized_data(&serialized, info_len, parity_len)
                    .unwrap();

                headers_cur.push(header);
                shards_cur.push(shards);
            }

            // Send bundles from all but the missing authority
            for auth in 0..committee_size as u8 {
                if auth == blocked_authority {
                    continue;
                }

                let mut msgs = prepare_bundle_messages(
                    auth,
                    headers_cur[auth as usize].clone(),
                    &headers_prev,
                    &shards_prev,
                );

                if round == 1 {
                    // Exclude shards from round 0 for the first round to simulate
                    msgs.retain(|msg| !matches!(msg, TransactionMessage::Shard(_)));
                }

                tx.send(msgs).await.unwrap();
            }

            // Advance: current round becomes next round's "previous"
            headers_prev = headers_cur;
            shards_prev = shards_cur;
        }

        // WHEN: let the reconstructor work
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // THEN: we should have reconstructed exactly 9 missing sets (from round 1 to 9)
        // for the blocked authority
        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(
            fetched.len(),
            9,
            "We should reconstruct exactly one missing block per round for the missing authority"
        );

        // Check all reconstructed transactions correspond to the missing authority
        for vt in &fetched {
            assert_eq!(
                vt.author().value(),
                blocked_authority as usize,
                "Reconstructed block must belong to the blocked authority"
            );
        }

        h.handle.stop().await.unwrap();
    }

    /// In a `BlockBundle` the shards belong to blocks from *previous* rounds,
    /// not to the bundle's own (`carrier`) block. The correct `block_ref` for
    /// each `ShardMessage` must therefore come from the shard itself, not from
    /// the carrier block passed to `create_transaction_messages`.
    #[test]
    fn test_create_transaction_messages_shard_uses_shard_block_ref_not_carrier_block_ref() {
        // GIVEN: a carrier block (round 2, authority 0) — the block in the current
        // bundle.
        let carrier_block = VerifiedBlock::new_for_test(TestBlockHeader::new(2, 0).build());

        // GIVEN: a shard-source block (round 1, authority 1) — the block the shard
        // was erasure-coded from. It is from a *different* round and author than the
        // carrier block, which is the normal situation inside a BlockBundle.
        let shard_source = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(1, 1).build());
        let shard_source_ref = shard_source.reference();

        // Sanity: the two blocks must have distinct references for the test to be
        // meaningful.
        assert_ne!(
            carrier_block.reference(),
            shard_source_ref,
            "Test pre-condition: carrier and shard-source blocks must differ"
        );

        // GIVEN: a ShardWithProof whose block_ref points to shard_source (V1 variant,
        // transaction_ref_enabled = false).
        let shard_with_proof = ShardWithProof::new(
            vec![0u8; 32],
            vec![],
            shard_source_ref,
            shard_source.transactions_commitment(),
            false,
        );

        // WHEN: build transaction messages using the carrier block together with a
        // shard that belongs to shard_source.
        let messages =
            TransactionMessage::create_transaction_messages(&carrier_block, &[shard_with_proof], 1);

        // THEN: the shard message must carry the shard's own transaction reference
        // (round=1, authority=1), not the carrier block's reference (round=2,
        // authority=0).
        let shard_msgs: Vec<_> = messages
            .iter()
            .filter(|m| matches!(m, TransactionMessage::Shard(_)))
            .collect();

        assert_eq!(shard_msgs.len(), 1, "Expected exactly one shard message");

        let shard_tx_ref = shard_msgs[0].transaction_ref();
        assert_eq!(
            shard_tx_ref.round, shard_source_ref.round,
            "ShardMessage.transaction_ref.round must match the shard-source block's round"
        );
        assert_eq!(
            shard_tx_ref.author, shard_source_ref.author,
            "ShardMessage.transaction_ref.author must point to the shard-source block's author \
             (authority=1), not the carrier block's author (authority=0)"
        );
    }
}
