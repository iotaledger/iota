// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    sync::Arc,
    time::Duration,
};

use parking_lot::RwLock;
use starfish_config::{AuthorityIndex, Committee, Stake};
use tokio::{
    sync::{
        Mutex, mpsc,
        mpsc::{Receiver, Sender},
    },
    task::{JoinError, JoinHandle},
    time::{Instant, sleep_until},
};
use tracing::{debug, error, warn};

use crate::{
    Round, Transaction,
    block_header::{
        BlockHeaderDigest, CommitmentVerifiedTransactions, GENESIS_ROUND, Shard, ShardWithProof,
        ShardWithProofAPI, TransactionsCommitment, VerifiedBlock,
    },
    block_verifier::BlockVerifier,
    context::Context,
    core_thread::CoreThreadDispatcher,
    dag_state::{DagState, DataSource},
    decoder::{ShardsDecoder, create_decoder},
    encoder::{ShardEncoder, create_encoder},
    error::{ConsensusError, ConsensusResult},
    misbehavior_store::MisbehaviorStore,
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
    /// Collected shards, one slot per authority, indexed by the authority index
    /// of the peer that relayed the shard.
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

    /// Ready once enough shards for decoding are collected and the relayers'
    /// combined stake reaches the validity threshold (f+1), which guarantees
    /// an honest relayer and thus a genuinely authored commitment.
    fn is_ready_to_reconstruct(&self, info_length: usize, committee: &Committee) -> bool {
        if self.number_shards < info_length {
            return false;
        }
        let relayer_stake: Stake = self
            .collected_shard_indices()
            .filter_map(|i| committee.to_authority_index(i))
            .map(|i| committee.stake(i))
            .sum();
        committee.reached_validity(relayer_stake)
    }

    /// Indices of the shards collected so far. A shard at index `i` is
    /// authenticated as authority `i`'s (its Merkle proof is verified at
    /// position `i`, which is the relaying peer's index), so a collected
    /// shard identifies the peer that relayed it.
    fn collected_shard_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.collected_shards
            .iter()
            .enumerate()
            .filter_map(|(i, shard)| shard.as_ref().map(|_| i))
    }

    /// Decodes the transaction data from the collected shards and verifies
    /// the reconstructed bytes against the transactions commitment in the
    /// ref. Consumes the accumulator, so the collected shards move into the
    /// decoder rather than being copied.
    fn decode_and_verify_commitment(
        self,
        codec: &mut Codec,
    ) -> ConsensusResult<CommitmentVerifiedTransactions> {
        let Self {
            transaction_ref,
            block_digest,
            collected_shards,
            ..
        } = self;

        let transactions = codec.decoder.decode_shards(
            codec.info_length,
            codec.parity_length,
            collected_shards,
        )?;

        let serialized =
            Transaction::serialize(&transactions).expect("We should expect serialization to work");

        // Verify the commitment
        let computed_commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized,
            &codec.context.clone(),
            &mut codec.encoder,
        )?;
        if computed_commitment != transaction_ref.transactions_commitment {
            return Err(ConsensusError::TransactionCommitmentMismatch { transaction_ref });
        }

        Ok(CommitmentVerifiedTransactions::new(
            transactions,
            transaction_ref,
            block_digest,
            serialized,
        ))
    }

    /// Whether a shard is already collected at the given index.
    fn contains_shard_at_index(&self, shard_index: usize) -> bool {
        self.collected_shards[shard_index].is_some()
    }

    /// Remove the shard collected at the given index, if any.
    fn remove_shard_at_index(&mut self, shard_index: usize) {
        if self.collected_shards[shard_index].take().is_some() {
            self.number_shards -= 1;
        }
    }
}

/// Attributes a reconstructed payload that failed the transaction validity
/// check.
///
/// A peer must hold the full payload to erasure-code its shard, so every peer
/// that contributed a shard could have checked the transactions' validity and
/// is charged an unprovable fault for relaying invalid bytes. The commitment
/// here is peer-supplied, so without a verified author-signed header for this
/// ref a coalition of peers could reconstruct invalid transactions under a
/// fabricated commitment to frame the author; charge the author a provable
/// fault only when such a header exists, as he then committed to the invalid
/// transactions.
///
/// Attribution is one-shot: a header arriving only after this failure does not
/// retroactively charge the author (the failed ref is marked processed and not
/// revisited until garbage collection). Such an author is instead charged on
/// the direct primary-block route, where the full payload is verified against
/// the author.
fn record_reconstruction_validity_failure(
    dag_state: &RwLock<DagState>,
    misbehavior_store: &MisbehaviorStore,
    tx_ref: TransactionRef,
    relayers: Vec<AuthorityIndex>,
    err: &ConsensusError,
) {
    let author = tx_ref.author;
    let authored = dag_state
        .read()
        .contains_verified_block_headers_for_transaction_refs(&[tx_ref])[0];
    misbehavior_store.record_faulty_transactions(author, authored, relayers);
    error!(
        "Reconstructed transactions for {:?} failed the validity check: {:?}",
        tx_ref, err
    );
}

/// Charges the peers that relayed shards for a reconstructed payload whose
/// commitment doesn't match the one its ref commits to. The mismatch comes from
/// the peer-supplied shards, not the author, so no author fault is recorded
/// even when a verified header for the ref exists.
fn record_reconstruction_commitment_mismatch(
    misbehavior_store: &MisbehaviorStore,
    tx_ref: TransactionRef,
    relayers: Vec<AuthorityIndex>,
) {
    misbehavior_store.record_faulty_transactions(tx_ref.author, false, relayers);
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
        block_verifier: Arc<dyn BlockVerifier>,
    ) -> Arc<ShardReconstructorHandle> {
        let (mut reconstructor, transaction_message_sender) =
            ShardReconstructor::new(context, dag_state, core_dispatcher, block_verifier);

        let join_handle = tokio::spawn(async move {
            reconstructor.run().await;
        });

        Arc::new(ShardReconstructorHandle {
            transaction_message_sender,
            join_handle: Mutex::new(Some(join_handle)),
        })
    }
}

/// Result of a reconstruction job: the verified transactions on success, or
/// the failed job's transaction reference so its queue entry can be dropped.
type ReconstructionResult = Result<CommitmentVerifiedTransactions, TransactionRef>;

/// The main structure responsible for collecting shards and reconstructing
/// transaction data once enough shards are collected. Keeps track of already
/// locally available transaction data. The transaction is reconstructed only
/// when it is still not locally available and enough shards are reconstructed.
/// The structure periodically sends data to the core. In addition, eviction
/// mechanism is implemented by relying on the transaction GC round.
pub struct ShardReconstructor<C: CoreThreadDispatcher> {
    /// Shards below this round will not be collected
    transaction_gc_round: Round,
    /// Minimum number of shards the decoder needs to reconstruct the data
    info_length: usize,
    /// The total number of shards
    total_length: usize,
    context: Arc<Context>,
    /// Already processed transaction either by authority service or by shard
    /// reconstructor
    processed_transactions: BTreeSet<TransactionRef>,
    /// A cache of reconstructed transactions that will be periodically sent in
    /// the core
    reconstructed_transactions: BTreeMap<TransactionRef, CommitmentVerifiedTransactions>,
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
    /// Applies the same transaction limit and batch verification checks to
    /// reconstructed payloads as the direct block-bundle route
    block_verifier: Arc<dyn BlockVerifier>,
    /// Charges faults for payloads that fail verification: the author, when a
    /// verified header proves the payload is theirs, and the peers that relayed
    /// the shards.
    misbehavior_store: Arc<MisbehaviorStore>,
    /// Queue is used to not reconstruct the same data twice
    reconstruction_queue: BTreeSet<TransactionRef>,
    /// Once enough shards are collected, they are sent to reconstructor workers
    ready_to_reconstruct_sender: Sender<ShardAccumulator>,
    /// Channel to receive accumulated shard for reconstruction by workers
    ready_to_reconstruct_receiver: Arc<Mutex<Receiver<ShardAccumulator>>>,
    /// Reconstruction workers report each job's result through this channel
    reconstruction_result_sender: Sender<ReconstructionResult>,
    /// Job results are received by this channel
    reconstruction_result_receiver: Receiver<ReconstructionResult>,
    /// For each authority, the accumulators currently retaining a shard it
    /// relayed. Enforces the per-authority shard budget: at the budget, the
    /// authority's oldest retained shard is evicted to admit a new one.
    retained_shards_by_authority: Vec<BTreeSet<TransactionRef>>,
    /// Slots whose genuine payload is already known, from a successful decode
    /// or a directly received full payload. Their accumulators are purged and
    /// further shards for them are dropped. Periodically evicted by round.
    resolved_slots: BTreeSet<(Round, AuthorityIndex)>,
    /// Maximum number of shards from one relaying authority retained across
    /// all accumulators.
    shard_budget_per_authority: usize,
}

