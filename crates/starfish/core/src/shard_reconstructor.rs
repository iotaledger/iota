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
    BlockRef, Round, Transaction,
    block_header::{
        BlockHeaderDigest, GENESIS_ROUND, Shard, ShardWithProof, TransactionsCommitment,
        VerifiedBlock, VerifiedTransactions,
    },
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::{DagState, TransactionSource},
    decoder::{ShardsDecoder, create_decoder},
    encoder::{ShardEncoder, create_encoder},
    error::{ConsensusError, ConsensusResult},
};

const EVICTION_TIMEOUT: Duration = Duration::from_secs(1);

const SEND_TO_CORE_RECONSTRUCTED_TXS_TIMEOUT: Duration = Duration::from_millis(100);
const NUMBER_OF_RECONSTRUCTION_WORKERS: usize = 5;

/// Using transaction messages we update the state of shard reconstructor
/// Two types of messages are supported: full transaction and shard
#[derive(Clone, Debug)]
pub(crate) enum TransactionMessage {
    FullTransaction(FullTransactionMessage),
    Shard(ShardMessage),
}

/// Shard message contains shard with index and the reference to a block
/// corresponding to the shard and the commitment in the respected block header
#[derive(Clone, Debug)]
pub(crate) struct ShardMessage {
    block_ref: BlockRef,
    transactions_commitment: TransactionsCommitment,
    shard: Shard,
    shard_index: usize,
}

/// Full transaction message acknowledge that the respected transactions from a
/// given block were verified and locally available
#[derive(Clone, Debug)]
pub(crate) struct FullTransactionMessage {
    block_ref: BlockRef,
    transactions_commitment: TransactionsCommitment,
}

impl TransactionMessage {
    pub fn block_ref(&self) -> BlockRef {
        match self {
            TransactionMessage::FullTransaction(msg) => msg.block_ref,
            TransactionMessage::Shard(msg) => msg.block_ref,
        }
    }

    pub fn transactions_commitment(&self) -> TransactionsCommitment {
        match self {
            TransactionMessage::FullTransaction(msg) => msg.transactions_commitment,
            TransactionMessage::Shard(msg) => msg.transactions_commitment,
        }
    }

    /// Create transaction messages (full, shards) for a given block
    /// bundle
    pub fn create_transaction_messages(
        block: &VerifiedBlock,
        shards: &[ShardWithProof],
        shard_index: usize,
    ) -> Vec<TransactionMessage> {
        let mut messages = Vec::new();

        // Full transaction message
        let full_msg = FullTransactionMessage {
            block_ref: block.reference(),
            transactions_commitment: block.transactions_commitment(),
        };
        messages.push(TransactionMessage::FullTransaction(full_msg));

        // Shard messages
        for shard_with_proof in shards {
            let shard_msg = ShardMessage {
                block_ref: shard_with_proof.block_ref,
                transactions_commitment: shard_with_proof.transaction_commitment,
                shard: shard_with_proof.shard.clone(),
                shard_index,
            };
            messages.push(TransactionMessage::Shard(shard_msg));
        }

        messages
    }
}

/// A basic structure that represents the collection of shards for a given block
/// reference and transaction commitment. We track the number of shards and the
/// shard themselves
#[derive(Clone)]
pub struct ShardAccumulator {
    /// Reference to the block these shards correspond to
    block_ref: BlockRef,
    /// Commitment to the transactions in the block
    transactions_commitment: TransactionsCommitment,
    /// Collected shards, indexed by their shard index
    collected_shards: Vec<Option<Shard>>,
    /// Number of collected data shards
    number_shards: usize,
}

