// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeSet,
    sync::{Arc, atomic::Ordering},
};

use consensus_config::AuthorityIndex;
use iota_common::scoring_metrics::VersionedScoringMetrics;
use iota_protocol_config::ProtocolConfig;
use itertools::izip;

use crate::{BlockRef, context::Context, error::ConsensusError, metrics::NodeMetrics};
/// Struct that holds the scoring metrics for all authorities in the committee,
/// both cached and uncached. It also holds a shared reference to the current
/// local metrics count used by Scorer.
pub(crate) struct MysticetiScoringMetricsStore {
    pub current_local_metrics_count: Arc<VersionedScoringMetrics>,
    pub cached_metrics: VersionedScoringMetrics,
    pub uncached_metrics: VersionedScoringMetrics,
}

impl MysticetiScoringMetricsStore {
    pub(crate) fn new(
        committee_size: usize,
        current_local_metrics_count: Arc<VersionedScoringMetrics>,
        protocol_config: &ProtocolConfig,
    ) -> Self {
        Self {
            current_local_metrics_count,
            cached_metrics: VersionedScoringMetrics::new(committee_size, protocol_config),
            uncached_metrics: VersionedScoringMetrics::new(committee_size, protocol_config),
        }
    }

    // Initializes the scoring metrics store according to the
    // recovered_scoring_metrics and blocks_in_cache_by_authority.
    pub(crate) fn initialize_scoring_metrics(
        &self,
        recovered_scoring_metrics: Option<VersionedScoringMetrics>,
        blocks_in_cache_by_authority: &Vec<BTreeSet<BlockRef>>,
        threshold_clock_round: u32,
        eviction_rounds: &Vec<u32>,
        context: &Arc<Context>,
    ) {
        // At the beginning of the epoch (i.e., before the epoch's first eviction),
        // there is nothing to be recovered from storage, so recovered_scoring_metrics
        // will be None. In this case, the initialization functions will simply
        // initialize zeroed metrics for all authorities, so we can return here.
        let Some(recovered_scoring_metrics) = recovered_scoring_metrics else {
            return;
        };

        // If recovered_scoring_metrics is not empty, we initialize metrics according to
        // them.
        match &context.protocol_config.scorer_version_as_option() {
            None | Some(1) => self.initialize_scoring_metrics_v1(
                recovered_scoring_metrics,
                blocks_in_cache_by_authority,
                threshold_clock_round,
                eviction_rounds,
                context,
            ),
            _ => panic!("Unsupported scorer version"),
        }
    }

    // Initializes the scoring metrics store according to the
    // recovered_scoring_metrics and blocks_in_cache_by_authority.
    pub(crate) fn initialize_scoring_metrics_v1(
        &self,
        recovered_scoring_metrics: VersionedScoringMetrics,
        blocks_in_cache_by_authority: &Vec<BTreeSet<BlockRef>>,
        threshold_clock_round: u32,
        eviction_rounds: &Vec<u32>,
        context: &Arc<Context>,
    ) {
        let faulty_blocks_provable = recovered_scoring_metrics.load_faulty_blocks_provable();
        let faulty_blocks_unprovable = recovered_scoring_metrics.load_faulty_blocks_unprovable();
        let missing_proposals = recovered_scoring_metrics.load_missing_proposals();
        let equivocations = recovered_scoring_metrics.load_equivocations();

        for (
            (authority_index, authority),
            blocks_in_cache,
            &eviction_round,
            &fbp,
            &fbu,
            &mp,
            &eq,
        ) in izip!(
            context.committee.authorities(),
            blocks_in_cache_by_authority,
            eviction_rounds,
            &faulty_blocks_provable,
            &faulty_blocks_unprovable,
            &missing_proposals,
            &equivocations
        ) {
            let hostname = authority.hostname.as_str();

            // Initialize the uncached scoring metrics according to
            // recovered_scoring_metrics
            self.initialize_faulty_blocks_metrics(
                fbp,
                fbu,
                hostname,
                authority_index,
                &context.metrics.node_metrics,
            );
            self.update_missing_blocks_and_equivocations(
                mp,
                eq,
                hostname,
                authority_index,
                StoreType::Uncached,
                &context.metrics.node_metrics,
            );

            // Initialize the cached scoring metrics according to blocks_in_cache.
            let block_rounds_in_cache = blocks_in_cache
                .iter()
                .map(|block_ref| block_ref.round)
                .collect();
            let (cached_equivocations, missing_blocks_in_cached_rounds) =
                calculate_scoring_metrics_for_range(
                    block_rounds_in_cache,
                    eviction_round + 1,
                    threshold_clock_round - 1,
                );
            self.update_missing_blocks_and_equivocations(
                missing_blocks_in_cached_rounds,
                cached_equivocations,
                hostname,
                authority_index,
                StoreType::Cached,
                &context.metrics.node_metrics,
            );
        }
    }

    // Updates the scoring metrics according to the received block's
    // authority and error encountered during its processing.
    pub(crate) fn update_scoring_metrics_on_block_receival(
        &self,
        authority_index: AuthorityIndex,
        hostname: &str,
        error: ConsensusError,
        source: ErrorSource,
        node_metrics: &NodeMetrics,
        protocol_config: &ProtocolConfig,
    ) {
        // authority_index will be always a valid index. However, this method will
        // panic if authority_index >= committee_size. We run this check only to avoid
        // this panic.
        if authority_index.value() >= self.cached_metrics.faulty_blocks_provable().len() {
            return;
        }

        match protocol_config.scorer_version_as_option() {
            None | Some(1) => self.update_scoring_metrics_on_block_receival_v1(
                authority_index,
                hostname,
                error,
                source,
                node_metrics,
            ),
            _ => panic!("Unsupported scorer version"),
        }
    }

