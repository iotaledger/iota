// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    sync::Arc,
    time::Duration,
};
use parking_lot::RwLock;
use tokio::{
    sync::{
        Mutex, mpsc,
        mpsc::{Receiver, Sender},
    },
    time::{Instant, sleep_until},
};
use tokio::task::{JoinError, JoinHandle};
use tracing::log::{debug, warn};
use starfish_config::AuthorityIndex;
use crate::{
    BlockRef, Transaction,
    block_header::{Shard, TransactionsCommitment, VerifiedTransactions},
    context::Context,
    core_thread::CoreThreadDispatcher,
    decoder::{ShardsDecoder, create_decoder},
    encoder::{ShardEncoder, create_encoder},
    error::{ConsensusError, ConsensusResult},
};
use crate::block_header::BlockHeaderDigest;
use crate::dag_state::DagState;

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
    /// The block headers was checked on correctness of the transaction
    /// commitment If true, we can trust the commitment and do not need to
    /// verify it again
    header_verified: bool,
}

#[derive(Clone, Debug)]
pub enum TransactionMessage {
    FullTransaction(FullTransactionMessage),
    Shard(ShardMessage),
    Header(HeaderMessage),
}

impl TransactionMessage {
    pub fn block_ref(&self) -> &BlockRef {
        match self {
            TransactionMessage::FullTransaction(msg) => &msg.block_ref,
            TransactionMessage::Shard(msg) => &msg.block_ref,
            TransactionMessage::Header(msg) => &msg.block_ref,
        }
    }

    pub fn transactions_commitment(&self) -> &TransactionsCommitment {
        match self {
            TransactionMessage::FullTransaction(msg) => &msg.commitment,
            TransactionMessage::Shard(msg) => &msg.transactions_commitment,
            TransactionMessage::Header(msg) => &msg.commitment,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ShardMessage {
    block_ref: BlockRef,
    transactions_commitment: TransactionsCommitment,
    shard: Shard,
    shard_index: usize,
}

#[derive(Clone, Debug)]
pub struct HeaderMessage {
    block_ref: BlockRef,
    commitment: TransactionsCommitment,
}

impl HeaderMessage {
    pub fn new(block_ref: BlockRef, commitment: TransactionsCommitment) -> Self {
        Self {
            block_ref,
            commitment,
        }
    }
}

#[derive(Clone, Debug)]
pub struct FullTransactionMessage {
    block_ref: BlockRef,
    commitment: TransactionsCommitment,
}
impl FullTransactionMessage {
    pub fn new(block_ref: BlockRef, commitment: TransactionsCommitment) -> Self {
        Self {
            block_ref,
            commitment,
        }
    }
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
            header_verified: false,
        }
    }

    /// Create a new accumulator initialized with the first header
    fn new_with_header(msg: HeaderMessage, total_length: usize) -> Self {
        let HeaderMessage {
            block_ref,
            commitment,
        } = msg;
        let collected_shards = vec![None; total_length];
        Self {
            block_ref,
            transactions_commitment: commitment,
            collected_shards,
            number_shards: 0,
            header_verified: true,
        }
    }

    /// Update the accumulator with a new shard, returning the new count of
    /// collected shards
    fn update_with_shard(&mut self, msg: ShardMessage) -> usize {
        let ShardMessage {
            shard, shard_index, ..
        } = msg;
        if self.collected_shards[shard_index].is_none() {
            self.collected_shards[shard_index] = Some(shard);
            self.number_shards = self.number_shards + 1;
        }
        self.number_shards
    }

    /// Update the accumulator with a new header
    fn update_with_header(&mut self) {
        self.header_verified = true;
    }

    fn is_ready_to_reconstruct(&self, info_length: usize) -> bool {
        self.number_shards >= info_length && self.header_verified
    }

    fn decode_block(&self, codec: &mut Codec) -> ConsensusResult<VerifiedTransactions> {
        let transactions = codec.decoder.decode_shards(
            codec.info_length,
            codec.parity_length,
            self.collected_shards.clone(),
        )?;

        let serialized =
            Transaction::serialize(&transactions).expect("We should expect serialization to work");

        // Verify the commitment
        // TODO: remove this verification since the header was verified before the
        // reconstruction and f+1 shards guarantee that one honest validator already
        // checked the alignment between the commitment and the header.
        let computed_commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized,
            &codec.context.clone(),
            &mut codec.encoder,
        )?;
        if computed_commitment != self.transactions_commitment {
            return Err(ConsensusError::TransactionCommitmentMismatch {
                block_ref: self.block_ref.clone(),
            });
        }

        Ok(VerifiedTransactions::new(
            transactions,
            self.block_ref.clone(),
            self.transactions_commitment,
            serialized,
        ))
    }
}

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