impl ShardAccumulator {
    /// Create a new accumulator initialized with the first shard
    fn new_with_shard(msg: ShardMessage, total_length: usize) -> Self {
        let ShardMessage {
            block_ref,
            transactions_commitment,
            shard,
            shard_index,
        } = msg;
        let mut collected_shards = vec![None; total_length];
        collected_shards[shard_index] = Some(shard);
        Self {
            block_ref,
            transactions_commitment,
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
        if computed_commitment != self.transactions_commitment {
            return Err(ConsensusError::TransactionCommitmentMismatch {
                block_ref: self.block_ref,
            });
        }

        Ok(VerifiedTransactions::new(
            transactions,
            self.block_ref,
            self.transactions_commitment,
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
    processed_transactions: BTreeSet<BlockRef>,
    /// A cache of reconstructed transactions that will be periodically sent in
    /// the core
    reconstructed_transactions: BTreeMap<BlockRef, VerifiedTransactions>,
    /// A map of all shard accumulators. Periodically evicted. Keyed by a pair
    /// (BlockRef, TransactionsCommitment) since transaction commitment is not
    /// supposed to be verified against the block ref when receiving by
    /// ShardReconstructor
    shard_accumulators: BTreeMap<(BlockRef, TransactionsCommitment), ShardAccumulator>,
    /// Use only read access to the dag state to read the transaction GC round
    /// and check whether the respected headers are available
    dag_state: Arc<RwLock<DagState>>,
    /// The receiver for transaction message sent from the authority service
    transaction_message_receiver: Receiver<Vec<TransactionMessage>>,
    /// After full reconstruction and verification, send data to the core
    core_dispatcher: Arc<C>,
    /// Queue is used to not reconstruct the same data twice
    reconstruction_queue: BTreeSet<BlockRef>,
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
            context: context.clone(),
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
                                        "Successfully reconstructed transactions for block {:?}",
                                        shard_accumulator.block_ref
                                    );
                                    if let Err(err) = result_tx.send(verified_transactions).await {
                                        warn!(
                                            "Failed to send the result to shard accumulator {err}"
                                        );
                                    }
                                }
                                Err(err) => {
                                    warn!(
                                        "Failed to reconstruct transactions for block {:?}: {:?}",
                                        shard_accumulator.block_ref, err
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
                        self.processed_transactions.insert(verified_transactions.block_ref());
                        self.reconstruction_queue.remove(&verified_transactions.block_ref());
                        self.reconstructed_transactions.insert(verified_transactions.block_ref(), verified_transactions);
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

        let transaction_gc_round = self.dag_state.read().gc_round_for_last_solid_commit();

        // Update the internal transaction_gc_round
        self.transaction_gc_round = transaction_gc_round;

        let lower_bound = BlockRef::new(
            transaction_gc_round,
            AuthorityIndex::ZERO,
            BlockHeaderDigest::MIN,
        );

        self.processed_transactions = self.processed_transactions.split_off(&lower_bound);
        self.reconstructed_transactions = self.reconstructed_transactions.split_off(&lower_bound);
        let lower_bound_key = (lower_bound, TransactionsCommitment::MIN);
        self.shard_accumulators = self.shard_accumulators.split_off(&lower_bound_key);
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
                let block_headers_opt = {
                    let block_refs: Vec<BlockRef> = transactions_map.keys().copied().collect();
                    self.dag_state.read().get_block_headers(&block_refs)
                };
                for (block_header_opt, (block_ref, transactions)) in block_headers_opt
                    .into_iter()
                    .zip(transactions_map.into_iter())
                {
                    if let Some(block_header) = block_header_opt {
                        // Check the correctness of the transactions commitment
                        assert_eq!(
                            block_header.transactions_commitment(),
                            transactions.transactions_commitment(),
                            "The network has at least f+1 Byzantine validators"
                        );
                        ready_to_be_sent_transactions.push(transactions);
                    } else {
                        to_stay_transactions.insert(block_ref, transactions);
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
                let difference =
                    highest_accepted_round.saturating_sub(transaction.block_ref().round);
                self.context
                    .metrics
                    .node_metrics
                    .reconstruction_lag
                    .observe(difference as f64);
            }

            // Add the transactions to the core
            self.core_dispatcher
                .add_transactions(transactions, TransactionSource::ShardReconstructor)
                .await
                .map_err(|_| ConsensusError::Shutdown)?;
        }
        Ok(())
    }

    /// Handle a message and update internal state
    async fn handle_transaction_message(&mut self, msg: TransactionMessage) -> ConsensusResult<()> {
        if self.processed_transactions.contains(&msg.block_ref())
            || self.reconstruction_queue.contains(&msg.block_ref())
            || msg.block_ref().round < self.transaction_gc_round
        {
            return Ok(());
        }

        let key = (msg.block_ref(), msg.transactions_commitment());
        let total_length = self.total_length;

        match msg {
            TransactionMessage::Shard(shard_msg) => match self.shard_accumulators.entry(key) {
                Entry::Vacant(v) => {
                    v.insert(ShardAccumulator::new_with_shard(shard_msg, total_length));
                }
                Entry::Occupied(mut o) => {
                    o.get_mut().update_with_shard(shard_msg);
                }
            },

            TransactionMessage::FullTransaction(full_msg) => {
                self.processed_transactions.insert(full_msg.block_ref);
                return Ok(());
            }
        }

        // Check if we can reconstruct the block now and enqueue it if so
        Self::enqueue_if_ready(
            &mut self.shard_accumulators,
            &mut self.reconstruction_queue,
            &self.ready_to_reconstruct_sender,
            self.info_length,
            &key,
        )
        .await?;

        Ok(())
    }

    /// If the accumulator for the given key is ready to reconstruct, remove it
    /// from the map and enqueue it for reconstruction
    async fn enqueue_if_ready(
        accumulators: &mut BTreeMap<(BlockRef, TransactionsCommitment), ShardAccumulator>,
        reconstruction_queue: &mut BTreeSet<BlockRef>,
        sender: &Sender<ShardAccumulator>,
        info_length: usize,
        key: &(BlockRef, TransactionsCommitment),
    ) -> ConsensusResult<()> {
        if let Some(acc) = accumulators.get(key) {
            if acc.is_ready_to_reconstruct(info_length) {
                // take ownership out of map
                let acc = accumulators
                    .remove(key)
                    .expect("We should expect the shard accumulator to be present");
                sender
                    .send(acc)
                    .await
                    .map_err(|_| ConsensusError::AccumulatorSenderClosed)?;
                reconstruction_queue.insert(key.0);
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
    use tokio::sync::Mutex;

    use crate::{
        BlockRef, Round, TestBlockHeader, Transaction, VerifiedBlockHeader,
        block_header::{
            Shard, TransactionsCommitment, VerifiedBlock, VerifiedOwnShard, VerifiedTransactions,
        },
        commit::CertifiedCommits,
        context::Context,
        core_thread::{CoreError, CoreThreadDispatcher},
        dag_state::{DagState, TransactionSource},
        encoder::create_encoder,
        shard_reconstructor::{
            FullTransactionMessage, ShardMessage, ShardReconstructor, TransactionMessage,
        },
        storage::mem_store::MemStore,
    };

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
            _source: TransactionSource,
        ) -> Result<(), CoreError> {
            let mut guard = self.transactions.lock().await;
            guard.extend(txs);
            Ok(())
        }
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
            _blocks: Vec<VerifiedBlockHeader>,
        ) -> Result<
            (
                BTreeSet<BlockRef>,
                BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>,
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
        ) -> Result<BTreeMap<BlockRef, BTreeSet<AuthorityIndex>>, CoreError> {
            unimplemented!()
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
            FullTransactionMessage {
                block_ref: header_cur.reference(),
                transactions_commitment: header_cur.transactions_commitment(),
            },
        ));

        // 2. The j-th shard of every authority’s transaction data from round i-1
        let j_index = authority_j as usize;
        for (auth_index, shards) in shards_prev.iter().enumerate() {
            if let Some(shard) = shards.get(j_index) {
                msgs.push(TransactionMessage::Shard(ShardMessage {
                    block_ref: headers_prev[auth_index].reference(),
                    transactions_commitment: headers_prev[auth_index].transactions_commitment(),
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
        let committee_size = 10;
        let (context, _) = Context::new_for_test(committee_size);
        let context = Arc::new(context);

        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());

        let handle =
            ShardReconstructor::start(context.clone(), dag_state.clone(), core_dispatcher.clone());
        let transaction_message_sender = handle.transaction_message_sender();

        // Create block header & transactions
        let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build());
        let block_ref = header.reference();

        let txs = Transaction::random_transactions(4, 48);
        let serialized = Transaction::serialize(&txs).unwrap();

        let mut encoder = create_encoder(&context);
        let commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized,
            &context,
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
                block_ref,
                transactions_commitment: commitment,
                shard: all_shards[i].clone(),
                shard_index: i,
            }));
        }

        transaction_message_sender.send(batch).await.unwrap();

        // Wait — should not reconstruct yet
        tokio::time::sleep(Duration::from_millis(400)).await;
        let fetched = core_dispatcher.get_and_drain_transactions().await;
        assert!(
            fetched.is_empty(),
            "With header + (info_length - 1) shards, no reconstruction should happen"
        );

        // Now send ONE more random shard (the missing one to make total info_length)
        let extra_shard_index = indices[info_length - 1];
        transaction_message_sender
            .send(vec![TransactionMessage::Shard(ShardMessage {
                block_ref,
                transactions_commitment: commitment,
                shard: all_shards[extra_shard_index].clone(),
                shard_index: extra_shard_index,
            })])
            .await
            .unwrap();

        // THEN: reconstruction should happen
        tokio::time::sleep(Duration::from_millis(600)).await;
        let fetched = core_dispatcher.get_and_drain_transactions().await;

        assert_eq!(
            fetched.len(),
            1,
            "Reconstruction should happen after reaching info_length shards"
        );
        let vt = &fetched[0];
        assert_eq!(vt.block_ref(), block_ref);
        assert_eq!(vt.transactions(), txs);

        handle
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
        let committee_size = 15;
        let (context, _) = Context::new_for_test(committee_size);
        let context = Arc::new(context);

        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());

        let handle =
            ShardReconstructor::start(context.clone(), dag_state.clone(), core_dispatcher.clone());
        let transaction_message_sender = handle.transaction_message_sender();

        // Create block header & transactions
        let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(7, 1).build());
        let block_ref = header.reference();