    // Updates the scoring metrics according to the received block's
    // authority and error encountered during its processing.
    pub(crate) fn update_scoring_metrics_on_block_receival_v1(
        &self,
        authority_index: AuthorityIndex,
        hostname: &str,
        error: ConsensusError,
        source: ErrorSource,
        node_metrics: &NodeMetrics,
    ) {
        let source_str = source.as_str();
        let metric_type = match source {
            ErrorSource::CommitSyncer => classify_commit_syncer_error_v1(&error),
            ErrorSource::Subscriber => classify_subscriber_error_v1(&error),
            ErrorSource::Synchronizer => classify_synchronizer_error_v1(&error),
        };
        match metric_type {
            MetricType::Provable => {
                self.uncached_metrics
                    .increment_faulty_blocks_provable(authority_index.value(), 1);
                node_metrics
                    .faulty_blocks_provable_by_authority
                    .with_label_values(&[hostname, source_str, error.name()])
                    .inc();
            }
            MetricType::Unprovable => {
                self.uncached_metrics
                    .increment_faulty_blocks_unprovable(authority_index.value(), 1);
                node_metrics
                    .faulty_blocks_unprovable_by_authority
                    .with_label_values(&[hostname, source_str, error.name()])
                    .inc();
            }
            MetricType::Untracked => {
                // No scoring metrics need to be updated.
            }
        }
    }

    // Auxiliary function to initialize scoring metrics relative to faulty blocks.
    // The `authority` parameter should be a valid index, otherwise the function
    // will panic. This check is not performed here, as it is assumed that the
    // caller has already checked it.
    pub(crate) fn initialize_faulty_blocks_metrics(
        &self,
        faulty_blocks_provable: u64,
        faulty_blocks_unprovable: u64,
        hostname: &str,
        authority_index: AuthorityIndex,
        node_metrics: &NodeMetrics,
    ) {
        node_metrics
            .faulty_blocks_provable_by_authority
            .with_label_values(&[hostname, "loaded from storage", "loaded from storage"])
            .inc_by(faulty_blocks_provable);
        node_metrics
            .faulty_blocks_unprovable_by_authority
            .with_label_values(&[hostname, "loaded from storage", "loaded from storage"])
            .inc_by(faulty_blocks_unprovable);
        self.uncached_metrics
            .store_faulty_blocks_provable(authority_index.value(), faulty_blocks_provable);
        self.uncached_metrics
            .store_faulty_blocks_unprovable(authority_index.value(), faulty_blocks_unprovable);
    }

    // Auxiliary function to update scoring metrics relative to missing blocks
    // and equivocations. The `authority` parameter should be a valid index,
    // otherwise the function will panic. This check is not performed here, as
    // it is assumed that the caller has already checked it.
    pub(crate) fn update_missing_blocks_and_equivocations(
        &self,
        missing_blocks: u64,
        equivocations: u64,
        hostname: &str,
        authority: AuthorityIndex,
        metric_type: StoreType,
        node_metrics: &NodeMetrics,
    ) {
        match metric_type {
            StoreType::Cached => {
                self.cached_metrics
                    .store_equivocations(authority.value(), equivocations);
                self.cached_metrics
                    .store_missing_proposals(authority.value(), missing_blocks);
                node_metrics
                    .equivocations_in_cache_by_authority
                    .with_label_values(&[hostname])
                    .set(equivocations as i64);
                node_metrics
                    .missing_proposals_in_cache_by_authority
                    .with_label_values(&[hostname])
                    .set(missing_blocks as i64);
            }

            StoreType::Uncached => {
                self.uncached_metrics
                    .increment_equivocations(authority.value(), equivocations);
                self.uncached_metrics
                    .increment_missing_proposals(authority.value(), missing_blocks);
                node_metrics
                    .uncached_equivocations_by_authority
                    .with_label_values(&[hostname])
                    .inc_by(equivocations);
                node_metrics
                    .uncached_missing_proposals_by_authority
                    .with_label_values(&[hostname])
                    .inc_by(missing_blocks);
            }
        }
    }

    // Updates the authority's scoring metrics according to the recent changes in
    // the DAG state, i.e., recent evictions and additions to cache. It also
    // updates the current local metrics count used by Scorer. authority_index
    // should be a valid AuthorityIndex, otherwise this function will panic.
    pub(crate) fn update_scoring_metrics_on_eviction(
        &self,
        authority_index: AuthorityIndex,
        hostname: &str,
        recent_refs: &BTreeSet<BlockRef>,
        eviction_round: u32,
        last_eviction_round: u32,
        threshold_clock_round: u32,
        context: &Arc<Context>,
    ) {
        let node_metrics = &context.metrics.node_metrics;
        // threshold_clock_round should be always at least 1.
        if threshold_clock_round == 0 {
            return;
        }

        // Get the blocks rounds that were not evicted.
        let cached_block_rounds = recent_refs
            .iter()
            .map(|block| block.round)
            .filter(|&round| round > eviction_round && round < threshold_clock_round)
            .collect::<Vec<u32>>();

        // Update metrics according to the blocks from rounds still in cache.
        let (cached_equivocations, missing_blocks_in_cached_rounds) =
            calculate_scoring_metrics_for_range(
                cached_block_rounds,
                eviction_round + 1,
                threshold_clock_round.saturating_sub(1),
            );

        self.update_missing_blocks_and_equivocations(
            missing_blocks_in_cached_rounds,
            cached_equivocations,
            hostname,
            authority_index,
            StoreType::Cached,
            node_metrics,
        );

        // If no eviction happened, we do not update the current local metrics counts
        // and the metrics on storage.
        if eviction_round == last_eviction_round {
            return;
        }

        // Get the evicted blocks rounds.
        let evicted_block_rounds = recent_refs
            .iter()
            .map(|block| block.round)
            .filter(|&round| round <= eviction_round)
            .collect::<Vec<u32>>();

        // Calculate metrics according to the blocks from evicted rounds.
        let (evicted_equivocations, missing_blocks_in_evicted_rounds) =
            calculate_scoring_metrics_for_range(
                evicted_block_rounds,
                last_eviction_round + 1,
                eviction_round,
            );

        // Update metrics according to the blocks from evicted rounds.
        self.update_missing_blocks_and_equivocations(
            missing_blocks_in_evicted_rounds,
            evicted_equivocations,
            hostname,
            authority_index,
            StoreType::Uncached,
            node_metrics,
        );

        // Update current local metrics count according to the uncached metrics at this
        // point in time. Note that this will update the counts for the metrics
        // calculated above, but also for the other uncached metrics that were changed
        // since the last eviction (e.g., faulty blocks metrics updated on block
        // receival).
        self.update_current_local_metrics_count(authority_index);
    }

    // The `authority_index` should be a valid index, otherwise the function will
    // panic. This check is not performed here, as it is assumed that the caller has
    // already checked it.
    pub(crate) fn update_current_local_metrics_count(&self, authority_index: AuthorityIndex) {
        let uncached_metrics = &self
            .uncached_metrics
            .iter()
            .map(|metric_vec| metric_vec[authority_index].load(Ordering::Relaxed))
            .collect::<Vec<u64>>();
        self.current_local_metrics_count
            .iter()
            .zip(uncached_metrics)
            .for_each(|(local_metric_vec, uncached_metric)| {
                local_metric_vec[authority_index].store(*uncached_metric, Ordering::Relaxed);
            });
    }
}