const EVICTION_TIMEOUT: Duration = Duration::from_secs(1);

const SEND_TO_CORE_RECONSTRUCTED_TXS_TIMEOUT: Duration = Duration::from_millis(100);
const NUMBER_OF_RECONSTRUCTION_WORKERS: usize = 5;

pub struct ShardReconstructorHandle {
    pub transaction_message_sender: Sender<Vec<TransactionMessage>>,
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
                Err(e) => Err(e), // propagate panic or other errors
            }
        } else {
            Ok(()) // already stopped
        }
    }
}

impl<C: CoreThreadDispatcher + 'static> ShardReconstructor<C> {
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


pub struct ShardReconstructor<C: CoreThreadDispatcher> {
    info_length: usize,
    total_length: usize,
    context: Arc<Context>,
    processed_transactions: BTreeSet<BlockRef>,
    reconstructed_transactions: Vec<VerifiedTransactions>,
    accumulators: BTreeMap<(BlockRef, TransactionsCommitment), ShardAccumulator>,
    dag_state: Arc<RwLock<DagState>>,
    transaction_message_receiver: Receiver<Vec<TransactionMessage>>,
    core_dispatcher: Arc<C>,
    reconstruction_queue: BTreeSet<BlockRef>,
    ready_to_reconstruct_sender: Sender<ShardAccumulator>,
    ready_to_reconstruct_receiver: Arc<Mutex<Receiver<ShardAccumulator>>>,
    reconstructed_transactions_receiver: Receiver<VerifiedTransactions>,
    reconstructed_transactions_sender: Sender<VerifiedTransactions>,
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
            reconstruction_queue: BTreeSet::new(),
            ready_to_reconstruct_sender: ready_sender,
            ready_to_reconstruct_receiver: Arc::new(Mutex::new(ready_receiver)),
            reconstructed_transactions_sender: result_sender,
            reconstructed_transactions_receiver: result_receiver,
            processed_transactions: BTreeSet::new(),
            reconstructed_transactions: Vec::new(),
            accumulators: BTreeMap::new(),
            transaction_message_receiver,
        };

        (reconstructor, transaction_message_sender)
    }

    pub fn start_reconstruction_workers(&self) {
        for _ in 0..NUMBER_OF_RECONSTRUCTION_WORKERS {
            let mut codec = Codec::new(&self.context);
            let ready_rx = Arc::clone(&self.ready_to_reconstruct_receiver);
            let result_tx = self.reconstructed_transactions_sender.clone();

            tokio::spawn(async move {
                loop {
                    // Receive a job from the ready to reconstruct channel
                    let job = {
                        let mut rx = ready_rx.lock().await;
                        rx.recv().await
                    };

                    match job {
                        Some(shard_accumulator) => {
                            match shard_accumulator.decode_block(&mut codec) {
                                Ok(verified_transactions) => {
                                    debug!(
                                        "Successfully reconstructed transactions for block {:?}",
                                        shard_accumulator.block_ref
                                    );
                                    let _ = result_tx.send(verified_transactions).await;
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to reconstruct transactions for block {:?}: {:?}",
                                        shard_accumulator.block_ref, e
                                    );
                                }
                            }
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
                                    if let Err(e) = self.handle_transaction_message(msg.clone()) {
                                        debug!("Error when handling transaction message{:?}: {:?}", msg, e);
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
                        self.reconstructed_transactions.push(verified_transactions);
                    }

                 () = &mut send_to_core_timeout => {
                    // we want to start a new task only if the number of tasks is not too large.
                    if let Err(e) = self.send_to_core().await {
                        debug!("Error when sending reconstructed transactions to core: {:?}", e);
                    }

                    send_to_core_timeout
                        .as_mut()
                        .reset(Instant::now() + SEND_TO_CORE_RECONSTRUCTED_TXS_TIMEOUT);
                        }

                 () = &mut eviction_timeout => {
                    // we want to start a new task only if the number of tasks is not too large.
                    self.evict_memory();

                    eviction_timeout
                        .as_mut()
                        .reset(Instant::now() + EVICTION_TIMEOUT);
                }

            }
        }
    }

    /// Evict old accumulators and processed transactions to free memory. We read the dag state to find the
    /// transaction garbage collection round and evict all accumulators and processed transactions
    /// below that round.
    fn evict_memory(&mut self) {
        let transaction_gc_round = self.dag_state.read().gc_round_for_last_solid_commit();

        let lower_bound =
            BlockRef::new(transaction_gc_round, AuthorityIndex::ZERO, BlockHeaderDigest::MIN);


        self.processed_transactions = self
            .processed_transactions
            .split_off(&lower_bound);

        let lower_bound_key = (lower_bound, TransactionsCommitment::MIN);
        self.accumulators = self
            .accumulators
            .split_off(&lower_bound_key);

    }

    /// Send reconstructed transactions to the core
    async fn send_to_core(&mut self) -> ConsensusResult<()> {
        let transactions = std::mem::take(&mut self.reconstructed_transactions);
        if !transactions.is_empty() {
            // Add the transactions to the core
            self.core_dispatcher
                .add_transactions(transactions)
                .await
                .map_err(|_| ConsensusError::Shutdown)?;
        }
        Ok(())
    }

    /// Handle a message and update internal state
    fn handle_transaction_message(&mut self, msg: TransactionMessage) -> ConsensusResult<()> {
        if self.processed_transactions.contains(msg.block_ref()) || self.reconstruction_queue.contains(msg.block_ref()) {
            return Ok(());
        }

        let key = (
            msg.block_ref().clone(),
            msg.transactions_commitment().clone(),
        );
        let total_length = self.total_length;

        match msg {
            TransactionMessage::Shard(shard_msg) => match self.accumulators.entry(key.clone()) {
                Entry::Vacant(v) => {
                    v.insert(ShardAccumulator::new_with_shard(shard_msg, total_length));
                }
                Entry::Occupied(mut o) => {
                    o.get_mut().update_with_shard(shard_msg);
                }
            },

            TransactionMessage::Header(header_msg) => match self.accumulators.entry(key.clone()) {
                Entry::Vacant(v) => {
                    v.insert(ShardAccumulator::new_with_header(header_msg, total_length));
                }
                Entry::Occupied(mut o) => {
                    o.get_mut().update_with_header();
                }
            },

            TransactionMessage::FullTransaction(full_msg) => {
                self.processed_transactions.insert(full_msg.block_ref);
                return Ok(());
            }
        }

        // Check if we can reconstruct the block now and enqueue it if so
        Self::enqueue_if_ready(
            &mut self.accumulators,
            &mut self.reconstruction_queue,
            &self.ready_to_reconstruct_sender,
            self.info_length,
            &key,
        )?;

        Ok(())
    }

    fn enqueue_if_ready(
        accumulators: &mut BTreeMap<(BlockRef, TransactionsCommitment), ShardAccumulator>,
        reconstruction_queue: &mut BTreeSet<BlockRef>,
        sender: &Sender<ShardAccumulator>,
        info_length: usize,
        key: &(BlockRef, TransactionsCommitment),
    ) -> ConsensusResult<()> {
        if let Some(acc) = accumulators.get(key) {
            if acc.is_ready_to_reconstruct(info_length) {
                // take ownership out of map
                let acc = accumulators.remove(key).expect("We should expect the shard accumulator to be present");
                sender.try_send(acc).map_err(|_| ConsensusError::AccumulatorSenderClosed)?;
                reconstruction_queue.insert(key.0.clone());
            }
        }
        Ok(())
    }
}