        let txs = Transaction::random_transactions(5, 64);
        let serialized = Transaction::serialize(&txs).unwrap();

        let mut encoder = create_encoder(&context);
        let transactions_commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized,
            &context,
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
                block_ref,
                transactions_commitment,
                shard: all_shards[i].clone(),
                shard_index: i,
            }));
        }

        transaction_message_sender.send(batch).await.unwrap();

        // Wait — should not reconstruct yet
        tokio::time::sleep(Duration::from_millis(600)).await;
        let fetched = core_dispatcher.get_and_drain_transactions().await;
        assert!(
            fetched.is_empty(),
            "With header + (info_length - 1) shards, no reconstruction should happen"
        );

        // WHEN: send a FullTransaction message. The reconstructor should stop
        // collecting shards
        transaction_message_sender
            .send(vec![TransactionMessage::FullTransaction(
                FullTransactionMessage {
                    block_ref,
                    transactions_commitment,
                },
            )])
            .await
            .unwrap();

        // Now send ONE more random shard (the missing one to make total info_length)
        let extra_shard_index = indices[missing_index];
        transaction_message_sender
            .send(vec![TransactionMessage::Shard(ShardMessage {
                block_ref,
                transactions_commitment,
                shard: all_shards[extra_shard_index].clone(),
                shard_index: extra_shard_index,
            })])
            .await
            .unwrap();

        // Wait and check that no reconstruction happens
        tokio::time::sleep(Duration::from_millis(600)).await;
        let fetched = core_dispatcher.get_and_drain_transactions().await;
        assert!(
            fetched.is_empty(),
            "Once FullTransaction is received, reconstructor should ignore shards and not reconstruct"
        );

        // Clean up
        handle
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
        let (context, _) = Context::new_for_test(committee_size);
        let context = Arc::new(context);

        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));

        let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());

        let handle =
            ShardReconstructor::start(context.clone(), dag_state.clone(), core_dispatcher.clone());
        let tx = handle.transaction_message_sender();

        let mut encoder = create_encoder(&context);
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
                &context,
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
                    &context,
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
        let fetched = core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(
            fetched.len(),
            9,
            "We should reconstruct exactly one missing block per round for the missing authority"
        );

        // Check all reconstructed transactions correspond to the missing authority
        for vt in &fetched {
            assert_eq!(
                vt.block_ref().author.value(),
                blocked_authority as usize,
                "Reconstructed block must belong to the blocked authority"
            );
        }

        handle.stop().await.unwrap();
    }
}