// Given the set of blocks issued by an authority in rounds in the inclusive
// range [start, end], this function calculates and returns the number of
// equivocations and missing blocks in that range . The function should receive
// the vector with the rounds of such blocks and the range start and end points.
fn calculate_scoring_metrics_for_range(
    mut block_rounds: Vec<u32>,
    start: u32,
    end: u32,
) -> (u64, u64) {
    // Filter out rounds that are not in the range [start, end].
    block_rounds.retain(|&round| round >= start && round <= end);
    let number_of_blocks = block_rounds.len();
    block_rounds.dedup();
    let unique_block_rounds = block_rounds.len();
    // We use saturating_sub to avoid unexpected underflows, but the subtractions
    // below should never result in negative values by construction:
    // 1) unique_block_rounds <= number_of_blocks
    // 2) end - start + 1 >= unique_block_rounds
    let number_of_equivocations = number_of_blocks.saturating_sub(unique_block_rounds) as u64;
    let number_of_missing_blocks =
        (end + 1).saturating_sub(start + unique_block_rounds as u32) as u64;

    (number_of_equivocations, number_of_missing_blocks)
}

pub(crate) enum StoreType {
    Cached,
    Uncached,
}

#[derive(PartialEq)]
// Enum to classify errors into provable, unprovable, or untracked metrics.
// Provable metrics are those that can be proven to a third party by providing
// some cryptographic proof, such the signed block itself. Untracked metrics
// are those that are not of interest for scoring.
pub(crate) enum MetricType {
    Provable,
    Unprovable,
    Untracked,
}

// Classifies errors returned by the commit syncer as unprovable, and errors not
// returned by it as untracked. We do not classify any error as provable here
// because we cannot prove to a third party that a block or commit was fetched
// from a particular authority.
fn classify_commit_syncer_error_v1(error: &ConsensusError) -> MetricType {
    match error {
        ConsensusError::MalformedCommit(_) => MetricType::Unprovable,
        ConsensusError::UnexpectedStartCommit { .. } => MetricType::Unprovable,
        ConsensusError::UnexpectedCommitSequence { .. } => MetricType::Unprovable,
        ConsensusError::NoCommitReceived { .. } => MetricType::Unprovable,
        ConsensusError::MalformedBlock(_) => MetricType::Unprovable,
        ConsensusError::NotEnoughCommitVotes { .. } => MetricType::Unprovable,
        ConsensusError::UnexpectedNumberOfBlocksFetched { .. } => MetricType::Unprovable,
        ConsensusError::UnexpectedBlockForCommit { .. } => MetricType::Unprovable,
        // Overwrite block verifier classification to return unprovable.
        error => match classify_block_verifier_error_v1(error) {
            MetricType::Provable => MetricType::Unprovable,
            metric_type => metric_type,
        },
    }
}

// Classifies errors returned by the block verifier into provable or unprovable,
// and errors not returned by it as untracked. Errors classified as provable are
// those that can be proven to a third party by providing the signed faulty
// block itself
fn classify_block_verifier_error_v1(error: &ConsensusError) -> MetricType {
    match error {
        ConsensusError::WrongEpoch { .. } => MetricType::Unprovable,
        ConsensusError::UnexpectedGenesisBlock => MetricType::Unprovable,
        ConsensusError::InvalidAuthorityIndex { .. } => MetricType::Unprovable,
        ConsensusError::SerializationFailure(_) => MetricType::Unprovable,
        ConsensusError::MalformedSignature(_) => MetricType::Unprovable,
        ConsensusError::SignatureVerificationFailure { .. } => MetricType::Unprovable,
        // Signed block verification
        ConsensusError::TooManyAncestors { .. } => MetricType::Provable,
        ConsensusError::InsufficientParentStakes { .. } => MetricType::Provable,
        ConsensusError::InvalidAncestorAuthorityIndex { .. } => MetricType::Provable,
        ConsensusError::InvalidAncestorPosition { .. } => MetricType::Provable,
        ConsensusError::InvalidAncestorRound { .. } => MetricType::Provable,
        ConsensusError::InvalidGenesisAncestor { .. } => MetricType::Provable,
        ConsensusError::DuplicatedAncestorsAuthority { .. } => MetricType::Provable,
        ConsensusError::TransactionTooLarge { .. } => MetricType::Provable,
        ConsensusError::TooManyTransactions { .. } => MetricType::Provable,
        ConsensusError::TooManyTransactionBytes { .. } => MetricType::Provable,
        ConsensusError::InvalidTransaction { .. } => MetricType::Provable,
        _ => MetricType::Untracked,
    }
}

// Classifies errors returned by the subscriber into provable or unprovable, and
// errors not returned by it as untracked. Errors classified as provable are
// those that can be proven to a third party by providing the signed faulty
// block itself. Obs: BlockRejected errors are untracked because even though
// the rejected block signature can be verified, the reason for the rejection
// is not objective nor clearly the block author's fault.
fn classify_subscriber_error_v1(error: &ConsensusError) -> MetricType {
    match error {
        ConsensusError::MalformedBlock { .. } => MetricType::Unprovable,
        ConsensusError::UnexpectedAuthority(..) => MetricType::Unprovable,
        ConsensusError::BlockRejected { .. } => MetricType::Untracked,
        error => classify_block_verifier_error_v1(error),
    }
}

// Classifies errors returned by the synchronizer as unprovable, and errors not
// returned by it as untracked. We do not classify any error as provable here
// because we cannot prove to a third party that a block was fetched from a
// particular authority.
fn classify_synchronizer_error_v1(error: &ConsensusError) -> MetricType {
    match error {
        ConsensusError::TooManyFetchedBlocksReturned { .. } => MetricType::Unprovable,
        ConsensusError::MalformedBlock { .. } => MetricType::Unprovable,
        ConsensusError::UnexpectedFetchedBlock { .. } => MetricType::Unprovable,
        // Overwrite block verifier classification to return unprovable.
        error => match classify_block_verifier_error_v1(error) {
            MetricType::Provable => MetricType::Unprovable,
            metric_type => metric_type,
        },
    }
}