impl<C: CoreThreadDispatcher> ShardReconstructor<C> {
    /// Create a new ShardReconstructor and its associated Sender
    pub fn new(
        context: Arc<Context>,
        dag_state: Arc<RwLock<DagState>>,
        core_dispatcher: Arc<C>,
        block_verifier: Arc<dyn BlockVerifier>,
    ) -> (Self, Sender<Vec<TransactionMessage>>) {
        let info_length = context.committee.info_length();
        let total_length = context.committee.size();
        let shard_budget_per_authority = context.parameters.shard_budget_per_authority as usize;

        let (transaction_message_sender, transaction_message_receiver) = mpsc::channel(1000);
        let (ready_sender, ready_receiver) = mpsc::channel(1000);
        let (result_sender, result_receiver) = mpsc::channel(1000);

        let misbehavior_store = dag_state.read().misbehavior_store().clone();
        let reconstructor = Self {
            info_length,
            total_length,
            context,
            core_dispatcher,
            dag_state,
            block_verifier,
            misbehavior_store,
            transaction_gc_round: GENESIS_ROUND,
            reconstruction_queue: BTreeSet::new(),
            ready_to_reconstruct_sender: ready_sender,
            ready_to_reconstruct_receiver: Arc::new(Mutex::new(ready_receiver)),
            reconstruction_result_sender: result_sender,
            reconstruction_result_receiver: result_receiver,
            processed_transactions: BTreeSet::new(),
            reconstructed_transactions: BTreeMap::new(),
            shard_accumulators: BTreeMap::new(),
            transaction_message_receiver,
            retained_shards_by_authority: vec![BTreeSet::new(); total_length],
            resolved_slots: BTreeSet::new(),
            shard_budget_per_authority,
        };

        (reconstructor, transaction_message_sender)
    }