#[derive(Clone)]
pub(crate) enum ErrorSource {
    // Errors from the fetch loop, returned from fetch_once.
    CommitSyncer,
    // Errors from the subscription loop, returned from handle_send_block.
    Subscriber,
    // Errors returned from process_fetched_blocks.
    Synchronizer,
}

impl ErrorSource {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            ErrorSource::CommitSyncer => "fetch_once",
            ErrorSource::Subscriber => "handle_send_block",
            ErrorSource::Synchronizer => "process_fetched_blocks",
        }
    }
}

#[cfg(test)]
impl MysticetiScoringMetricsStore {
    // Creates a dummy scoring metrics store for testing purposes (i.e., without any
    // connection to a Scorer)
    pub(crate) fn dummy_for_test(committee_size: usize, protocol_config: &ProtocolConfig) -> Self {
        let current_local_metrics_count = Arc::new(VersionedScoringMetrics::new(
            committee_size,
            protocol_config,
        ));
        MysticetiScoringMetricsStore::new(
            committee_size,
            current_local_metrics_count,
            protocol_config,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, sync::Arc, vec};

    use consensus_config::{AuthorityIndex, NetworkKeyPair, ProtocolKeyPair};
    use parking_lot::RwLock;
    use tokio::sync::broadcast;

    use crate::{
        TransactionVerifier, ValidationError,
        authority_service::{
            AuthorityService,
            tests::{FakeCoreThreadDispatcher, FakeNetworkClient},
        },
        block::{BlockDigest, BlockRef},
        block_verifier::SignedBlockVerifier,
        commit_vote_monitor::CommitVoteMonitor,
        context::Context,
        dag_state::DagState,
        error::ConsensusError,
        scoring_metrics_store::{ErrorSource, MysticetiScoringMetricsStore},
        storage::mem_store::MemStore,
        synchronizer::Synchronizer,
        test_dag_builder::DagBuilder,
    };

    struct TxnSizeVerifier {}

    impl TransactionVerifier for TxnSizeVerifier {
        fn verify_batch(&self, _transactions: &[&[u8]]) -> Result<(), ValidationError> {
            unimplemented!("Unimplemented")
        }
    }

    // Creates a new authority service for scoring metrics testing purposes.
    fn new_authority_service_for_metrics_tests(
        committee_size: usize,
    ) -> (
        Vec<(NetworkKeyPair, ProtocolKeyPair)>,
        Arc<Context>,
        Arc<FakeCoreThreadDispatcher>,
        Arc<AuthorityService<FakeCoreThreadDispatcher>>,
    ) {
        let (context, keys) = Context::new_for_test(committee_size);
        let context = Arc::new(context);
        let block_verifier = Arc::new(SignedBlockVerifier::new(
            context.clone(),
            Arc::new(TxnSizeVerifier {}),
        ));
        let commit_vote_monitor = Arc::new(CommitVoteMonitor::new(context.clone()));
        let core_dispatcher = Arc::new(FakeCoreThreadDispatcher::new());
        let (_tx_block_broadcast, rx_block_broadcast) = broadcast::channel(100);
        let network_client = Arc::new(FakeNetworkClient::default());
        let store = Arc::new(MemStore::new());
        let dag_state = Arc::new(RwLock::new(DagState::new(context.clone(), store.clone())));
        let synchronizer = Synchronizer::start(
            network_client,
            context.clone(),
            core_dispatcher.clone(),
            commit_vote_monitor.clone(),
            block_verifier.clone(),
            dag_state.clone(),
            false,
        );
        let authority_service = Arc::new(AuthorityService::new(
            context.clone(),
            block_verifier,
            commit_vote_monitor,
            synchronizer,
            core_dispatcher.clone(),
            rx_block_broadcast,
            dag_state,
            store,
        ));
        (keys, context, core_dispatcher, authority_service)
    }

    impl MysticetiScoringMetricsStore {
        pub(crate) fn uncached_missing_proposals_by_authority(&self) -> Vec<u64> {
            self.uncached_metrics.load_missing_proposals()
        }

        pub(crate) fn equivocations_in_cache_by_authority(&self) -> Vec<u64> {
            self.cached_metrics.load_equivocations()
        }

        pub(crate) fn missing_proposals_in_cache_by_authority(&self) -> Vec<u64> {
            self.cached_metrics.load_missing_proposals()
        }

        pub(crate) fn uncached_equivocations_by_authority(&self) -> Vec<u64> {
            self.uncached_metrics.load_equivocations()
        }

        pub(crate) fn faulty_blocks_provable_by_authority(&self) -> Vec<u64> {
            self.uncached_metrics.load_faulty_blocks_provable()
        }

        pub(crate) fn faulty_blocks_unprovable_by_authority(&self) -> Vec<u64> {
            self.uncached_metrics.load_faulty_blocks_unprovable()
        }
    }

    fn read_counter_metric(context: &Arc<Context>, metric: &prometheus::IntCounterVec) -> Vec<u64> {
        context
            .committee
            .authorities()
            .map(|(_, authority)| {
                metric
                    .get_metric_with_label_values(&[authority.hostname.as_str()])
                    .unwrap()
                    .get()
            })
            .collect()
    }

    fn read_gauge_metric(context: &Arc<Context>, metric: &prometheus::IntGaugeVec) -> Vec<u64> {
        context
            .committee
            .authorities()
            .map(|(_, authority)| {
                metric
                    .get_metric_with_label_values(&[authority.hostname.as_str()])
                    .unwrap()
                    .get()
                    .unsigned_abs()
            })
            .collect()
    }

    fn read_faulty_blocks_metric(
        context: &Arc<Context>,
        metric: &prometheus::IntCounterVec,
        source: &ErrorSource,
        error: &str,
    ) -> Vec<u64> {
        let source_str = source.as_str();
        context
            .committee
            .authorities()
            .map(|(_, authority)| {
                metric
                    .get_metric_with_label_values(&[authority.hostname.as_str(), source_str, error])
                    .unwrap()
                    .get()
            })
            .collect()
    }

    fn get_uncached_missing_proposals(context: &Arc<Context>) -> Vec<u64> {
        read_counter_metric(
            context,
            &context
                .metrics
                .node_metrics
                .uncached_missing_proposals_by_authority,
        )
    }

    fn get_missing_proposals_in_cache(context: &Arc<Context>) -> Vec<u64> {
        read_gauge_metric(
            context,
            &context
                .metrics
                .node_metrics
                .missing_proposals_in_cache_by_authority,
        )
    }

    fn get_uncached_equivocations(context: &Arc<Context>) -> Vec<u64> {
        read_counter_metric(
            context,
            &context
                .metrics
                .node_metrics
                .uncached_equivocations_by_authority,
        )
    }

    fn get_equivocations_in_cache(context: &Arc<Context>) -> Vec<u64> {
        read_gauge_metric(
            context,
            &context
                .metrics
                .node_metrics
                .equivocations_in_cache_by_authority,
        )
    }

    fn is_all_zeroes(metrics: &[u64]) -> bool {
        metrics.iter().all(|&v| v == 0)
    }

    fn get_faulty_blocks_provable(
        context: &Arc<Context>,
        source: &ErrorSource,
        error: &str,
    ) -> Vec<u64> {
        read_faulty_blocks_metric(
            context,
            &context
                .metrics
                .node_metrics
                .faulty_blocks_provable_by_authority,
            source,
            error,
        )
    }

    fn get_faulty_blocks_unprovable(
        context: &Arc<Context>,
        source: &ErrorSource,
        error: &str,
    ) -> Vec<u64> {
        read_faulty_blocks_metric(
            context,
            &context
                .metrics
                .node_metrics
                .faulty_blocks_unprovable_by_authority,
            source,
            error,
        )
    }

    #[tokio::test]
    async fn test_update_scoring_metrics_on_eviction_edge_cases() {
        let context = Arc::new(Context::new_for_test(4).0);
        let scoring_metrics_store = context.scoring_metrics_store.clone();
        let authority_index = AuthorityIndex::new_for_test(0);
        let hostname = "test_host";
        let recent_refs_by_authority = BTreeSet::new();
        // Test different unexpected combinations of eviction_round, last_evicted_round,
        // and threshold_clock_round. Since recent_refs_by_authority is empty, the
        // function should never panic or return more than zero equivocations.
        // Each of the cases below have a small explanation of why they are unexpected
        // and why they are supposed to return what they return.

        // Unexpected because: threshold_clock_round = last_evicted_round means that a
        // round with blocks from less than 2f+1 stake was evicted.
        // Uncached metrics unchanged (all zeros) because nothing is evicted.
        let last_evicted_round = 5;
        let eviction_round = 5;
        let threshold_clock_round = 5;
        scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert!(is_all_zeroes(
            &scoring_metrics_store.uncached_equivocations_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_provable_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_unprovable_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.uncached_missing_proposals_by_authority()
        ));

        // Unexpected because: threshold_clock_round = 0 means that genesis is missing.
        // Uncached metrics unchanged (all zeros) because of early return.
        let last_evicted_round = 0;
        let eviction_round = 0;
        let threshold_clock_round = 0;
        scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert!(is_all_zeroes(
            &scoring_metrics_store.uncached_equivocations_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_provable_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_unprovable_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.uncached_missing_proposals_by_authority()
        ));

        // Unexpected because: threshold_clock_round < eviction_round means that a
        // round with blocks from less than 2f+1 stake is being evicted.
        // Uncached: 3 missing proposals for authority 0, from rounds 1 to 3.
        let last_evicted_round = 0;
        let eviction_round = 3;
        let threshold_clock_round = 2;
        scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert_eq!(
            scoring_metrics_store.uncached_missing_proposals_by_authority(),
            vec![3, 0, 0, 0]
        );
        assert!(is_all_zeroes(
            &scoring_metrics_store.uncached_equivocations_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_provable_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_unprovable_by_authority()
        ));

        // Unexpected because: eviction_round < last_evicted_round means that blocks
        // below or in last_evicted_round were accepted.
        // Uncached metrics unchanged because evicted range [2, 0] is inverted.
        let last_evicted_round = 1;
        let eviction_round = 0;
        let threshold_clock_round = 2;
        scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert_eq!(
            scoring_metrics_store.uncached_missing_proposals_by_authority(),
            vec![3, 0, 0, 0]
        );
        assert!(is_all_zeroes(
            &scoring_metrics_store.uncached_equivocations_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_provable_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_unprovable_by_authority()
        ));

        // Unexpected because: threshold_clock_round < eviction_round <
        // last_evicted_round and threshold_clock_round.
        // Uncached metrics unchanged because evicted range [3, 0] is inverted.
        let last_evicted_round = 2;
        let eviction_round = 0;
        let threshold_clock_round = 1;
        scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert_eq!(
            scoring_metrics_store.uncached_missing_proposals_by_authority(),
            vec![3, 0, 0, 0]
        );
        assert!(is_all_zeroes(
            &scoring_metrics_store.uncached_equivocations_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_provable_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_unprovable_by_authority()
        ));

        // Unexpected because: threshold_clock_round < last_evicted_round means that a
        // round with blocks from less than 2f+1 stake was evicted.
        // Uncached metrics unchanged because threshold_clock_round = 0 causes
        // early return.
        let last_evicted_round = 1;
        let eviction_round = 2;
        let threshold_clock_round = 0;
        scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert_eq!(
            scoring_metrics_store.uncached_missing_proposals_by_authority(),
            vec![3, 0, 0, 0]
        );
        assert!(is_all_zeroes(
            &scoring_metrics_store.uncached_equivocations_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_provable_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_unprovable_by_authority()
        ));

        // Same as above but with eviction_round < last_evicted_round.
        let last_evicted_round = 2;
        let eviction_round = 1;
        let threshold_clock_round = 0;
        scoring_metrics_store.update_scoring_metrics_on_eviction(
            authority_index,
            hostname,
            &recent_refs_by_authority,
            eviction_round,
            last_evicted_round,
            threshold_clock_round,
            &context,
        );
        assert_eq!(
            scoring_metrics_store.uncached_missing_proposals_by_authority(),
            vec![3, 0, 0, 0]
        );
        assert!(is_all_zeroes(
            &scoring_metrics_store.uncached_equivocations_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_provable_by_authority()
        ));
        assert!(is_all_zeroes(
            &scoring_metrics_store.faulty_blocks_unprovable_by_authority()
        ));
    }

    #[tokio::test]
    async fn test_metrics_flush_and_recovery_gc_enabled() {
        telemetry_subscribers::init_for_testing();

        const GC_DEPTH: u32 = 3;
        const CACHED_ROUNDS: u32 = 4;

        let committee_size = 4;
        let (mut context, _) = Context::new_for_test(committee_size);

        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;
        context
            .protocol_config
            .set_consensus_gc_depth_for_testing(GC_DEPTH);
        context
            .protocol_config
            .set_consensus_linearize_subdag_v2_for_testing(true);

        let context = Arc::new(context);
        let hostnames: Vec<&str> = context
            .committee
            .authorities()
            .map(|a| a.1.hostname.as_str())
            .collect();
        let scoring_metrics = &context.scoring_metrics_store;
        let node_metrics = &context.metrics.node_metrics;
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Initialize the DAG builder with 20 layers. Blocks in the DAG will reference
        // all blocks from the previous round.
        // - Rounds 1 to 5 will have unique blocks from all authorities.
        // - Rounds 6 to 8 will have unique blocks from all authorities, except 0, who
        //   will not propose any block.
        // - Rounds 9 to 10 will have unique blocks from all authorities.
        // - Rounds 11 to 20 will have unique blocks from all authorities, except:
        //      - Authority 1, who will produce 1 equivocating blocks at round 11 (i.e.,
        //        1+1 blocks)
        //      - Authority 2, who will produce 2 equivocating blocks at round 13 (i.e.,
        //        1+2 blocks)
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=5).build();
        dag_builder
            .layers(6..=8)
            .authorities(vec![AuthorityIndex::new_for_test(0)])
            .skip_block()
            .build();
        dag_builder.layers(9..=10).build();
        dag_builder
            .layers(11..=11)
            .authorities(vec![AuthorityIndex::new_for_test(1)])
            .equivocate(1)
            .build();
        dag_builder.layers(12..=12).build();
        dag_builder
            .layers(13..=13)
            .authorities(vec![AuthorityIndex::new_for_test(2)])
            .equivocate(2)
            .build();
        dag_builder.layers(14..=20).build();

        let mut commits = dag_builder
            .get_sub_dag_and_commits(1..=20)
            .into_iter()
            .map(|(_subdag, commit)| commit)
            .collect::<Vec<_>>();

        // Add the blocks and commits from first 10 rounds to the dag state. Since
        // authority 0 skipped a leader round, we use the 9 first items of the commits
        // vector
        let mut temp_commits = commits.split_off(9);
        dag_state.accept_blocks(dag_builder.blocks(1..=10));
        for commit in commits.clone() {
            dag_state.add_commit(commit);
        }

        // Checks that metrics are still all zeroed, since even though we accepted
        // blocks to the dag state, the metrics updates are done when the dag state is
        // flushed.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Flush the dag state
        dag_state.flush();

        // Check that metrics were updated correctly after flushing.
        //
        // Equivocations:
        // - We only accepted blocks from rounds <= 10, thus, no equivocations were
        //   accepted yet. Equivocations metrics, then, should be still all zeroed.
        //
        // Missing proposals:
        // - The last_commit_round should be 10, so gc_round should be 6. The eviction
        //   round, then, should be 6 for all authorities.
        // - The threshold_clock_round should be 11, since we already accepted all
        //   blocks from epoch 10.
        // - Then, finally, we should have counted:
        //      - 1 uncached missing proposal for authority 0;
        //      - 2 missing proposal in cache for authority 0;
        //      - 0 missing proposals for authorities 1, 2, and 3.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0; committee_size],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0],
            ]
        );

        // Clear and check all metrics
        scoring_metrics.uncached_metrics.reset();
        scoring_metrics.cached_metrics.reset();
        node_metrics
            .uncached_missing_proposals_by_authority
            .with_label_values(&[hostnames[0]])
            .reset();
        node_metrics
            .missing_proposals_in_cache_by_authority
            .with_label_values(&[hostnames[0]])
            .set(0);
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Destroy and recover dag state from storage.
        drop(dag_state);
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Metrics should have been initialized as before the recovery.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0; committee_size],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0]
            ]
        );

        // Add blocks and commits from rounds 11 and 12 to the dag state.
        let second_temp_commits = temp_commits.split_off(2);
        dag_state.accept_blocks(dag_builder.blocks(11..=12));
        for commit in temp_commits.clone() {
            dag_state.add_commit(commit);
        }

        // Flush the dag state
        dag_state.flush();

        // Check that metrics were updated correctly after flushing.
        //
        // Missing proposals:
        // - The last_commit_round should be 12, so gc_round should be 8. The eviction
        //   round, then, should be 8 for all authorities. Then, we should have counted:
        //      - 3 missing proposal in cache for authority 0;
        //      - 0 missing proposals for authorities 1, 2, and 3.
        // Equivocations:
        // - We only removed from cache blocks from rounds <= 8, thus, no equivocations
        //   should be uncached. Then, we should have counted:
        //      - 0 uncached equivocations;
        //      - 1 equivocation in cache for authority 1;
        //      - 0 equivocations in cache for authorities 0, 2 and 3;
        //

        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![3, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![3, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0; committee_size],
            ]
        );

        // Accept all the rest of blocks and commits.
        dag_state.accept_blocks(dag_builder.blocks(13..=20));
        for commit in second_temp_commits.clone() {
            dag_state.add_commit(commit);
        }

        // Clear and check all metrics
        scoring_metrics.uncached_metrics.reset();
        scoring_metrics.cached_metrics.reset();
        node_metrics
            .uncached_missing_proposals_by_authority
            .with_label_values(&[hostnames[0]])
            .reset();
        node_metrics
            .equivocations_in_cache_by_authority
            .with_label_values(&[hostnames[1]])
            .set(0);

        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Destroy and recover dag state from storage.
        drop(dag_state);
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Since the last accepted blocks were not flushed, the equivocations from
        // rounds 13 to 20 should not be accounted for. The metrics should remain
        // the same as before this acceptance.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![3, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![3, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![0; committee_size],
            ]
        );

        // Now we accept those lost blocks again and flush the dag state
        dag_state.accept_blocks(dag_builder.blocks(13..=20));
        for commit in second_temp_commits.clone() {
            dag_state.add_commit(commit);
        }
        dag_state.flush();

        // Now all misbehaviors should be accounted for in the uncached metrics.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context),
            ],
            [
                vec![0, 1, 2, 0],
                vec![3, 0, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0, 1, 2, 0],
                vec![3, 0, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
            ]
        );
    }

    #[tokio::test]
    async fn test_metrics_flush_and_recovery() {
        telemetry_subscribers::init_for_testing();

        const GC_DEPTH: u32 = 0;
        const CACHED_ROUNDS: u32 = 5;

        let committee_size = 4;
        let (mut context, _) = Context::new_for_test(committee_size);

        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;
        context
            .protocol_config
            .set_consensus_gc_depth_for_testing(GC_DEPTH);
        context
            .protocol_config
            .set_consensus_linearize_subdag_v2_for_testing(false);
        context
            .protocol_config
            .set_consensus_median_timestamp_with_checkpoint_enforcement_for_testing(false);

        let context = Arc::new(context);
        let hostnames: Vec<&str> = context
            .committee
            .authorities()
            .map(|a| a.1.hostname.as_str())
            .collect();
        let scoring_metrics = &context.scoring_metrics_store;
        let node_metrics = &context.metrics.node_metrics;

        let store = Arc::new(MemStore::new());
        // `cached_rounds` is initialized here as 5.
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Initialize the DAG builder with 20 layers. Blocks in the DAG will reference
        // all blocks from the previous round.
        // - Rounds 1 to 5 will have unique blocks from all authorities.
        // - Rounds 6 to 8 will have unique blocks from all authorities, except 0, who
        //   will not propose any block.
        // - Rounds 9 to 10 will have unique blocks from all authorities.
        // - Rounds 11 to 20 will have unique blocks from all authorities, except:
        //      - Authority 1, who will produce 1 equivocating blocks at round 11 (i.e.,
        //        1+1 blocks)
        //      - Authority 2, who will produce 2 equivocating blocks at round 13 (i.e.,
        //        1+2 blocks)
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=5).build();
        dag_builder
            .layers(6..=8)
            .authorities(vec![AuthorityIndex::new_for_test(0)])
            .skip_block()
            .build();
        dag_builder.layers(9..=10).build();
        dag_builder
            .layers(11..=11)
            .authorities(vec![AuthorityIndex::new_for_test(1)])
            .equivocate(1)
            .build();
        dag_builder.layers(12..=12).build();
        dag_builder
            .layers(13..=13)
            .authorities(vec![AuthorityIndex::new_for_test(2)])
            .equivocate(2)
            .build();
        dag_builder.layers(14..=20).build();

        let mut commits = dag_builder
            .get_sub_dag_and_commits(1..=20)
            .into_iter()
            .map(|(_subdag, commit)| commit)
            .collect::<Vec<_>>();

        // Add the blocks and commits from first 10 rounds to the dag state. Since
        // authority 0 skipped a leader round, we use the 9 first items of the commits
        // vector
        let mut temp_commits = commits.split_off(9);
        dag_state.accept_blocks(dag_builder.blocks(1..=10));
        for commit in commits.clone() {
            dag_state.add_commit(commit);
        }

        // Checks that metrics are still all zeroed, since even though we accepted
        // blocks to the dag state, the metrics updates are done when the dag state is
        // flushed.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Flush the dag state
        dag_state.flush();

        // Check that metrics were updated correctly after flushing.
        //
        // Equivocations:
        // - We only accepted blocks from rounds <= 10, thus, no equivocations were
        //   accepted yet. Equivocations metrics, then, should be still all zeroed.
        //
        // Missing proposals:
        // - The last committed round is 10, so the eviction round should be 5 for
        //   authority 2 (leader of round 10) and 4 for all other authorities.
        // - The threshold_clock_round should be 11, since we already accepted all
        //   blocks from epoch 10.
        // - Then, finally, we should have counted:
        //      - 0 uncached missing proposals for authority 0;
        //      - 3 missing proposal in cache for authority 0;
        //      - 0 missing proposals for authorities 1, 2, and 3.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![3, 0, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![3, 0, 0, 0],
            ]
        );

        // Clear and check all metrics
        scoring_metrics.uncached_metrics.reset();
        scoring_metrics.cached_metrics.reset();
        node_metrics
            .uncached_missing_proposals_by_authority
            .with_label_values(&[hostnames[0]])
            .reset();
        node_metrics
            .missing_proposals_in_cache_by_authority
            .with_label_values(&[hostnames[0]])
            .set(0);
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Destroy and recover dag state from storage.
        drop(dag_state);
        let mut dag_state = DagState::new(context.clone(), store.clone());

        assert_eq!(dag_state.last_commit_index(), 9);
        assert_eq!(dag_state.last_committed_rounds(), [9, 9, 10, 9]);

        // Metrics should have been initialized as before the recovery.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority()
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![3, 0, 0, 0],
            ]
        );

        // Add blocks and commits from rounds 11 and 12 to the dag state.
        let second_temp_commits = temp_commits.split_off(2);
        dag_state.accept_blocks(dag_builder.blocks(11..=12));
        for commit in temp_commits.clone() {
            dag_state.add_commit(commit);
        }

        // Flush the dag state
        dag_state.flush();

        // Check that metrics were updated correctly after flushing.
        //
        // Missing proposals:
        // - The last commit round is 12, so the eviction round should be 7 for
        //   authority 0 (leader of round 12) and 6 for all other authorities. Then, we
        //   should have counted:
        //      - 2 uncached missing proposals for authority 0;
        //      - 1 missing proposal in cache for authority 0;
        //      - 0 missing proposals for authorities 1, 2, and 3.
        //
        // Equivocations:
        // - We only removed from cache blocks from rounds <= 7, thus, no equivocations
        //   should be uncached. Then, we should have counted:
        //      - 0 uncached equivocations;
        //      - 1 equivocation in cache for authority 1;
        //      - 0 equivocations in cache for authorities 0, 2 and 3;
        //

        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![1, 0, 0, 0],
            ]
        );

        // Accept all the rest of blocks and commits.
        dag_state.accept_blocks(dag_builder.blocks(13..=20));
        for commit in second_temp_commits.clone() {
            dag_state.add_commit(commit);
        }

        // Clear and check all metrics
        scoring_metrics.uncached_metrics.reset();
        scoring_metrics.cached_metrics.reset();
        node_metrics
            .uncached_missing_proposals_by_authority
            .with_label_values(&[hostnames[0]])
            .reset();
        node_metrics
            .missing_proposals_in_cache_by_authority
            .with_label_values(&[hostnames[0]])
            .set(0);
        node_metrics
            .equivocations_in_cache_by_authority
            .with_label_values(&[hostnames[1]])
            .set(0);

        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0; committee_size]
            ]
        );

        // Destroy and recover dag state from storage.
        drop(dag_state);
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Since the last accepted blocks were not flushed, the equivocations from
        // rounds 13 to 20 should not be accounted for. The metrics should remain
        // the same as before this acceptance.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![1, 0, 0, 0],
                vec![0; committee_size],
                vec![2, 0, 0, 0],
                vec![0, 1, 0, 0],
                vec![1, 0, 0, 0],
            ]
        );

        // Now we accept those lost blocks again and flush the dag state
        dag_state.accept_blocks(dag_builder.blocks(13..=20));
        for commit in second_temp_commits.clone() {
            dag_state.add_commit(commit);
        }
        dag_state.flush();

        // Now all misbehaviors should be accounted for in the uncached metrics.
        assert_eq!(
            [
                scoring_metrics.uncached_equivocations_by_authority(),
                scoring_metrics.uncached_missing_proposals_by_authority(),
                scoring_metrics.equivocations_in_cache_by_authority(),
                scoring_metrics.missing_proposals_in_cache_by_authority(),
                get_uncached_equivocations(&context),
                get_uncached_missing_proposals(&context),
                get_equivocations_in_cache(&context),
                get_missing_proposals_in_cache(&context)
            ],
            [
                vec![0, 1, 2, 0],
                vec![3, 0, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
                vec![0, 1, 2, 0],
                vec![3, 0, 0, 0],
                vec![0; committee_size],
                vec![0; committee_size],
            ]
        );
    }

    #[tokio::test]
    async fn test_metrics_handle_send_block() {
        // Initialize context and authority service given a committee_size
        let committee_size = 4;
        let (_, context, _, _) = new_authority_service_for_metrics_tests(committee_size);
        let scoring_metrics = &context.scoring_metrics_store;
        let source = ErrorSource::Subscriber;
        // Create a set of errors to test
        let ignored_error = ConsensusError::Shutdown;
        let parsing_error = ConsensusError::MalformedBlock(bcs::Error::Eof);
        let block_verification_error = ConsensusError::InvalidAuthorityIndex {
            index: AuthorityIndex::new_for_test(5),
            max: 4,
        };
        let block_rejected_error = ConsensusError::BlockRejected {
            block_ref: BlockRef::new(10, AuthorityIndex::new_for_test(10), BlockDigest::MIN),
            reason: "string".to_string(),
        };
        // Update metrics for each authority with an error that should be ignored.
        // Metrics should not be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    ignored_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_rejected_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
            ]
        );

        // Update metrics for each authority with a parsing error.
        // Only unprovable metrics should be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    parsing_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_rejected_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0]
            ]
        );

        // Update metrics for each authority with a unsigned block verification error.
        // Only unprovable metrics should be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    block_verification_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_rejected_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![2, 2, 2, 2],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![1, 1, 1, 1],
                vec![0, 0, 0, 0],
            ]
        );

        // Update metrics for each authority with a block rejected verification error.
        // No metrics should be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    block_rejected_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_rejected_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![2, 2, 2, 2],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![1, 1, 1, 1],
                vec![0, 0, 0, 0],
            ]
        );
    }

    #[tokio::test]
    async fn test_metrics_fetch_once() {
        // Initialize context and authority service given a committee_size
        let committee_size = 4;
        let (_, context, _, _) = new_authority_service_for_metrics_tests(committee_size);
        let scoring_metrics = &context.scoring_metrics_store;
        let source = ErrorSource::CommitSyncer;
        // Create a set of errors to test
        let ignored_error = ConsensusError::Shutdown;
        let parsing_error = ConsensusError::MalformedBlock(bcs::Error::Eof);
        let block_verification_error = ConsensusError::TooManyAncestors(2, 2);

        // Update metrics for each authority with an error that should be ignored.
        // Metrics should not be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    ignored_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0]
            ]
        );

        // Update metrics for each authority with a parsing error.
        // Only unprovable metrics should be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    parsing_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![0, 0, 0, 0]
            ]
        );

        // Update metrics for each authority with a signed block verification error.
        // Since for error comes from the commit syncer, blocks received are not
        // necessarily from the peer. Thus, it is not provable that the peer actually
        // sent this block. Only unprovable metrics should be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    block_verification_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![2, 2, 2, 2],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![1, 1, 1, 1],
            ]
        );
    }

    #[tokio::test]
    async fn test_metrics_process_fetched_blocks() {
        // Initialize context and authority service given a committee_size
        let committee_size = 4;
        let (_, context, _, _) = new_authority_service_for_metrics_tests(committee_size);
        let scoring_metrics = &context.scoring_metrics_store;
        let source = ErrorSource::Synchronizer;
        // Create a set of errors to test
        let ignored_error = ConsensusError::Shutdown;
        let parsing_error = ConsensusError::MalformedBlock(bcs::Error::Eof);
        let block_verification_error = ConsensusError::TooManyAncestors(2, 2);

        // Update metrics for each authority with an error that should be ignored.
        // Metrics should not be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    ignored_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0]
            ]
        );

        // Update metrics for each authority with a parsing error.
        // Only unprovable metrics should be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    parsing_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![0, 0, 0, 0]
            ]
        );

        // Update metrics for each authority with a signed block verification error.
        // Since for error comes from the synchronizer, blocks received are not
        // necessarily from the peer. Thus, it is not provable that the peer actually
        // sent this block. Only unprovable metrics should be updated for this error.
        for authority in context.committee.authorities() {
            context
                .scoring_metrics_store
                .update_scoring_metrics_on_block_receival(
                    authority.0,
                    authority.1.hostname.as_str(),
                    block_verification_error.clone(),
                    source.clone(),
                    &context.metrics.node_metrics,
                    &context.protocol_config,
                );
        }
        assert_eq!(
            [
                scoring_metrics.faulty_blocks_provable_by_authority(),
                scoring_metrics.faulty_blocks_unprovable_by_authority(),
                get_faulty_blocks_provable(&context, &source, ignored_error.name()),
                get_faulty_blocks_provable(&context, &source, parsing_error.name()),
                get_faulty_blocks_provable(&context, &source, block_verification_error.name()),
                get_faulty_blocks_unprovable(&context, &source, ignored_error.name()),
                get_faulty_blocks_unprovable(&context, &source, parsing_error.name()),
                get_faulty_blocks_unprovable(&context, &source, block_verification_error.name())
            ],
            [
                vec![0, 0, 0, 0],
                vec![2, 2, 2, 2],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![1, 1, 1, 1],
            ]
        );
    }
}