    pub fn start_reconstruction_workers(&self) {
        for _ in 0..NUMBER_OF_RECONSTRUCTION_WORKERS {
            let mut codec = Codec::new(&self.context);
            let ready_rx = Arc::clone(&self.ready_to_reconstruct_receiver);
            let result_tx = self.reconstruction_result_sender.clone();
            let context = self.context.clone();
            let dag_state = self.dag_state.clone();
            let block_verifier = self.block_verifier.clone();
            let misbehavior_store = self.misbehavior_store.clone();
            tokio::spawn(async move {
                let metrics = &context.metrics;
                // Receive a job from the ready to reconstruct channel until it closes.
                while let Some(shard_accumulator) = {
                    let mut rx = ready_rx.lock().await;
                    rx.recv().await
                } {
                    metrics.node_metrics.reconstruction_jobs_started.inc();
                    // Read what the failure paths attribute with before decoding
                    // consumes the accumulator.
                    let tx_ref = shard_accumulator.transaction_ref;
                    let relayers: Vec<_> = shard_accumulator
                        .collected_shard_indices()
                        .filter_map(|i| context.committee.to_authority_index(i))
                        .collect();
                    let result = match shard_accumulator.decode_and_verify_commitment(&mut codec) {
                        // Validity-threshold relayer stake guarantees an honest
                        // relayer and thus a genuine commitment, so a decode
                        // failure indicates a codec bug or Byzantine stake
                        // beyond the fault model.
                        Err(err) => {
                            error!("Failed to reconstruct transactions for {tx_ref:?}: {err:?}");
                            // A commitment mismatch means the reconstructed bytes
                            // aren't the ones the author committed to; the shards,
                            // and thus the mismatch, come from peers, so charge
                            // only the peers that relayed shards, never the author.
                            if matches!(err, ConsensusError::TransactionCommitmentMismatch { .. }) {
                                record_reconstruction_commitment_mismatch(
                                    &misbehavior_store,
                                    tx_ref,
                                    relayers,
                                );
                            }
                            Err(tx_ref)
                        }
                        Ok(verified_transactions) => match block_verifier
                            .verify_transactions_validity(&verified_transactions)
                        {
                            Ok(()) => {
                                debug!("Successfully reconstructed transactions for {tx_ref:?}");
                                Ok(verified_transactions)
                            }
                            Err(err) => {
                                record_reconstruction_validity_failure(
                                    &dag_state,
                                    &misbehavior_store,
                                    tx_ref,
                                    relayers,
                                    &err,
                                );
                                Err(tx_ref)
                            }
                        },
                    };
                    if let Err(err) = result_tx.send(result).await {
                        warn!("Failed to send the result to shard accumulator {err}");
                    }
                    metrics.node_metrics.reconstruction_jobs_finished.inc();
                }
                debug!("Ready to reconstruct channel closed, workers exiting");
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
                    // A reconstruction job finished in one of the reconstruction workers
                    Some(result) = self.reconstruction_result_receiver.recv() => {
                        // Success and failure are both final for the ref: the shards
                        // proved membership against the commitment inside the ref, so
                        // a failed job would fail identically on retry.
                        let tx_ref = match &result {
                            Ok(verified_transactions) => verified_transactions.transaction_ref(),
                            Err(tx_ref) => *tx_ref,
                        };
                        self.processed_transactions.insert(tx_ref);
                        self.reconstruction_queue.remove(&tx_ref);
                        if let Ok(verified_transactions) = result {
                            self.reconstructed_transactions.insert(tx_ref, verified_transactions);
                            self.resolve_slot(tx_ref);
                        }
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
        self.evict_below(transaction_gc_round);
    }

    /// Evict accumulators, processed and reconstructed transactions, resolved
    /// slots, and retained-shard bookkeeping below the given round.
    fn evict_below(&mut self, transaction_gc_round: Round) {
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
        self.reconstruction_queue = self.reconstruction_queue.split_off(&lower_bound);
        self.resolved_slots = self
            .resolved_slots
            .split_off(&(transaction_gc_round, AuthorityIndex::ZERO));
        for retained in &mut self.retained_shards_by_authority {
            *retained = retained.split_off(&lower_bound);
        }
    }

    fn get_transactions_with_headers_in_dag_state(
        &mut self,
    ) -> Vec<CommitmentVerifiedTransactions> {
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
                for (exists, (tx_ref, transactions)) in
                    block_headers_exist.into_iter().zip(transactions_map)
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
        let transactions = self.get_transactions_with_headers_in_dag_state();
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

        let shard_msg = match msg {
            TransactionMessage::FullTransaction(tx_ref) => {
                self.processed_transactions.insert(tx_ref);
                // The full payload arrived through the direct path, so no
                // accumulator in the slot can ever be needed again — release
                // them instead of waiting for round eviction.
                self.resolve_slot(tx_ref);
                return Ok(());
            }
            TransactionMessage::Shard(shard_msg) => shard_msg,
        };

        // The slot's genuine payload is already known, so this shard
        // can only re-grow an accumulator the resolution purged.
        if self.resolved_slots.contains(&(tx_ref.round, tx_ref.author)) {
            self.context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["slot_resolved"])
                .inc();
            debug!("Dropping shard for {tx_ref:?}: its slot is already resolved");
            return Ok(());
        }

        // Relaying two shards for one slot is the peer's own fault and is
        // charged; exceeding the accumulator limit is not, as others may
        // have filled the slot.
        let slot_start = TransactionRef {
            round: tx_ref.round,
            author: tx_ref.author,
            transactions_commitment: TransactionsCommitment::MIN,
        };
        let slot_end = TransactionRef {
            round: tx_ref.round,
            author: tx_ref.author,
            transactions_commitment: TransactionsCommitment::MAX,
        };
        let mut accumulators_in_slot = 0usize;
        let mut peer_in_other_accumulator = false;
        for (existing_ref, accumulator) in self.shard_accumulators.range(slot_start..=slot_end) {
            accumulators_in_slot += 1;
            if *existing_ref != tx_ref && accumulator.contains_shard_at_index(shard_msg.shard_index)
            {
                peer_in_other_accumulator = true;
            }
        }

        // One shard per (relaying peer, slot), across all accumulators: the
        // per-slot header cap leaves an honest peer holding exactly one own
        // shard per slot, so a peer whose index already appears in another
        // accumulator of the slot has relayed a shard it could not have held
        // honestly. The shard carries no relayer signature, so the evidence
        // stays local.
        if peer_in_other_accumulator {
            self.context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["peer_already_in_slot"])
                .inc();
            debug!(
                "Dropping shard for {tx_ref:?}: peer index {} already contributed a shard in this slot",
                shard_msg.shard_index
            );
            if let Some(peer) = self
                .context
                .committee
                .to_authority_index(shard_msg.shard_index)
            {
                self.misbehavior_store.record_faulty_block(
                    peer,
                    peer,
                    &ConsensusError::SecondShardForSlot {
                        peer,
                        author: tx_ref.author,
                        round: tx_ref.round,
                    },
                );
            }
            return Ok(());
        }

        let shard_index = shard_msg.shard_index;
        let occupies_new_position = match self.shard_accumulators.get(&tx_ref) {
            Some(accumulator) => !accumulator.contains_shard_at_index(shard_index),
            None => {
                // With one shard per (peer, slot), a slot holding
                // `parity_length + 1` accumulators has too few
                // uncommitted peers left for any further commitment to
                // ever gather `info_length` contributors — a new
                // accumulator would be dead weight by construction.
                let max_accumulators_per_slot = self.context.committee.parity_length() + 1;
                if accumulators_in_slot >= max_accumulators_per_slot {
                    self.context
                        .metrics
                        .node_metrics
                        .shard_reconstructor_dropped_shards
                        .with_label_values(&["slot_full"])
                        .inc();
                    debug!(
                        "Dropping shard for {tx_ref:?}: slot already holds {accumulators_in_slot} accumulators"
                    );
                    return Ok(());
                }
                true
            }
        };
        if occupies_new_position {
            self.make_room_in_peer_budget(shard_index);
            self.retained_shards_by_authority[shard_index].insert(tx_ref);
        }
        match self.shard_accumulators.entry(tx_ref) {
            Entry::Vacant(v) => {
                v.insert(ShardAccumulator::new_with_shard(shard_msg, total_length));
            }
            Entry::Occupied(mut o) => {
                o.get_mut().update_with_shard(shard_msg);
            }
        }

        // Check if we can reconstruct the block now and enqueue it if so
        Self::enqueue_if_ready(
            &mut self.shard_accumulators,
            &mut self.reconstruction_queue,
            &self.ready_to_reconstruct_sender,
            self.info_length,
            &self.context.committee,
            &tx_ref,
            &mut self.retained_shards_by_authority,
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
        committee: &Committee,
        tx_ref: &TransactionRef,
        retained_shards_by_authority: &mut [BTreeSet<TransactionRef>],
    ) -> ConsensusResult<()> {
        if let Some(acc) = accumulators.get(tx_ref) {
            if acc.is_ready_to_reconstruct(info_length, committee) {
                // take ownership out of map
                let acc = accumulators
                    .remove(tx_ref)
                    .expect("We should expect the shard accumulator to be present");
                for shard_index in acc.collected_shard_indices() {
                    retained_shards_by_authority[shard_index].remove(tx_ref);
                }
                sender
                    .send(acc)
                    .await
                    .map_err(|_| ConsensusError::AccumulatorSenderClosed)?;
                reconstruction_queue.insert(*tx_ref);
            }
        }
        Ok(())
    }

    /// At the authority's shard budget, evicts its oldest (lowest-round)
    /// retained shard to admit the new one. Whatever eviction costs stays
    /// fetchable via the transaction synchronizer, since a decodable
    /// payload's relayers hold it in full.
    fn make_room_in_peer_budget(&mut self, shard_index: usize) {
        let retained = &self.retained_shards_by_authority[shard_index];
        if retained.len() < self.shard_budget_per_authority {
            return;
        }
        let Some(victim_ref) = retained.first().copied() else {
            return;
        };
        self.retained_shards_by_authority[shard_index].remove(&victim_ref);
        if let Entry::Occupied(mut occupied) = self.shard_accumulators.entry(victim_ref) {
            occupied.get_mut().remove_shard_at_index(shard_index);
            if occupied.get().number_shards == 0 {
                occupied.remove();
            }
        }
        self.context
            .metrics
            .node_metrics
            .shard_reconstructor_dropped_shards
            .with_label_values(&["peer_budget_evicted"])
            .inc();
        debug!(
            "Evicted the oldest retained shard of peer index {shard_index} ({victim_ref:?}) to admit a new one"
        );
    }

    /// Marks the slot of `tx_ref` resolved and releases every accumulator in
    /// it. Both triggers pin an author-signed payload (a decode reached f+1
    /// relayer stake, so an honest relayer coded it from the signed payload);
    /// every other commitment in the slot is fabricated, except an
    /// equivocating twin, which the transaction synchronizer can still fetch.
    fn resolve_slot(&mut self, tx_ref: TransactionRef) {
        self.resolved_slots.insert((tx_ref.round, tx_ref.author));
        let slot_start = TransactionRef {
            round: tx_ref.round,
            author: tx_ref.author,
            transactions_commitment: TransactionsCommitment::MIN,
        };
        let slot_end = TransactionRef {
            round: tx_ref.round,
            author: tx_ref.author,
            transactions_commitment: TransactionsCommitment::MAX,
        };
        let purged_refs: Vec<TransactionRef> = self
            .shard_accumulators
            .range(slot_start..=slot_end)
            .map(|(purged_ref, _)| *purged_ref)
            .collect();
        for purged_ref in purged_refs {
            if let Some(accumulator) = self.shard_accumulators.remove(&purged_ref) {
                for shard_index in accumulator.collected_shard_indices() {
                    self.retained_shards_by_authority[shard_index].remove(&purged_ref);
                }
            }
        }
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
    use starfish_config::{AuthorityIndex, Parameters};
    use tokio::sync::{Mutex, mpsc::Sender};

    use crate::{
        BlockRef, Round, TestBlockHeader, Transaction, VerifiedBlockHeader,
        block_header::{
            CommitmentVerifiedTransactions, Shard, ShardWithProof, TransactionsCommitment,
            VerifiedBlock, VerifiedOwnShard,
        },
        block_verifier::{
            BlockVerifier, NoopBlockVerifier, SignedBlockVerifier, test::TxnSizeVerifier,
        },
        commit::CertifiedCommits,
        context::Context,
        core::ReasonToCreateBlock,
        core_thread::{CoreError, CoreThreadDispatcher},
        dag_state::{DagState, DataSource},
        encoder::{ShardEncoder, create_encoder},
        misbehavior_store::MisbehaviorCounts,
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
        dag_state: Arc<RwLock<DagState>>,
        tx: Sender<Vec<TransactionMessage>>,
    }

    impl TestHarness {
        fn new(committee_size: usize) -> Self {
            let (context, _) = Context::new_for_test(committee_size);
            Self::new_with_block_verifier(Arc::new(context), Arc::new(NoopBlockVerifier))
        }

        /// Builds the harness with a caller-supplied `block_verifier`, so
        /// tests can exercise the transaction-validity rejection path
        /// with a verifier stricter than the default no-op. `context` is
        /// taken from the caller so it can build a `block_verifier` bound to
        /// the same committee and protocol config.
        fn new_with_block_verifier(
            context: Arc<Context>,
            block_verifier: Arc<dyn BlockVerifier>,
        ) -> Self {
            let store = Arc::new(MemStore::new());
            let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
            let core_dispatcher = Arc::new(MockCoreThreadDispatcher::new());
            let handle = ShardReconstructor::start(
                context.clone(),
                dag_state.clone(),
                core_dispatcher.clone(),
                block_verifier,
            );
            let tx = handle.transaction_message_sender();
            Self {
                context,
                core_dispatcher,
                handle,
                dag_state,
                tx,
            }
        }
    }

    #[derive(Default)]
    struct MockCoreThreadDispatcher {
        transactions: Mutex<Vec<CommitmentVerifiedTransactions>>,
    }

    impl MockCoreThreadDispatcher {
        fn new() -> Self {
            Self::default()
        }

        async fn get_and_drain_transactions(&self) -> Vec<CommitmentVerifiedTransactions> {
            let mut guard = self.transactions.lock().await;
            guard.drain(..).collect()
        }
    }

    #[async_trait::async_trait]
    impl CoreThreadDispatcher for MockCoreThreadDispatcher {
        async fn add_transactions(
            &self,
            txs: Vec<CommitmentVerifiedTransactions>,
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

    /// `info_length` shards from low-stake relayers must not reconstruct;
    /// decoding starts only at validity threshold (f+1) relayer stake
    #[tokio::test]
    async fn test_reconstruction_waits_for_validity_threshold_stake() {
        telemetry_subscribers::init_for_testing();

        // GIVEN a committee of 10 where the validity threshold (34) is
        // unreachable without authority 9 (stake 91).
        let (context, _) = Context::new_for_test(10);
        let mut stakes = vec![1; 9];
        stakes.push(91);
        let (committee, _) = starfish_config::local_committee_and_keys(0, stakes);
        let context = Arc::new(context.with_committee(committee));
        let h = TestHarness::new_with_block_verifier(context.clone(), Arc::new(NoopBlockVerifier));
        let transaction_message_sender = h.tx.clone();

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

        // Shards from the info_length lowest-stake relayers: enough to decode,
        // not enough stake.
        let batch: Vec<_> = (0..info_length)
            .map(|i| {
                TransactionMessage::Shard(ShardMessage {
                    transaction_ref: TransactionRef::new(block_ref, commitment),
                    block_digest: Some(block_ref.digest),
                    shard: all_shards[i].clone(),
                    shard_index: i,
                })
            })
            .collect();
        transaction_message_sender.send(batch).await.unwrap();

        tokio::time::sleep(Duration::from_millis(400)).await;
        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        assert!(
            fetched.is_empty(),
            "info_length shards whose relayers hold less than the validity threshold stake must not reconstruct"
        );

        // WHEN the high-stake relayer's shard pushes the combined stake over
        // the validity threshold
        let high_stake_index = context.committee.size() - 1;
        transaction_message_sender
            .send(vec![TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef::new(block_ref, commitment),
                block_digest: Some(block_ref.digest),
                shard: all_shards[high_stake_index].clone(),
                shard_index: high_stake_index,
            })])
            .await
            .unwrap();

        // THEN reconstruction should happen
        tokio::time::sleep(Duration::from_millis(600)).await;
        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(
            fetched.len(),
            1,
            "Reconstruction should happen once the relayers' stake reaches the validity threshold"
        );
        assert_eq!(fetched[0].transactions(), txs);

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

        // GIVEN: a ShardWithProof whose block_ref points to shard_source.
        let shard_with_proof = ShardWithProof::new(
            vec![0u8; 32],
            vec![],
            shard_source_ref,
            shard_source.transactions_commitment(),
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

    /// Reconstructs a payload that fails verification from a full set of shards
    /// and returns the resulting misbehavior snapshot, the payload author, and
    /// the info length. When `accept_header` is set, a verified block header
    /// committing to the reconstructed payload is accepted into the dag state
    /// first, so the invalid payload is provably the author's.
    async fn reconstruct_failing_payload(
        accept_header: bool,
    ) -> (Vec<MisbehaviorCounts>, AuthorityIndex, usize) {
        telemetry_subscribers::init_for_testing();

        // GIVEN a harness whose block_verifier rejects transactions shorter
        // than 4 bytes.
        let (context, _) = Context::new_for_test(10);
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(TxnSizeVerifier {}),
        ));
        let h = TestHarness::new_with_block_verifier(context.clone(), block_verifier);
        let transaction_message_sender = h.tx.clone();

        // A 2-byte transaction fails `TxnSizeVerifier::verify_batch` (< 4 bytes).
        let txs = vec![Transaction::new(vec![0u8; 2])];
        let serialized = Transaction::serialize(&txs).unwrap();

        let mut encoder = create_encoder(&context);
        let commitment = TransactionsCommitment::compute_transactions_commitment(
            &serialized,
            &context,
            &mut encoder,
        )
        .unwrap();

        let header = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(5, 1)
                .set_commitment(commitment)
                .build(),
        );
        let block_ref = header.reference();
        let author = block_ref.author;

        // The author is charged only when a verified header ties the commitment
        // to them; accept one for that case.
        if accept_header {
            h.dag_state
                .write()
                .accept_block_header(header, DataSource::BlockBundleStream);
        }

        let info_length = context.committee.info_length();
        let parity_length = context.committee.parity_length();

        let all_shards = encoder
            .encode_serialized_data(&serialized, info_length, parity_length)
            .unwrap();

        let batch: Vec<_> = (0..info_length)
            .map(|i| {
                TransactionMessage::Shard(ShardMessage {
                    transaction_ref: TransactionRef::new(block_ref, commitment),
                    block_digest: Some(block_ref.digest),
                    shard: all_shards[i].clone(),
                    shard_index: i,
                })
            })
            .collect();

        // WHEN enough shards arrive to reconstruct the payload.
        transaction_message_sender.send(batch).await.unwrap();
        tokio::time::sleep(Duration::from_millis(600)).await;

        // THEN the payload is dropped instead of being handed to Core, so it
        // can never be acknowledged.
        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        assert!(
            fetched.is_empty(),
            "A reconstructed payload failing the validity check must never reach Core"
        );

        let counts = h.dag_state.read().misbehavior_store().snapshot_totals();
        h.handle
            .stop()
            .await
            .expect("We should expect graceful shutdown");
        (counts, author, info_length)
    }

    /// A reconstructed payload must pass the same per-transaction limit and
    /// `verify_batch` checks the direct route enforces before it can reach
    /// Core. Otherwise it could be acknowledged and become committable while
    /// diverging from nodes that received the same payload directly.
    ///
    /// Without a verified header tying the peer-supplied commitment to the
    /// author, the author must not be charged (a coalition of peers could
    /// otherwise frame them); every peer that relayed a shard is charged an
    /// unprovable fault.
    #[tokio::test]
    async fn test_reconstruction_rejects_transactions_failing_validity_check() {
        let (counts, author, info_length) = reconstruct_failing_payload(false).await;

        let author_counts = counts[author.value()].as_v2();
        assert_eq!(
            author_counts.faulty_blocks_provable, 0,
            "The author must not be charged without a verified header tying the commitment to them"
        );
        for counts in counts.iter().take(info_length) {
            let peer_counts = counts.as_v2();
            assert_eq!(
                peer_counts.faulty_blocks_unprovable, 1,
                "Each peer that relayed a shard of the invalid payload must be charged unprovably"
            );
        }
    }

    /// When a verified header commits to the reconstructed payload, the invalid
    /// transactions are provably the author's, so the author is charged a
    /// provable fault while the relaying peers are still charged unprovably.
    #[tokio::test]
    async fn test_reconstruction_charges_author_when_header_present() {
        let (counts, author, info_length) = reconstruct_failing_payload(true).await;

        let author_counts = counts[author.value()].as_v2();
        assert_eq!(
            author_counts.faulty_blocks_provable, 1,
            "The author must be charged provably when a verified header commits to the payload"
        );
        for (i, counts) in counts.iter().enumerate().take(info_length) {
            if i == author.value() {
                continue;
            }
            let peer_counts = counts.as_v2();
            assert_eq!(
                peer_counts.faulty_blocks_unprovable, 1,
                "Each relaying peer other than the author must still be charged unprovably"
            );
        }
    }

    /// A reconstructed payload whose commitment doesn't match the ref's is not
    /// provably the author's — the shards, and thus the mismatch, come from
    /// peers — so only the relaying peers are charged, even when a verified
    /// header for the ref exists.
    #[tokio::test]
    async fn test_reconstruction_charges_relayers_on_commitment_mismatch() {
        telemetry_subscribers::init_for_testing();

        let (context, _) = Context::new_for_test(10);
        let context = Arc::new(context);
        let h = TestHarness::new_with_block_verifier(context.clone(), Arc::new(NoopBlockVerifier));
        let transaction_message_sender = h.tx.clone();

        // Shards encode `txs`, but the ref commits to a different payload's
        // commitment, so the reconstructed bytes won't match it.
        let txs = vec![Transaction::new(vec![7u8; 8])];
        let serialized = Transaction::serialize(&txs).unwrap();
        let mut encoder = create_encoder(&context);
        let other = Transaction::serialize(&[Transaction::new(vec![9u8; 8])]).unwrap();
        let wrong_commitment =
            TransactionsCommitment::compute_transactions_commitment(&other, &context, &mut encoder)
                .unwrap();

        // Accept a verified header committing to that (wrong) commitment, to
        // prove the author is still not charged for a peer-produced mismatch.
        let header = VerifiedBlockHeader::new_for_test(
            TestBlockHeader::new(5, 1)
                .set_commitment(wrong_commitment)
                .build(),
        );
        let block_ref = header.reference();
        let author = block_ref.author;
        h.dag_state
            .write()
            .accept_block_header(header, DataSource::BlockBundleStream);

        let info_length = context.committee.info_length();
        let parity_length = context.committee.parity_length();
        let all_shards = encoder
            .encode_serialized_data(&serialized, info_length, parity_length)
            .unwrap();

        let batch: Vec<_> = (0..info_length)
            .map(|i| {
                TransactionMessage::Shard(ShardMessage {
                    transaction_ref: TransactionRef::new(block_ref, wrong_commitment),
                    block_digest: Some(block_ref.digest),
                    shard: all_shards[i].clone(),
                    shard_index: i,
                })
            })
            .collect();

        // WHEN enough shards arrive to reconstruct the payload.
        transaction_message_sender.send(batch).await.unwrap();
        tokio::time::sleep(Duration::from_millis(600)).await;

        // THEN the payload is dropped instead of being handed to Core.
        assert!(
            h.core_dispatcher
                .get_and_drain_transactions()
                .await
                .is_empty(),
            "A reconstructed payload failing commitment verification must never reach Core"
        );

        let counts = h.dag_state.read().misbehavior_store().snapshot_totals();
        h.handle
            .stop()
            .await
            .expect("We should expect graceful shutdown");

        // The author is not charged provably, even though a verified header ties
        // it to the (wrong) commitment.
        let author_counts = counts[author.value()].as_v2();
        assert_eq!(
            author_counts.faulty_blocks_provable, 0,
            "The author must not be charged for a peer-produced commitment mismatch"
        );
        // Every peer that relayed a shard — including the author, as a relayer —
        // is charged an unprovable fault.
        for counts in counts.iter().take(info_length) {
            let peer_counts = counts.as_v2();
            assert_eq!(
                peer_counts.faulty_blocks_unprovable, 1,
                "Each peer that relayed a shard must be charged unprovably"
            );
        }
    }

    /// Encodes a distinct payload for `slot` and returns its commitment plus
    /// the full shard set, so tests can build several commitments in one slot.
    fn encode_payload_for_slot(
        context: &Arc<Context>,
        encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
        marker: u8,
    ) -> (TransactionsCommitment, Vec<Shard>) {
        let serialized = Transaction::serialize(&[Transaction::new(vec![marker; 16])]).unwrap();
        let commitment =
            TransactionsCommitment::compute_transactions_commitment(&serialized, context, encoder)
                .unwrap();
        let shards = encoder
            .encode_serialized_data(
                &serialized,
                context.committee.info_length(),
                context.committee.parity_length(),
            )
            .unwrap();
        (commitment, shards)
    }

    /// A peer gets one shard per (author, round) slot across all accumulators:
    /// its shard for a second commitment in the slot is dropped and charged as
    /// a bundle-part fault, whether that would create a new accumulator or
    /// join one another peer created. The first commitment still reconstructs
    /// from `info_length` distinct peers.
    #[tokio::test]
    async fn test_shard_admission_one_shard_per_peer_per_slot() {
        telemetry_subscribers::init_for_testing();

        let h = TestHarness::new(10);
        let context = h.context.clone();
        let tx = h.tx.clone();
        let mut encoder = create_encoder(&context);
        let info_length = context.committee.info_length();

        let block_ref =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build()).reference();
        let (first_commitment, first_shards) = encode_payload_for_slot(&context, &mut encoder, 1);
        let (second_commitment, second_shards) = encode_payload_for_slot(&context, &mut encoder, 2);

        // Peer 0 contributes to the first commitment, creating its accumulator.
        tx.send(vec![TransactionMessage::Shard(ShardMessage {
            transaction_ref: TransactionRef::new(block_ref, first_commitment),
            block_digest: Some(block_ref.digest),
            shard: first_shards[0].clone(),
            shard_index: 0,
        })])
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Peer 0's shard for a second commitment in the same slot is dropped —
        // it would create a second accumulator.
        tx.send(vec![TransactionMessage::Shard(ShardMessage {
            transaction_ref: TransactionRef::new(block_ref, second_commitment),
            block_digest: Some(block_ref.digest),
            shard: second_shards[0].clone(),
            shard_index: 0,
        })])
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["peer_already_in_slot"])
                .get(),
            1,
        );

        // Peer 1 creates the second commitment's accumulator, then peer 0 tries
        // to join it — also dropped, this time through the occupied arm.
        tx.send(vec![TransactionMessage::Shard(ShardMessage {
            transaction_ref: TransactionRef::new(block_ref, second_commitment),
            block_digest: Some(block_ref.digest),
            shard: second_shards[1].clone(),
            shard_index: 1,
        })])
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        tx.send(vec![TransactionMessage::Shard(ShardMessage {
            transaction_ref: TransactionRef::new(block_ref, second_commitment),
            block_digest: Some(block_ref.digest),
            shard: second_shards[0].clone(),
            shard_index: 0,
        })])
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["peer_already_in_slot"])
                .get(),
            2,
        );

        // The first commitment still reconstructs: peers 2..info_length+1 are
        // free to contribute, so it reaches `info_length` shards.
        let rest: Vec<_> = (2..=info_length)
            .map(|i| {
                TransactionMessage::Shard(ShardMessage {
                    transaction_ref: TransactionRef::new(block_ref, first_commitment),
                    block_digest: Some(block_ref.digest),
                    shard: first_shards[i].clone(),
                    shard_index: i,
                })
            })
            .collect();
        tx.send(rest).await.unwrap();
        tokio::time::sleep(Duration::from_millis(600)).await;

        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(
            fetched.len(),
            1,
            "the first commitment must still reconstruct from info_length distinct peers"
        );
        assert_eq!(
            fetched[0].transaction_ref().transactions_commitment,
            first_commitment
        );

        // Only peer 0 is charged, once per dropped shard; peer 1 relayed a
        // single shard into the slot and stays clean.
        let counts = h.dag_state.read().misbehavior_store().snapshot_totals();
        assert_eq!(
            counts[0].as_v2().invalid_bundle_parts,
            2,
            "each second shard relayed into the slot must be charged to peer 0"
        );
        assert_eq!(counts[0].as_v2().faulty_blocks_unprovable, 0);
        assert_eq!(counts[1].as_v2().invalid_bundle_parts, 0);

        h.handle.stop().await.unwrap();
    }

    /// Two commitments in one slot, each backed by a disjoint set of
    /// `info_length` peers, both reconstruct: the per-peer rule never fires for
    /// peers that relay only their own single shard.
    #[tokio::test]
    async fn test_two_commitments_in_slot_from_disjoint_peers_both_reconstruct() {
        telemetry_subscribers::init_for_testing();

        let h = TestHarness::new(10);
        let context = h.context.clone();
        let tx = h.tx.clone();
        let mut encoder = create_encoder(&context);
        let info_length = context.committee.info_length();
        assert!(
            2 * info_length <= context.committee.size(),
            "the committee must be large enough for two disjoint peer sets"
        );

        let block_ref =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build()).reference();
        let (first_commitment, first_shards) = encode_payload_for_slot(&context, &mut encoder, 1);
        let (second_commitment, second_shards) = encode_payload_for_slot(&context, &mut encoder, 2);

        // Peers 0..info_length back the first commitment, peers
        // info_length..2*info_length back the second.
        let mut batch = Vec::new();
        for (i, shard) in first_shards.iter().enumerate().take(info_length) {
            batch.push(TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef::new(block_ref, first_commitment),
                block_digest: Some(block_ref.digest),
                shard: shard.clone(),
                shard_index: i,
            }));
        }
        for (i, shard) in second_shards
            .iter()
            .enumerate()
            .take(2 * info_length)
            .skip(info_length)
        {
            batch.push(TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef::new(block_ref, second_commitment),
                block_digest: Some(block_ref.digest),
                shard: shard.clone(),
                shard_index: i,
            }));
        }
        tx.send(batch).await.unwrap();
        tokio::time::sleep(Duration::from_millis(800)).await;

        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        let commitments: BTreeSet<_> = fetched
            .iter()
            .map(|vt| vt.transaction_ref().transactions_commitment)
            .collect();
        assert_eq!(
            commitments,
            BTreeSet::from([first_commitment, second_commitment]),
            "both commitments backed by disjoint peer sets must reconstruct"
        );
        assert_eq!(
            context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["peer_already_in_slot"])
                .get(),
            0,
            "honest single-shard-per-peer traffic must never be dropped"
        );
        let counts = h.dag_state.read().misbehavior_store().snapshot_totals();
        assert!(
            counts.iter().all(|c| c.as_v2().invalid_bundle_parts == 0),
            "honest single-shard-per-peer traffic must never be charged"
        );

        h.handle.stop().await.unwrap();
    }

    /// A slot admits at most `parity_length + 1` accumulators; past that any
    /// further commitment is dead weight by construction and is dropped.
    #[tokio::test]
    async fn test_shard_admission_caps_accumulators_per_slot() {
        telemetry_subscribers::init_for_testing();

        let h = TestHarness::new(10);
        let context = h.context.clone();
        let tx = h.tx.clone();
        let mut encoder = create_encoder(&context);
        let max_accumulators = context.committee.parity_length() + 1;
        assert!(max_accumulators < context.committee.size());

        let block_ref =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build()).reference();

        // Peer i creates accumulator i, one distinct commitment each, filling
        // the slot exactly to the cap.
        for peer in 0..max_accumulators {
            let (commitment, shards) = encode_payload_for_slot(&context, &mut encoder, peer as u8);
            tx.send(vec![TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef::new(block_ref, commitment),
                block_digest: Some(block_ref.digest),
                shard: shards[peer].clone(),
                shard_index: peer,
            })])
            .await
            .unwrap();
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["slot_full"])
                .get(),
            0,
            "filling the slot exactly to the cap must admit every accumulator"
        );

        // One more distinct commitment, from a peer that has not contributed to
        // the slot yet, is refused.
        let next_peer = max_accumulators;
        let (commitment, shards) = encode_payload_for_slot(&context, &mut encoder, next_peer as u8);
        tx.send(vec![TransactionMessage::Shard(ShardMessage {
            transaction_ref: TransactionRef::new(block_ref, commitment),
            block_digest: Some(block_ref.digest),
            shard: shards[next_peer].clone(),
            shard_index: next_peer,
        })])
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(
            context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["slot_full"])
                .get(),
            1,
        );

        h.handle.stop().await.unwrap();
    }

    /// The full payload arriving directly releases a partially filled
    /// accumulator for that ref instead of leaving it to round eviction.
    #[tokio::test]
    async fn test_full_transaction_releases_pending_accumulator() {
        telemetry_subscribers::init_for_testing();

        let h = TestHarness::new(10);
        let context = h.context.clone();
        let tx = h.tx.clone();
        let mut encoder = create_encoder(&context);
        let info_length = context.committee.info_length();

        let block_ref =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build()).reference();
        let (commitment, shards) = encode_payload_for_slot(&context, &mut encoder, 1);
        let transaction_ref = TransactionRef::new(block_ref, commitment);

        // Fewer than info_length shards, so the accumulator stays pending.
        let batch: Vec<_> = (0..info_length - 1)
            .map(|i| {
                TransactionMessage::Shard(ShardMessage {
                    transaction_ref,
                    block_digest: Some(block_ref.digest),
                    shard: shards[i].clone(),
                    shard_index: i,
                })
            })
            .collect();
        tx.send(batch).await.unwrap();

        // The gauge is refreshed on the eviction tick (once per second).
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(
            context.metrics.node_metrics.shard_accumulators.get(),
            1,
            "a partially filled accumulator is pending"
        );

        tx.send(vec![TransactionMessage::FullTransaction(transaction_ref)])
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(
            context.metrics.node_metrics.shard_accumulators.get(),
            0,
            "the pending accumulator is released once the full payload arrives"
        );

        h.handle.stop().await.unwrap();
    }

    /// A failed reconstruction must not leak its queue entry: the ref is
    /// dropped from `reconstruction_queue` and marked processed, so shards
    /// for it are dropped without re-accumulating (a retry would fail
    /// identically — the shards proved membership against the same
    /// commitment).
    #[tokio::test]
    async fn test_failed_reconstruction_clears_queue_and_is_not_retried() {
        telemetry_subscribers::init_for_testing();

        let h = TestHarness::new(10);
        let context = &h.context;
        let transaction_message_sender = h.tx.clone();

        // Shards encode `txs`, but the ref commits to a different payload's
        // commitment, so the decode fails the commitment recheck.
        let txs = vec![Transaction::new(vec![7u8; 8])];
        let serialized = Transaction::serialize(&txs).unwrap();
        let mut encoder = create_encoder(context);
        let other = Transaction::serialize(&[Transaction::new(vec![9u8; 8])]).unwrap();
        let wrong_commitment =
            TransactionsCommitment::compute_transactions_commitment(&other, context, &mut encoder)
                .unwrap();

        let header = VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build());
        let block_ref = header.reference();

        let info_length = context.committee.info_length();
        let parity_length = context.committee.parity_length();
        let all_shards = encoder
            .encode_serialized_data(&serialized, info_length, parity_length)
            .unwrap();

        let batch: Vec<_> = (0..info_length)
            .map(|i| {
                TransactionMessage::Shard(ShardMessage {
                    transaction_ref: TransactionRef::new(block_ref, wrong_commitment),
                    block_digest: Some(block_ref.digest),
                    shard: all_shards[i].clone(),
                    shard_index: i,
                })
            })
            .collect();

        // WHEN enough shards arrive and the decode fails.
        transaction_message_sender
            .send(batch.clone())
            .await
            .unwrap();
        // Wait past EVICTION_TIMEOUT so the gauges are refreshed.
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // THEN the queue entry is gone and the ref is marked processed.
        let metrics = &context.metrics.node_metrics;
        assert_eq!(
            metrics.reconstruction_queue.get(),
            0,
            "A failed reconstruction must not leave its ref in the queue"
        );
        assert_eq!(
            metrics.shard_reconstructor_processed_transactions.get(),
            1,
            "A failed reconstruction must mark its ref processed"
        );
        assert_eq!(metrics.reconstruction_jobs_started.get(), 1);

        // AND resending the same shards does not start another job.
        transaction_message_sender.send(batch).await.unwrap();
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_eq!(
            metrics.reconstruction_jobs_started.get(),
            1,
            "Shards for a failed ref must be dropped without re-accumulating"
        );
        assert!(
            h.core_dispatcher
                .get_and_drain_transactions()
                .await
                .is_empty(),
            "A failed reconstruction must never reach Core"
        );

        h.handle
            .stop()
            .await
            .expect("We should expect graceful shutdown");
    }

    /// Builds a reconstructor without starting its run loop, for tests that
    /// drive `handle_transaction_message` directly and inspect internal state.
    fn new_reconstructor_with_budget(
        committee_size: usize,
        shard_budget_per_authority: u32,
    ) -> (Arc<Context>, ShardReconstructor<MockCoreThreadDispatcher>) {
        let (context, _) = Context::new_for_test(committee_size);
        let context = Arc::new(context.with_parameters(Parameters {
            shard_budget_per_authority,
            ..Default::default()
        }));
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store)));
        let (reconstructor, _sender) = ShardReconstructor::new(
            context.clone(),
            dag_state,
            Arc::new(MockCoreThreadDispatcher::new()),
            Arc::new(NoopBlockVerifier),
        );
        (context, reconstructor)
    }

    /// A shard message for the (round, author) slot: `marker` selects the
    /// payload (and thus the commitment), `shard_index` the relaying peer.
    fn shard_for_slot(
        context: &Arc<Context>,
        encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
        round: Round,
        author: u8,
        marker: u8,
        shard_index: usize,
    ) -> TransactionMessage {
        let block_ref =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(round, author).build())
                .reference();
        let (commitment, shards) = encode_payload_for_slot(context, encoder, marker);
        TransactionMessage::Shard(ShardMessage {
            transaction_ref: TransactionRef::new(block_ref, commitment),
            block_digest: Some(block_ref.digest),
            shard: shards[shard_index].clone(),
            shard_index,
        })
    }

    /// At the per-authority budget, admitting a new shard evicts the
    /// authority's oldest retained one instead of dropping the new one, and
    /// other authorities' budgets are unaffected.
    #[tokio::test]
    async fn test_peer_budget_evicts_oldest_shard_to_admit_new_one() {
        telemetry_subscribers::init_for_testing();
        let (context, mut reconstructor) = new_reconstructor_with_budget(10, 2);
        let mut encoder = create_encoder(&context);
        let evicted_shards = context
            .metrics
            .node_metrics
            .shard_reconstructor_dropped_shards
            .with_label_values(&["peer_budget_evicted"]);

        // Peer 0 fills its budget with single-shard accumulators at rounds 5, 6.
        let old = shard_for_slot(&context, &mut encoder, 5, 1, 1, 0);
        let old_ref = old.transaction_ref();
        reconstructor.handle_transaction_message(old).await.unwrap();
        let kept = shard_for_slot(&context, &mut encoder, 6, 1, 2, 0);
        let kept_ref = kept.transaction_ref();
        reconstructor
            .handle_transaction_message(kept)
            .await
            .unwrap();
        assert_eq!(evicted_shards.get(), 0);

        // A further shard from peer 0 evicts the round-5 one, and the emptied
        // accumulator with it.
        let new = shard_for_slot(&context, &mut encoder, 7, 1, 3, 0);
        let new_ref = new.transaction_ref();
        reconstructor.handle_transaction_message(new).await.unwrap();
        assert!(!reconstructor.shard_accumulators.contains_key(&old_ref));
        assert!(reconstructor.shard_accumulators.contains_key(&kept_ref));
        assert!(reconstructor.shard_accumulators.contains_key(&new_ref));
        assert_eq!(
            reconstructor.retained_shards_by_authority[0],
            BTreeSet::from([kept_ref, new_ref])
        );
        assert_eq!(evicted_shards.get(), 1);

        // Peer 1 is below its own budget: its shard for the evicted slot is
        // admitted without an eviction.
        let other_peer = shard_for_slot(&context, &mut encoder, 5, 1, 1, 1);
        reconstructor
            .handle_transaction_message(other_peer)
            .await
            .unwrap();
        assert!(reconstructor.shard_accumulators.contains_key(&old_ref));
        assert_eq!(
            reconstructor.retained_shards_by_authority[1],
            BTreeSet::from([old_ref])
        );
        assert_eq!(evicted_shards.get(), 1);
    }

    /// Evicting a shard from a multi-shard accumulator removes only the
    /// evicted peer's shard; the accumulator keeps the other peers'.
    #[tokio::test]
    async fn test_peer_budget_eviction_keeps_other_peers_shards() {
        telemetry_subscribers::init_for_testing();
        let (context, mut reconstructor) = new_reconstructor_with_budget(10, 2);
        let mut encoder = create_encoder(&context);

        // Peers 0 and 1 share the round-5 accumulator; peer 0 also holds a
        // round-6 shard, filling its budget.
        let shared = shard_for_slot(&context, &mut encoder, 5, 1, 1, 0);
        let shared_ref = shared.transaction_ref();
        reconstructor
            .handle_transaction_message(shared)
            .await
            .unwrap();
        reconstructor
            .handle_transaction_message(shard_for_slot(&context, &mut encoder, 5, 1, 1, 1))
            .await
            .unwrap();
        let kept = shard_for_slot(&context, &mut encoder, 6, 1, 2, 0);
        let kept_ref = kept.transaction_ref();
        reconstructor
            .handle_transaction_message(kept)
            .await
            .unwrap();

        // Peer 0's next shard evicts its oldest — the round-5 one — while the
        // accumulator keeps peer 1's shard.
        let admitted = shard_for_slot(&context, &mut encoder, 7, 1, 3, 0);
        let admitted_ref = admitted.transaction_ref();
        reconstructor
            .handle_transaction_message(admitted)
            .await
            .unwrap();
        let shared_accumulator = reconstructor
            .shard_accumulators
            .get(&shared_ref)
            .expect("the accumulator keeps peer 1's shard");
        assert!(!shared_accumulator.contains_shard_at_index(0));
        assert!(shared_accumulator.contains_shard_at_index(1));
        assert_eq!(
            reconstructor.retained_shards_by_authority[0],
            BTreeSet::from([kept_ref, admitted_ref])
        );
        assert_eq!(
            reconstructor.retained_shards_by_authority[1],
            BTreeSet::from([shared_ref])
        );
    }

    /// The retained-shard bookkeeping empties when accumulators leave the map
    /// through reconstruction dequeue and through round eviction.
    #[tokio::test]
    async fn test_peer_budget_bookkeeping_cleared_when_accumulators_leave() {
        telemetry_subscribers::init_for_testing();
        let (context, mut reconstructor) = new_reconstructor_with_budget(10, 100);
        let mut encoder = create_encoder(&context);
        let info_length = context.committee.info_length();

        // Reconstruction dequeue: info_length shards reach the validity
        // threshold and the accumulator moves to the workers' queue.
        let block_ref =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build()).reference();
        let (commitment, shards) = encode_payload_for_slot(&context, &mut encoder, 1);
        for (i, shard) in shards.iter().enumerate().take(info_length) {
            reconstructor
                .handle_transaction_message(TransactionMessage::Shard(ShardMessage {
                    transaction_ref: TransactionRef::new(block_ref, commitment),
                    block_digest: Some(block_ref.digest),
                    shard: shard.clone(),
                    shard_index: i,
                }))
                .await
                .unwrap();
        }
        assert!(reconstructor.shard_accumulators.is_empty());
        assert!(
            reconstructor
                .retained_shards_by_authority
                .iter()
                .all(|retained| retained.is_empty())
        );

        // Round eviction: pending shards below the floor drop with their
        // bookkeeping.
        reconstructor
            .handle_transaction_message(shard_for_slot(&context, &mut encoder, 6, 1, 2, 0))
            .await
            .unwrap();
        reconstructor.evict_below(7);
        assert!(reconstructor.shard_accumulators.is_empty());
        assert!(
            reconstructor
                .retained_shards_by_authority
                .iter()
                .all(|retained| retained.is_empty())
        );
    }

    /// A directly received full payload resolves its whole slot: sibling
    /// accumulators for other commitments are purged, later shards for the
    /// slot are dropped, and the resolved slot is itself evicted by round.
    #[tokio::test]
    async fn test_full_transaction_resolves_slot_purging_and_rejecting_siblings() {
        telemetry_subscribers::init_for_testing();
        let (context, mut reconstructor) = new_reconstructor_with_budget(10, 100);
        let mut encoder = create_encoder(&context);

        let block_ref =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build()).reference();
        let (real_commitment, real_shards) = encode_payload_for_slot(&context, &mut encoder, 1);
        let real_ref = TransactionRef::new(block_ref, real_commitment);

        // A partially filled accumulator for the real commitment and a
        // fabricated sibling in the same slot.
        reconstructor
            .handle_transaction_message(TransactionMessage::Shard(ShardMessage {
                transaction_ref: real_ref,
                block_digest: Some(block_ref.digest),
                shard: real_shards[0].clone(),
                shard_index: 0,
            }))
            .await
            .unwrap();
        reconstructor
            .handle_transaction_message(shard_for_slot(&context, &mut encoder, 5, 1, 2, 1))
            .await
            .unwrap();
        assert_eq!(reconstructor.shard_accumulators.len(), 2);

        // The full payload resolves the slot: both accumulators are released.
        reconstructor
            .handle_transaction_message(TransactionMessage::FullTransaction(real_ref))
            .await
            .unwrap();
        assert!(reconstructor.shard_accumulators.is_empty());
        assert!(
            reconstructor
                .retained_shards_by_authority
                .iter()
                .all(|retained| retained.is_empty())
        );
        assert!(
            reconstructor
                .resolved_slots
                .contains(&(5, block_ref.author))
        );

        // A later shard for a third commitment in the slot opens nothing.
        reconstructor
            .handle_transaction_message(shard_for_slot(&context, &mut encoder, 5, 1, 3, 2))
            .await
            .unwrap();
        assert!(reconstructor.shard_accumulators.is_empty());
        assert_eq!(
            context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["slot_resolved"])
                .get(),
            1,
        );

        // Resolved slots below the gc floor are themselves evicted.
        reconstructor.evict_below(6);
        assert!(reconstructor.resolved_slots.is_empty());
    }

    /// A successful decode resolves the slot the same way: fabricated sibling
    /// accumulators are purged and later shards for the slot are dropped.
    #[tokio::test]
    async fn test_decode_resolves_slot_and_purges_siblings() {
        telemetry_subscribers::init_for_testing();
        let h = TestHarness::new(10);
        let context = h.context.clone();
        let tx = h.tx.clone();
        let mut encoder = create_encoder(&context);
        let info_length = context.committee.info_length();

        let block_ref =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(5, 1).build()).reference();
        let (real_commitment, real_shards) = encode_payload_for_slot(&context, &mut encoder, 1);
        let (sibling_commitment, sibling_shards) =
            encode_payload_for_slot(&context, &mut encoder, 2);

        // A fabricated sibling accumulator first, then enough real shards to
        // decode, from disjoint peers.
        let mut batch = vec![TransactionMessage::Shard(ShardMessage {
            transaction_ref: TransactionRef::new(block_ref, sibling_commitment),
            block_digest: Some(block_ref.digest),
            shard: sibling_shards[info_length].clone(),
            shard_index: info_length,
        })];
        for (i, shard) in real_shards.iter().enumerate().take(info_length) {
            batch.push(TransactionMessage::Shard(ShardMessage {
                transaction_ref: TransactionRef::new(block_ref, real_commitment),
                block_digest: Some(block_ref.digest),
                shard: shard.clone(),
                shard_index: i,
            }));
        }
        tx.send(batch).await.unwrap();

        // Wait past the eviction tick so the accumulator gauge refreshes.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert_eq!(
            h.core_dispatcher.get_and_drain_transactions().await.len(),
            1,
            "the real commitment must reconstruct"
        );
        assert_eq!(
            context.metrics.node_metrics.shard_accumulators.get(),
            0,
            "the fabricated sibling must be purged when the slot's decode succeeds"
        );

        // A later shard for yet another commitment in the resolved slot is
        // dropped without opening an accumulator.
        let (third_commitment, third_shards) = encode_payload_for_slot(&context, &mut encoder, 3);
        tx.send(vec![TransactionMessage::Shard(ShardMessage {
            transaction_ref: TransactionRef::new(block_ref, third_commitment),
            block_digest: Some(block_ref.digest),
            shard: third_shards[info_length + 1].clone(),
            shard_index: info_length + 1,
        })])
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert_eq!(
            context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["slot_resolved"])
                .get(),
            1,
        );
        assert_eq!(context.metrics.node_metrics.shard_accumulators.get(), 0);

        h.handle.stop().await.unwrap();
    }

    /// Fresh slots still reconstruct while every relaying peer sits at its
    /// budget: admission evicts the stale shard instead of dropping the new
    /// one.
    #[tokio::test]
    async fn test_reconstruction_completes_while_peers_at_budget() {
        telemetry_subscribers::init_for_testing();
        let (context, _) = Context::new_for_test(10);
        let context = Arc::new(context.with_parameters(Parameters {
            shard_budget_per_authority: 1,
            ..Default::default()
        }));
        let h = TestHarness::new_with_block_verifier(context.clone(), Arc::new(NoopBlockVerifier));
        let tx = h.tx.clone();
        let mut encoder = create_encoder(&context);
        let info_length = context.committee.info_length();

        // Each relaying peer's budget is filled by a shard stuck in its own
        // undecodable slot (a distinct author per peer, one shard each).
        let stuck: Vec<_> = (0..info_length)
            .map(|i| shard_for_slot(&context, &mut encoder, 5, (i + 2) as u8, (10 + i) as u8, i))
            .collect();
        tx.send(stuck).await.unwrap();

        // A fresh slot backed by the same peers reconstructs regardless.
        let block_ref =
            VerifiedBlockHeader::new_for_test(TestBlockHeader::new(6, 1).build()).reference();
        let (commitment, shards) = encode_payload_for_slot(&context, &mut encoder, 1);
        let batch: Vec<_> = (0..info_length)
            .map(|i| {
                TransactionMessage::Shard(ShardMessage {
                    transaction_ref: TransactionRef::new(block_ref, commitment),
                    block_digest: Some(block_ref.digest),
                    shard: shards[i].clone(),
                    shard_index: i,
                })
            })
            .collect();
        tx.send(batch).await.unwrap();

        tokio::time::sleep(Duration::from_millis(600)).await;
        let fetched = h.core_dispatcher.get_and_drain_transactions().await;
        assert_eq!(
            fetched.len(),
            1,
            "a fresh slot must reconstruct while its relayers sit at their budgets"
        );
        assert_eq!(
            fetched[0].transaction_ref().transactions_commitment,
            commitment
        );
        assert_eq!(
            context
                .metrics
                .node_metrics
                .shard_reconstructor_dropped_shards
                .with_label_values(&["peer_budget_evicted"])
                .get(),
            info_length as u64,
        );

        h.handle.stop().await.unwrap();
    }
}
