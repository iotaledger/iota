// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use iota_common::debug_fatal;
use prometheus_filtered::IntGauge;
use serde::{Deserialize, Serialize};
use starfish_config::{AuthorityIndex, Committee};

use crate::{
    block_header::{BlockRef, Round},
    context::Context,
    error::ConsensusError,
    metrics::NodeMetrics,
};

/// Per-authority misbehavior counters.
///
/// Two buckets track different lifecycle stages:
/// - `in_memory`: from blocks currently in the DAG cache (volatile, recomputed
///   on each flush from blocks still in cache).
/// - `persisted`: cumulative counts from blocks evicted from cache (restored
///   from storage on restart).
pub(crate) struct MisbehaviorStore {
    in_memory: CommitteeMisbehaviorCounts,
    persisted: CommitteeMisbehaviorCounts,
    /// Per-authority rounds in which a second verified header for an occupied
    /// slot was dropped at ingest. Counted into the in-memory equivocations on
    /// each flush and folded into the persisted count when the round is
    /// evicted.
    equivocating_rounds: Vec<Mutex<BTreeSet<Round>>>,
}

impl MisbehaviorStore {
    pub(crate) fn new(context: &Context) -> Self {
        let metrics = &context.metrics.node_metrics;
        Self {
            in_memory: CommitteeMisbehaviorCounts::new(&context.committee, metrics, "in_memory"),
            persisted: CommitteeMisbehaviorCounts::new(&context.committee, metrics, "persisted"),
            equivocating_rounds: (0..context.committee.size())
                .map(|_| Mutex::new(BTreeSet::new()))
                .collect(),
        }
    }

    /// Resets all counters to zero. Used during fast sync reinitialization.
    pub(crate) fn reset(&self) {
        self.in_memory.reset();
        self.persisted.reset();
        for rounds in &self.equivocating_rounds {
            rounds.lock().unwrap().clear();
        }
    }

    /// Records that `author` signed more than one header for `round`: a second
    /// verified header for an occupied slot was dropped at ingest. Idempotent —
    /// repeated arrivals and further distinct headers for the same slot
    /// collapse into one equivocation. Callers pass the author of a verified
    /// header, so an author outside the committee is a verification bug and is
    /// reported rather than recorded.
    pub(crate) fn record_equivocating_slot(&self, author: AuthorityIndex, round: Round) {
        let Some(rounds) = self.equivocating_rounds.get(author.value()) else {
            debug_fatal!("equivocating author {author} is outside the committee");
            return;
        };
        rounds.lock().unwrap().insert(round);
    }

    /// Restores persisted counts from storage and computes in-memory counts
    /// from the block refs already loaded into the DAG cache.
    pub(crate) fn initialize_misbehavior_counts(
        &self,
        recovered: BTreeMap<AuthorityIndex, MisbehaviorCounts>,
        recent_refs_by_authority: &[BTreeSet<BlockRef>],
        evicted_rounds: &[Round],
        threshold_clock_round: Round,
        context: &Arc<Context>,
    ) {
        for (authority_index, _) in context.committee.authorities() {
            let idx = authority_index.value();

            // Restore persisted counts from storage. Match exhaustively so a
            // future MisbehaviorCounts variant trips the compiler here.
            let storage_metrics = recovered.get(&authority_index).cloned().unwrap_or_default();
            match storage_metrics {
                // V1 rows carry no bundle-part count; restore it as zero.
                MisbehaviorCounts::V1(inner) => {
                    self.persisted.set_block_faults(
                        idx,
                        inner.faulty_blocks_provable,
                        inner.faulty_blocks_unprovable,
                        0,
                    );
                    self.persisted.set_dag_faults(
                        idx,
                        inner.missing_proposals,
                        inner.equivocations,
                    );
                }
                MisbehaviorCounts::V2(inner) => {
                    self.persisted.set_block_faults(
                        idx,
                        inner.faulty_blocks_provable,
                        inner.faulty_blocks_unprovable,
                        inner.invalid_bundle_parts,
                    );
                    self.persisted.set_dag_faults(
                        idx,
                        inner.missing_proposals,
                        inner.equivocations,
                    );
                }
            }

            // Compute in-memory counts from cached block refs.
            let eviction_round = evicted_rounds[idx];
            if threshold_clock_round > 0 {
                let cached_rounds: Vec<Round> = recent_refs_by_authority[idx]
                    .iter()
                    .map(|r| r.round)
                    .collect();
                // Equivocation flags are volatile: after a restart only the
                // cache surplus is visible until new drops are recorded.
                let (eq, missing) = calculate_misbehavior_counts_for_range(
                    cached_rounds,
                    &BTreeSet::new(),
                    eviction_round + 1,
                    threshold_clock_round.saturating_sub(1),
                );
                self.in_memory.set_dag_faults(idx, missing, eq);
            }
        }
    }

    /// Updates misbehavior counts for one authority during flush.
    /// Must be called before cache eviction is applied, while evicted block
    /// refs are still in `recent_refs`.
    /// Returns `Some(MisbehaviorCounts)` if persisted data changed and needs
    /// to be written to storage.
    pub(crate) fn update_misbehavior_counts_on_eviction(
        &self,
        authority_index: AuthorityIndex,
        recent_refs: &BTreeSet<BlockRef>,
        eviction_round: Round,
        last_eviction_round: Round,
        threshold_clock_round: Round,
        context: &Arc<Context>,
    ) -> Option<MisbehaviorCounts> {
        if threshold_clock_round == 0 || authority_index.value() >= context.committee.size() {
            return None;
        }
        let idx = authority_index.value();

        // Move buffered faulty block counts to persisted.
        let had_faulty = self.flush_faulty_block_buffer(idx);

        let mut equivocating_rounds = self.equivocating_rounds[idx].lock().unwrap();

        // Recompute in-memory window from blocks still in cache.
        let in_memory_block_rounds: Vec<Round> = recent_refs
            .iter()
            .map(|b| b.round)
            .filter(|&r| r > eviction_round && r < threshold_clock_round)
            .collect();
        let (in_memory_eq, in_memory_missing) = calculate_misbehavior_counts_for_range(
            in_memory_block_rounds,
            &equivocating_rounds,
            eviction_round + 1,
            threshold_clock_round.saturating_sub(1),
        );
        self.in_memory
            .set_dag_faults(idx, in_memory_missing, in_memory_eq);

        let eviction_advanced = eviction_round != last_eviction_round;
        if eviction_advanced {
            // Accumulate newly-evicted rounds into persisted.
            let evicted_block_rounds: Vec<Round> = recent_refs
                .iter()
                .map(|b| b.round)
                .filter(|&r| r <= eviction_round)
                .collect();
            let (evicted_eq, evicted_missing) = calculate_misbehavior_counts_for_range(
                evicted_block_rounds,
                &equivocating_rounds,
                last_eviction_round + 1,
                eviction_round,
            );
            self.persisted
                .add_dag_faults(idx, evicted_missing, evicted_eq);
            // Evicted rounds are folded into persisted above; drop their flags.
            let live_rounds = equivocating_rounds.split_off(&(eviction_round + 1));
            *equivocating_rounds = live_rounds;
        }

        if eviction_advanced || had_faulty {
            let counts = self.persisted.snapshot(idx);
            let persisted = match context.protocol_config.scorer_version_as_option() {
                Some(2) => MisbehaviorCounts::V2(counts),
                _ => MisbehaviorCounts::V1(counts.fold_into_v1()),
            };
            Some(persisted)
        } else {
            None
        }
    }

    /// Flush buffered faulty block counts from in_memory to persisted
    /// for one authority. Returns true if any counts were moved.
    fn flush_faulty_block_buffer(&self, idx: usize) -> bool {
        let (prov, unprov, bundle) = self.in_memory.drain_block_faults(idx);
        if prov == 0 && unprov == 0 && bundle == 0 {
            return false;
        }
        self.persisted.add_block_faults(idx, prov, unprov, bundle);
        true
    }

    /// Returns an absolute per-authority snapshot of `persisted + in_memory`
    /// counts for emission with `CommittedSubDag`. Locks the two buckets
    /// independently per authority; callers must hold `dag_state.read()` so
    /// concurrent flush (which writes both buckets under `dag_state.write()`)
    /// is excluded.
    pub(crate) fn snapshot_totals(&self) -> Vec<MisbehaviorCounts> {
        (0..self.in_memory.authorities.len())
            .map(|i| {
                let persisted = self.persisted.snapshot(i);
                let in_memory = self.in_memory.snapshot(i);
                MisbehaviorCounts::V2(MisbehaviorCountsV2 {
                    faulty_blocks_provable: persisted.faulty_blocks_provable
                        + in_memory.faulty_blocks_provable,
                    faulty_blocks_unprovable: persisted.faulty_blocks_unprovable
                        + in_memory.faulty_blocks_unprovable,
                    missing_proposals: persisted.missing_proposals + in_memory.missing_proposals,
                    equivocations: persisted.equivocations + in_memory.equivocations,
                    invalid_bundle_parts: persisted.invalid_bundle_parts
                        + in_memory.invalid_bundle_parts,
                })
            })
            .collect()
    }

    /// Records a faulty block event detected during ingest validation — a
    /// rejected block header, or a corrupt part of a relayed bundle (framing,
    /// metadata, or shard). Events are buffered in the in_memory bucket and
    /// moved to persisted on the next flush.
    ///
    /// `peer` is the authority that sent us the block (always known from the
    /// network connection). `author` is the claimed block author (from the
    /// header's author field, only trustworthy if the signature is valid).
    ///
    /// Attribution rules:
    /// - Provable faults (valid signature, protocol violation): charged to
    ///   `author` — cryptographic proof they produced an invalid block. If
    ///   `peer != author`, the peer is also charged (unprovable) for
    ///   distributing a block they could have verified themselves.
    /// - Unprovable faults (bad/missing signature): charged to `peer` only — we
    ///   can't verify the author field, but we know who sent it to us.
    /// - Bundle-part faults (corrupt framing, metadata, or shard): charged to
    ///   `peer` only, under the dedicated bundle-part counter.
    pub(crate) fn record_faulty_block(
        &self,
        peer: AuthorityIndex,
        author: AuthorityIndex,
        error: &ConsensusError,
    ) {
        // `peer` is trusted (authenticated connection); `author` is
        // attacker-controlled and may be out of committee range.
        let committee_size = self.in_memory.authorities.len();
        let peer_idx = peer.value();
        let author_idx = author.value();
        if peer_idx >= committee_size {
            return;
        }
        match classify_block_error(error) {
            FaultType::Provable => {
                if author_idx >= committee_size {
                    // Can't credit a bogus author; charge the serving peer instead.
                    self.in_memory.record_block_fault_unprovable(peer_idx);
                    return;
                }
                self.in_memory.record_block_fault_provable(author_idx);
                if peer != author {
                    self.in_memory.record_block_fault_unprovable(peer_idx);
                }
            }
            FaultType::Unprovable => {
                self.in_memory.record_block_fault_unprovable(peer_idx);
            }
            FaultType::BundlePart => {
                self.in_memory.record_bundle_part_fault(peer_idx);
            }
            FaultType::Untracked => {}
        }
    }

    /// Attributes a transaction payload that failed the commitment check or
    /// the transaction validity check.
    ///
    /// `author` is charged with a provable fault only when `authored` is true,
    /// i.e. a verified, author-signed block header commits to this payload, so
    /// the invalid transactions are provably the author's. Each peer in
    /// `relayers` is charged an unprovable fault for handing us a payload it
    /// should not have relayed; a relayer that is also the author is not
    /// double-charged when the author is already charged provably.
    pub(crate) fn record_faulty_transactions(
        &self,
        author: AuthorityIndex,
        authored: bool,
        relayers: impl IntoIterator<Item = AuthorityIndex>,
    ) {
        let committee_size = self.in_memory.authorities.len();
        if authored && author.value() < committee_size {
            self.in_memory.record_block_fault_provable(author.value());
        }
        for peer in relayers {
            let idx = peer.value();
            if idx >= committee_size || (authored && peer == author) {
                continue;
            }
            self.in_memory.record_block_fault_unprovable(idx);
        }
    }
}

/// Whether a block fault can be cryptographically proven.
enum FaultType {
    /// Block has a valid author signature but violates protocol rules.
    /// The signed block header itself is proof of misbehavior.
    Provable,
    /// Can't prove authorship — either because the signature is bad or
    /// missing, or because the header is rejected by a pre-signature check
    /// (epoch / genesis / author-vs-peer mismatch) so its `author` field
    /// can't be trusted. Charged to the sending peer, not the claimed author.
    Unprovable,
    /// Corrupt or invalid relayed bundle part (framing, metadata, or shard)
    /// that isn't tied to a verified author. Charged to the sending peer.
    BundlePart,
    /// Not counted as misbehavior.
    Untracked,
}

fn classify_block_error(error: &ConsensusError) -> FaultType {
    // Exhaustive on purpose: a new `ConsensusError` variant must be assigned a
    // fault type here rather than silently falling through to `Untracked`.
    match error {
        // Pre-signature / parsing errors — the header's author field can't
        // be trusted, so charge the sender, not the claimed author.
        ConsensusError::WrongEpoch { .. }
        | ConsensusError::UnexpectedGenesisHeader
        | ConsensusError::UnexpectedAuthority(..)
        | ConsensusError::InvalidAuthorityIndex { .. }
        | ConsensusError::MalformedHeader(_)
        | ConsensusError::MalformedTransactions(_)
        | ConsensusError::MalformedSignature(_)
        | ConsensusError::SignatureVerificationFailure(_)
        | ConsensusError::SerializationFailure(_)
        | ConsensusError::DeserializationFailure(_)
        | ConsensusError::SerializedTransactionsTooLarge { .. }
        | ConsensusError::TransactionCommitmentFailure { .. }
        | ConsensusError::UnexpectedBlockHeaderForCommit { .. }
        | ConsensusError::TooManyFetchedHeadersReturned { .. } => FaultType::Unprovable,

        // Corrupt or invalid relayed bundle parts (framing, additional-header
        // round, and shard structure/proof). We know which peer relayed them
        // but can't tie them to a verified author.
        ConsensusError::MalformedShard(_)
        | ConsensusError::TooBigHeaderRoundInABundle { .. }
        | ConsensusError::TooBigShardRoundInABundle { .. }
        | ConsensusError::IncorrectShardProof { .. }
        | ConsensusError::UnrequestedHeaderOutOfWindow { .. }
        | ConsensusError::SerializedShardTooLarge { .. } => FaultType::BundlePart,

        // Checks that run only after the author's signature is verified, so the
        // signed header itself proves the author produced a block that violates
        // protocol rules.
        ConsensusError::TooManyAncestors(..)
        | ConsensusError::InsufficientParentStakes { .. }
        | ConsensusError::InvalidAncestorPosition { .. }
        | ConsensusError::InvalidAncestorRound { .. }
        | ConsensusError::AncestorRoundTooOld { .. }
        | ConsensusError::AcknowledgmentRoundTooOld { .. }
        | ConsensusError::InvalidGenesisAncestor(_)
        | ConsensusError::DuplicatedAncestorsAuthority(_)
        | ConsensusError::InvalidOverlapIndices { .. }
        | ConsensusError::TooManyAcknowledgments { .. }
        | ConsensusError::TooManyCommitVotes { .. }
        | ConsensusError::InvalidAcknowledgmentRound { .. }
        | ConsensusError::InvalidStrongVoteAuthority { .. }
        | ConsensusError::StrongVoteLeaderNotInAncestors { .. }
        | ConsensusError::TransactionTooLarge { .. }
        | ConsensusError::TooManyTransactions { .. }
        | ConsensusError::TooManyTransactionBytes { .. }
        | ConsensusError::InvalidTransaction(_) => FaultType::Provable,

        // Not attributable as misbehavior from a signed block alone: subjective
        // rejections, commit-chain and commit-sync inconsistencies, fetch-shape
        // mismatches, reconstruction and encoder failures, storage, network
        // transport, request-shape checks, local lifecycle signals, and version
        // mismatches that may stem from a benign upgrade misconfiguration rather
        // than a faulty peer.
        ConsensusError::MalformedCommit(_)
        | ConsensusError::UnexpectedGenesisRequested { .. }
        | ConsensusError::NotEnoughHeadersFetched { .. }
        | ConsensusError::UnexpectedLastOwnHeader { .. }
        // Transaction fetch faults, recorded against the serving peer at the
        // fetch sites via `record_faulty_transactions` — deliberately not
        // tracked again here.
        | ConsensusError::TooManyFetchedTransactionsReturned(_)
        | ConsensusError::UnrequestedTransactionFetched { .. }
        | ConsensusError::UnexpectedTransactionForCommit { .. }
        | ConsensusError::TooManyAuthoritiesProvided(_)
        | ConsensusError::InvalidSizeOfHighestAcceptedRounds(..)
        | ConsensusError::InvalidAuthorityIndexRequested { .. }
        | ConsensusError::TransactionCommitmentMismatch { .. }
        | ConsensusError::SynchronizerSaturated(_)
        | ConsensusError::TransactionSynchronizerSaturated
        | ConsensusError::BlockRejected { .. }
        | ConsensusError::EmptyMerkleTree
        | ConsensusError::MissingBlockHeader { .. }
        | ConsensusError::CommitRangeExceededAfterScanning { .. }
        | ConsensusError::TooManyCommitsFromPeer { .. }
        | ConsensusError::NoCommitReceived { .. }
        | ConsensusError::UnexpectedStartCommit { .. }
        | ConsensusError::UnexpectedCommitSequence { .. }
        | ConsensusError::NotEnoughCommitVotes { .. }
        | ConsensusError::TooManyCommitVoteHeaders { .. }
        | ConsensusError::SerializedCommitTooLarge { .. }
        | ConsensusError::SerializedBlockHeaderTooLarge { .. }
        | ConsensusError::InvalidCommitRange { .. }
        | ConsensusError::FetchedTransactionsMismatch { .. }
        | ConsensusError::RocksDBFailure(_)
        | ConsensusError::NetworkConfig(_)
        | ConsensusError::NetworkClientConnection(_)
        | ConsensusError::NetworkRequest(_)
        | ConsensusError::NetworkRequestTimeout(_)
        | ConsensusError::AccumulatorSenderClosed
        | ConsensusError::Shutdown
        | ConsensusError::EncoderResetFailed(_)
        | ConsensusError::AddShardFailed(_)
        | ConsensusError::ShardsEncodingFailed(_)
        | ConsensusError::ShardsDecodingFailed(_)
        | ConsensusError::InsufficientShardsInDecoder(..)
        | ConsensusError::ShardsVecIsTooSmall(..)
        | ConsensusError::InconsistentTransactionRefVariants
        | ConsensusError::TransactionRefVariantMismatch { .. }
        | ConsensusError::FailedToFetchBlockHeaders { .. }
        | ConsensusError::MissingVotingBlockHeaderInStorage { .. }
        | ConsensusError::WrongShardVersion { .. }
        | ConsensusError::WrongCommitVersionForFlags { .. }
        | ConsensusError::WrongBlockHeaderVersionForFlag { .. } => FaultType::Untracked,
    }
}

/// The five Prometheus gauges that mirror `MisbehaviorCountsV2` for one
/// `(authority, source)` pair, pre-resolved at construction so per-call
/// methods don't repeat `with_label_values` lookups and don't have to
/// thread `&NodeMetrics` and `hostname` through every call site.
struct AuthorityGauges {
    block_fault_provable: IntGauge,
    block_fault_unprovable: IntGauge,
    missing_proposals: IntGauge,
    equivocations: IntGauge,
    invalid_bundle_parts: IntGauge,
}

/// Per-authority misbehavior counters and matching Prometheus gauges.
/// State mutations and gauge updates always happen together inside the same
/// per-authority `Mutex` lock, so concurrent observers see counter and gauge
/// agree.
struct CommitteeMisbehaviorCounts {
    authorities: Vec<Mutex<MisbehaviorCountsV2>>,
    gauges: Vec<AuthorityGauges>,
}

impl CommitteeMisbehaviorCounts {
    /// `source` is the `source` label baked into all gauge updates for this
    /// instance (`"in_memory"` or `"persisted"`).
    fn new(committee: &Committee, metrics: &NodeMetrics, source: &'static str) -> Self {
        let gauges = committee
            .authorities()
            .map(|(_, a)| {
                let labels = &[a.hostname.as_str(), source];
                AuthorityGauges {
                    block_fault_provable: metrics
                        .faulty_blocks_provable_by_authority
                        .with_label_values(labels),
                    block_fault_unprovable: metrics
                        .faulty_blocks_unprovable_by_peer
                        .with_label_values(labels),
                    missing_proposals: metrics
                        .missing_proposals_by_authority
                        .with_label_values(labels),
                    equivocations: metrics.equivocations_by_authority.with_label_values(labels),
                    invalid_bundle_parts: metrics
                        .invalid_bundle_parts_by_peer
                        .with_label_values(labels),
                }
            })
            .collect();
        Self {
            authorities: (0..committee.size())
                .map(|_| Mutex::new(MisbehaviorCountsV2::default()))
                .collect(),
            gauges,
        }
    }

    fn record_block_fault_provable(&self, idx: usize) {
        let mut c = self.authorities[idx].lock().unwrap();
        c.faulty_blocks_provable += 1;
        self.gauges[idx].block_fault_provable.inc();
    }

    fn record_block_fault_unprovable(&self, idx: usize) {
        let mut c = self.authorities[idx].lock().unwrap();
        c.faulty_blocks_unprovable += 1;
        self.gauges[idx].block_fault_unprovable.inc();
    }

    fn record_bundle_part_fault(&self, idx: usize) {
        let mut c = self.authorities[idx].lock().unwrap();
        c.invalid_bundle_parts += 1;
        self.gauges[idx].invalid_bundle_parts.inc();
    }

    /// Atomically take all block-fault counters AND zero the matching gauges.
    /// The caller transfers the returned `(provable, unprovable,
    /// bundle_parts)` into the persisted bucket via `add_block_faults`.
    fn drain_block_faults(&self, idx: usize) -> (u64, u64, u64) {
        let mut c = self.authorities[idx].lock().unwrap();
        let prov = std::mem::take(&mut c.faulty_blocks_provable);
        let unprov = std::mem::take(&mut c.faulty_blocks_unprovable);
        let bundle = std::mem::take(&mut c.invalid_bundle_parts);
        self.gauges[idx].block_fault_provable.set(0);
        self.gauges[idx].block_fault_unprovable.set(0);
        self.gauges[idx].invalid_bundle_parts.set(0);
        (prov, unprov, bundle)
    }

    /// Receive a `drain_block_faults` payload — additive on counters and
    /// gauges.
    fn add_block_faults(&self, idx: usize, prov: u64, unprov: u64, bundle: u64) {
        let mut c = self.authorities[idx].lock().unwrap();
        c.faulty_blocks_provable += prov;
        c.faulty_blocks_unprovable += unprov;
        c.invalid_bundle_parts += bundle;
        self.gauges[idx].block_fault_provable.add(prov as i64);
        self.gauges[idx].block_fault_unprovable.add(unprov as i64);
        self.gauges[idx].invalid_bundle_parts.add(bundle as i64);
    }

    /// Overwrite all block-fault counters and gauges. Used to restore from
    /// storage on startup.
    fn set_block_faults(&self, idx: usize, prov: u64, unprov: u64, bundle: u64) {
        let mut c = self.authorities[idx].lock().unwrap();
        c.faulty_blocks_provable = prov;
        c.faulty_blocks_unprovable = unprov;
        c.invalid_bundle_parts = bundle;
        self.gauges[idx].block_fault_provable.set(prov as i64);
        self.gauges[idx].block_fault_unprovable.set(unprov as i64);
        self.gauges[idx].invalid_bundle_parts.set(bundle as i64);
    }

    /// Overwrite the DAG-observed faults (`missing_proposals` +
    /// `equivocations`) and their gauges. Used on `in_memory` where the
    /// window is recomputed wholesale on every flush.
    fn set_dag_faults(&self, idx: usize, missing: u64, equiv: u64) {
        let mut c = self.authorities[idx].lock().unwrap();
        c.missing_proposals = missing;
        c.equivocations = equiv;
        self.gauges[idx].missing_proposals.set(missing as i64);
        self.gauges[idx].equivocations.set(equiv as i64);
    }

    /// Accumulate DAG-observed faults — additive on counters and gauges.
    /// Used on `persisted` to fold in newly-evicted rounds.
    fn add_dag_faults(&self, idx: usize, missing: u64, equiv: u64) {
        let mut c = self.authorities[idx].lock().unwrap();
        c.missing_proposals += missing;
        c.equivocations += equiv;
        self.gauges[idx].missing_proposals.add(missing as i64);
        self.gauges[idx].equivocations.add(equiv as i64);
    }

    /// Stable read clone of one authority's counters.
    fn snapshot(&self, idx: usize) -> MisbehaviorCountsV2 {
        self.authorities[idx].lock().unwrap().clone()
    }

    fn reset(&self) {
        for (m, g) in self.authorities.iter().zip(self.gauges.iter()) {
            let mut c = m.lock().unwrap();
            *c = MisbehaviorCountsV2::default();
            g.block_fault_provable.set(0);
            g.block_fault_unprovable.set(0);
            g.missing_proposals.set(0);
            g.equivocations.set(0);
            g.invalid_bundle_parts.set(0);
        }
    }

    #[cfg(test)]
    fn collect<F: Fn(&MisbehaviorCountsV2) -> u64>(&self, field: F) -> Vec<u64> {
        self.authorities
            .iter()
            .map(|m| field(&m.lock().unwrap()))
            .collect()
    }
}

/// Given one authority's cached block rounds in [start, end] and the rounds
/// flagged as equivocating at ingest, returns (equivocations,
/// missing_proposals). An equivocation is a round with more than one signed
/// header observed: more than one cached header, or a flag recorded when a
/// second header for the slot was dropped. A flagged round with no cached
/// header still counts as missing — none of its headers was accepted.
fn calculate_misbehavior_counts_for_range(
    mut block_rounds: Vec<Round>,
    flagged_rounds: &BTreeSet<Round>,
    start: Round,
    end: Round,
) -> (u64, u64) {
    if start > end {
        return (0, 0);
    }
    block_rounds.retain(|&round| round >= start && round <= end);
    block_rounds.sort();
    let mut equivocating_rounds: BTreeSet<Round> = block_rounds
        .windows(2)
        .filter_map(|pair| (pair[0] == pair[1]).then_some(pair[0]))
        .collect();
    equivocating_rounds.extend(flagged_rounds.range(start..=end).copied());
    block_rounds.dedup();
    let unique_block_rounds = block_rounds.len();
    let number_of_missing_blocks =
        (end + 1).saturating_sub(start + unique_block_rounds as u32) as u64;
    (equivocating_rounds.len() as u64, number_of_missing_blocks)
}

/// Versioned envelope for persisted scoring metrics. New versions are added as
/// enum variants so existing RocksDB data deserializes without migration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MisbehaviorCounts {
    V1(MisbehaviorCountsV1),
    V2(MisbehaviorCountsV2),
}

impl Default for MisbehaviorCounts {
    fn default() -> Self {
        Self::V2(MisbehaviorCountsV2::default())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MisbehaviorCountsV1 {
    pub faulty_blocks_provable: u64,
    pub faulty_blocks_unprovable: u64,
    pub missing_proposals: u64,
    /// Rounds in which the authority signed more than one block header,
    /// however many extra headers each round held.
    pub equivocations: u64,
}

/// Extends `MisbehaviorCountsV1` with a dedicated counter for invalid relayed
/// bundle parts (framing, metadata, or shard faults).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MisbehaviorCountsV2 {
    pub faulty_blocks_provable: u64,
    pub faulty_blocks_unprovable: u64,
    pub missing_proposals: u64,
    /// Rounds in which the authority signed more than one block header,
    /// however many extra headers each round held.
    pub equivocations: u64,
    pub invalid_bundle_parts: u64,
}

impl MisbehaviorCountsV2 {
    /// Folds the bundle-part count into `faulty_blocks_unprovable`.
    fn fold_into_v1(&self) -> MisbehaviorCountsV1 {
        MisbehaviorCountsV1 {
            faulty_blocks_provable: self.faulty_blocks_provable,
            faulty_blocks_unprovable: self
                .faulty_blocks_unprovable
                .saturating_add(self.invalid_bundle_parts),
            missing_proposals: self.missing_proposals,
            equivocations: self.equivocations,
        }
    }
}

#[cfg(test)]
impl MisbehaviorStore {
    pub(crate) fn persisted_missing_proposals(&self) -> Vec<u64> {
        self.persisted.collect(|c| c.missing_proposals)
    }

    pub(crate) fn persisted_equivocations(&self) -> Vec<u64> {
        self.persisted.collect(|c| c.equivocations)
    }

    pub(crate) fn in_memory_missing_proposals(&self) -> Vec<u64> {
        self.in_memory.collect(|c| c.missing_proposals)
    }

    pub(crate) fn in_memory_equivocations(&self) -> Vec<u64> {
        self.in_memory.collect(|c| c.equivocations)
    }

    pub(crate) fn in_memory_faulty_blocks_provable(&self) -> Vec<u64> {
        self.in_memory.collect(|c| c.faulty_blocks_provable)
    }

    pub(crate) fn in_memory_faulty_blocks_unprovable(&self) -> Vec<u64> {
        self.in_memory.collect(|c| c.faulty_blocks_unprovable)
    }

    pub(crate) fn equivocating_rounds(&self, author: AuthorityIndex) -> Vec<Round> {
        self.equivocating_rounds[author.value()]
            .lock()
            .unwrap()
            .iter()
            .copied()
            .collect()
    }
}

#[cfg(test)]
impl MisbehaviorCounts {
    pub(crate) fn new_v1_for_test(
        faulty_blocks_provable: u64,
        faulty_blocks_unprovable: u64,
        missing_proposals: u64,
        equivocations: u64,
    ) -> Self {
        Self::V1(MisbehaviorCountsV1 {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
        })
    }

    pub(crate) fn new_v2_for_test(
        faulty_blocks_provable: u64,
        faulty_blocks_unprovable: u64,
        missing_proposals: u64,
        equivocations: u64,
        invalid_bundle_parts: u64,
    ) -> Self {
        Self::V2(MisbehaviorCountsV2 {
            faulty_blocks_provable,
            faulty_blocks_unprovable,
            missing_proposals,
            equivocations,
            invalid_bundle_parts,
        })
    }

    /// Test accessor for the V2 payload; panics on any other variant. All
    /// counts produced by a running store are V2 (V1 exists only for reading
    /// old persisted rows).
    pub(crate) fn as_v2(&self) -> &MisbehaviorCountsV2 {
        match self {
            Self::V2(inner) => inner,
            other => panic!("expected MisbehaviorCounts::V2, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fastcrypto::error::FastCryptoError;
    use starfish_config::Parameters;

    use super::*;
    use crate::{
        block_header::{BlockHeaderDigest, BlockRef},
        context::Context,
        dag_state::{DagState, DataSource},
        error::ConsensusError,
        storage::mem_store::MemStore,
        test_dag_builder::DagBuilder,
    };

    #[test]
    fn test_calculate_misbehavior_counts_for_range_basic() {
        let no_flags = BTreeSet::new();

        // No blocks in range → all missing, no equivocations
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![], &no_flags, 1, 5);
        assert_eq!(eq, 0);
        assert_eq!(missing, 5);

        // All rounds present, no equivocations
        let (eq, missing) =
            calculate_misbehavior_counts_for_range(vec![1, 2, 3, 4, 5], &no_flags, 1, 5);
        assert_eq!(eq, 0);
        assert_eq!(missing, 0);

        // One equivocating round (duplicate round 3)
        let (eq, missing) =
            calculate_misbehavior_counts_for_range(vec![1, 2, 3, 3, 4, 5], &no_flags, 1, 5);
        assert_eq!(eq, 1);
        assert_eq!(missing, 0);

        // One missing (round 3) + one equivocating round (round 2)
        let (eq, missing) =
            calculate_misbehavior_counts_for_range(vec![1, 2, 2, 4, 5], &no_flags, 1, 5);
        assert_eq!(eq, 1);
        assert_eq!(missing, 1);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_filters_out_of_range() {
        // Rounds outside [2, 4] are filtered, in the cached rounds and in the
        // flags alike
        let flags = BTreeSet::from([1, 5]);
        let (eq, missing) =
            calculate_misbehavior_counts_for_range(vec![1, 2, 3, 4, 5], &flags, 2, 4);
        assert_eq!(eq, 0);
        assert_eq!(missing, 0);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_empty_range() {
        // start > end → no missing, no equivocations
        let (eq, missing) =
            calculate_misbehavior_counts_for_range(vec![], &BTreeSet::from([4]), 5, 3);
        assert_eq!(eq, 0);
        assert_eq!(missing, 0);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_unsorted_input() {
        // Unsorted with one equivocating round (round 3 appears twice) and one
        // missing (round 5)
        let (eq, missing) =
            calculate_misbehavior_counts_for_range(vec![4, 1, 3, 2, 3], &BTreeSet::new(), 1, 5);
        assert_eq!(eq, 1);
        assert_eq!(missing, 1);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_multiple_equivocations() {
        // Round 2 appears 3 times and round 4 twice — two equivocating rounds,
        // however many extra headers each holds
        let (eq, missing) = calculate_misbehavior_counts_for_range(
            vec![1, 2, 2, 2, 3, 4, 4],
            &BTreeSet::new(),
            1,
            4,
        );
        assert_eq!(eq, 2);
        assert_eq!(missing, 0);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_single_round() {
        let no_flags = BTreeSet::new();

        // Single-round range with block present
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![5], &no_flags, 5, 5);
        assert_eq!(eq, 0);
        assert_eq!(missing, 0);

        // Single-round range with no block
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![], &no_flags, 5, 5);
        assert_eq!(eq, 0);
        assert_eq!(missing, 1);

        // Single-round range with equivocation
        let (eq, missing) = calculate_misbehavior_counts_for_range(vec![5, 5], &no_flags, 5, 5);
        assert_eq!(eq, 1);
        assert_eq!(missing, 0);
    }

    #[test]
    fn test_calculate_misbehavior_counts_for_range_flagged_rounds() {
        // A flag alone marks the round as equivocating
        let (eq, missing) =
            calculate_misbehavior_counts_for_range(vec![1, 2, 3], &BTreeSet::from([2]), 1, 3);
        assert_eq!(eq, 1);
        assert_eq!(missing, 0);

        // A flag and a cached surplus on the same round count once
        let (eq, missing) =
            calculate_misbehavior_counts_for_range(vec![1, 2, 2, 3], &BTreeSet::from([2]), 1, 3);
        assert_eq!(eq, 1);
        assert_eq!(missing, 0);

        // A flagged round with no cached header counts as both equivocating
        // and missing: two headers were signed, none was accepted
        let (eq, missing) =
            calculate_misbehavior_counts_for_range(vec![1, 3], &BTreeSet::from([2]), 1, 3);
        assert_eq!(eq, 1);
        assert_eq!(missing, 1);
    }

    #[tokio::test]
    async fn test_update_misbehavior_counts_on_eviction_edge_cases() {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(&context);
        let authority_index = AuthorityIndex::new_for_test(0);
        let recent_refs = BTreeSet::new();

        // threshold_clock_round=0 → always returns None
        let result = store.update_misbehavior_counts_on_eviction(
            authority_index,
            &recent_refs,
            0,
            0,
            0,
            &context,
        );
        assert!(result.is_none());

        // No eviction (eviction_round == last_eviction_round) AND no buffered
        // faulty headers → None. (Eviction advance OR a faulty flush is what
        // triggers a write; here neither happens.)
        let result = store.update_misbehavior_counts_on_eviction(
            authority_index,
            &recent_refs,
            5,
            5,
            5,
            &context,
        );
        assert!(result.is_none());

        // Eviction happened with empty refs → missing proposals accumulated
        let result = store.update_misbehavior_counts_on_eviction(
            authority_index,
            &recent_refs,
            3,
            0,
            2,
            &context,
        );
        assert_eq!(
            result,
            Some(MisbehaviorCounts::new_v2_for_test(0, 0, 3, 0, 0))
        );

        // Out-of-bounds authority → None
        let oob = AuthorityIndex::new_for_test(4);
        let result =
            store.update_misbehavior_counts_on_eviction(oob, &recent_refs, 2, 1, 3, &context);
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_equivocating_slot_flags() {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(&context);
        let author = AuthorityIndex::new_for_test(1);
        let idx = author.value();

        // One cached header per round 1..=9.
        let recent_refs: BTreeSet<BlockRef> = (1..=9)
            .map(|round| BlockRef::new(round, author, BlockHeaderDigest::MIN))
            .collect();

        // Repeated arrivals and a further distinct header collapse per slot.
        store.record_equivocating_slot(author, 5);
        store.record_equivocating_slot(author, 5);
        store.record_equivocating_slot(author, 7);

        // No eviction advance: the flags show up in the in-memory window and
        // no block fault is charged.
        let result =
            store.update_misbehavior_counts_on_eviction(author, &recent_refs, 0, 0, 10, &context);
        assert!(result.is_none());
        assert_eq!(store.in_memory_equivocations()[idx], 2);
        assert_eq!(store.in_memory_faulty_blocks_provable()[idx], 0);

        // Eviction advances to round 6: the flag at round 5 folds into
        // persisted and is pruned; round 7 stays in the in-memory window.
        let result =
            store.update_misbehavior_counts_on_eviction(author, &recent_refs, 6, 0, 10, &context);
        assert!(result.is_some());
        assert_eq!(store.persisted_equivocations()[idx], 1);
        assert_eq!(store.in_memory_equivocations()[idx], 1);
        assert_eq!(store.equivocating_rounds(author), vec![7]);

        // The next advance folds the remaining flag; nothing is counted twice.
        let result =
            store.update_misbehavior_counts_on_eviction(author, &recent_refs, 9, 6, 10, &context);
        assert!(result.is_some());
        assert_eq!(store.persisted_equivocations()[idx], 2);
        assert_eq!(store.in_memory_equivocations()[idx], 0);
        assert!(store.equivocating_rounds(author).is_empty());

        // Reset clears the flags.
        store.record_equivocating_slot(author, 12);
        store.reset();
        assert!(store.equivocating_rounds(author).is_empty());
    }

    #[tokio::test]
    async fn test_persisted_variant_follows_scorer_version() {
        // With scorer version 2 inactive, persisted rows use the V1 format so a
        // rollback to a binary without the V2 variant can still read them, and
        // the bundle-part count folds into `faulty_blocks_unprovable`.
        let mut context = Context::new_for_test(4).0;
        context.protocol_config.set_scorer_version_for_testing(1);
        let context = Arc::new(context);
        let store = MisbehaviorStore::new(&context);
        let authority = AuthorityIndex::new_for_test(0);

        store.record_faulty_block(
            authority,
            authority,
            &ConsensusError::WrongEpoch {
                expected: 1,
                actual: 2,
            },
        );
        store.record_faulty_block(
            authority,
            authority,
            &ConsensusError::MalformedShard(bcs::Error::Custom("bad".to_string())),
        );

        let result = store.update_misbehavior_counts_on_eviction(
            authority,
            &BTreeSet::new(),
            0,
            0,
            1,
            &context,
        );

        // V1 row: the one bundle part folds into unprovable (1 + 1).
        assert_eq!(result, Some(MisbehaviorCounts::new_v1_for_test(0, 2, 0, 0)));
    }

    #[tokio::test]
    async fn test_metrics_flush_and_recovery() {
        let committee_size = 4;
        let (context, _) = Context::new_for_test(committee_size);
        let context = context.with_parameters(Parameters {
            dag_state_cached_rounds: 5,
            ..Default::default()
        });
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Build a 20-round DAG:
        // - Rounds 6-8: authority 0 skips proposals
        // - Round 11: authority 1 equivocates (1 extra block)
        // - Round 13: authority 2 equivocates (2 extra blocks)
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

        // Accept blocks+commits for first 10 rounds
        let temp_commits = commits.split_off(9);
        dag_state.accept_block_headers(dag_builder.block_headers(1..=10), DataSource::Test);
        for commit in commits {
            dag_state.add_commit(commit);
        }

        // Metrics should be zero before flush
        let scoring = dag_state.misbehavior_store();
        assert_eq!(scoring.persisted_equivocations(), vec![0; committee_size]);
        assert_eq!(
            scoring.persisted_missing_proposals(),
            vec![0; committee_size]
        );

        // Flush — this triggers misbehavior_counts_to_write
        dag_state.flush();

        // After flush: authority 0 should have missing proposals in the in-memory
        // window (cached rounds 6-8 where it didn't propose).
        // No equivocations yet (those are in rounds 11+, not accepted yet).
        let scoring = dag_state.misbehavior_store();
        assert_eq!(scoring.persisted_equivocations(), vec![0; committee_size]);
        assert!(scoring.in_memory_missing_proposals()[0] > 0);

        // Drop and recover from storage
        let persisted_missing_before = scoring.persisted_missing_proposals();
        let in_memory_missing_before_drop = scoring.in_memory_missing_proposals();
        drop(dag_state);
        let mut dag_state = DagState::new(context, store);

        // Persisted metrics should be restored, and in-memory recomputed from
        // the cached block refs loaded during recovery.
        let scoring = dag_state.misbehavior_store();
        assert_eq!(
            scoring.persisted_missing_proposals(),
            persisted_missing_before
        );
        // In-memory is recomputed from cached refs on init (not zero).
        let in_memory_after_recovery = scoring.in_memory_missing_proposals();
        assert_eq!(
            in_memory_after_recovery[0], in_memory_missing_before_drop[0],
            "In-memory should be recomputed from cached refs on startup"
        );

        // Accept rounds 11-20 and flush
        dag_state.accept_block_headers(dag_builder.block_headers(11..=20), DataSource::Test);
        for commit in temp_commits {
            dag_state.add_commit(commit);
        }
        dag_state.flush();

        // Now equivocations should be tracked
        let scoring = dag_state.misbehavior_store();
        let total_eq: u64 = scoring.persisted_equivocations().iter().sum::<u64>()
            + scoring.in_memory_equivocations().iter().sum::<u64>();
        assert!(total_eq > 0, "Should have detected equivocations");
    }

    #[tokio::test]
    async fn test_no_double_counting_on_restart() {
        let committee_size = 4;
        let (context, _) = Context::new_for_test(committee_size);
        let context = context.with_parameters(Parameters {
            dag_state_cached_rounds: 5,
            ..Default::default()
        });
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Build a DAG where authority 0 skips rounds 3-4
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=2).build();
        dag_builder
            .layers(3..=4)
            .authorities(vec![AuthorityIndex::new_for_test(0)])
            .skip_block()
            .build();
        dag_builder.layers(5..=10).build();

        let commits = dag_builder
            .get_sub_dag_and_commits(1..=10)
            .into_iter()
            .map(|(_subdag, commit)| commit)
            .collect::<Vec<_>>();

        dag_state.accept_block_headers(dag_builder.block_headers(1..=10), DataSource::Test);
        for commit in commits {
            dag_state.add_commit(commit);
        }
        dag_state.flush();

        // Record persisted counts after first flush
        let persisted_after_first_flush =
            dag_state.misbehavior_store().persisted_missing_proposals();
        drop(dag_state);

        // Recover from storage and flush again without new blocks
        let mut dag_state = DagState::new(context, store);
        dag_state.flush();

        // Persisted counts must NOT have doubled
        let persisted_after_restart_flush =
            dag_state.misbehavior_store().persisted_missing_proposals();
        assert_eq!(
            persisted_after_first_flush, persisted_after_restart_flush,
            "Persisted counts should not double on restart + flush"
        );
    }

    // ── Error classification tests ────────────────────────────────────────────

    /// Classifies `error` and returns `(provable_delta, unprovable_delta,
    /// bundle_part_delta)` for authority 0 (used as both peer and author so
    /// there is no peer-vs-author split to consider).
    fn classify_via_record(error: &ConsensusError) -> (u64, u64, u64) {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(&context);
        let authority = AuthorityIndex::new_for_test(0);
        store.record_faulty_block(authority, authority, error);
        let counts = store.in_memory.snapshot(0);
        (
            counts.faulty_blocks_provable,
            counts.faulty_blocks_unprovable,
            counts.invalid_bundle_parts,
        )
    }

    #[tokio::test]
    async fn test_provable_errors() {
        let cases: &[ConsensusError] = &[
            ConsensusError::TooManyAncestors(10, 5),
            ConsensusError::TooManyTransactions {
                count: 100,
                limit: 50,
            },
            ConsensusError::InvalidTransaction("bad tx".to_string()),
            // Bounds and structure checks that run only after the signature is
            // verified, so the signed header proves the author's fault.
            ConsensusError::TooManyAcknowledgments {
                count: 100,
                max: 50,
            },
            ConsensusError::TooManyCommitVotes {
                count: 100,
                max: 50,
            },
            ConsensusError::InvalidAcknowledgmentRound {
                acknowledgment: 5,
                block: 5,
            },
            ConsensusError::AcknowledgmentRoundTooOld {
                acknowledgment: 1,
                block: 10,
                gc_depth: 3,
            },
            ConsensusError::AncestorRoundTooOld {
                ancestor: 1,
                block: 10,
                gc_depth: 3,
            },
            ConsensusError::InvalidStrongVoteAuthority {
                index: AuthorityIndex::new_for_test(3),
                max: 4,
            },
            ConsensusError::StrongVoteLeaderNotInAncestors {
                block_round: 5,
                leader_round: 4,
                leader_authority: AuthorityIndex::new_for_test(1),
            },
        ];
        for e in cases {
            let (prov, unprov, bundle) = classify_via_record(e);
            assert_eq!(prov, 1, "expected provable for {e:?}");
            assert_eq!(unprov, 0, "expected no unprovable for {e:?}");
            assert_eq!(bundle, 0, "expected no bundle-part for {e:?}");
        }
    }

    #[tokio::test]
    async fn test_unprovable_errors() {
        let cases: &[ConsensusError] = &[
            ConsensusError::WrongEpoch {
                expected: 1,
                actual: 2,
            },
            ConsensusError::UnexpectedGenesisHeader,
            ConsensusError::UnexpectedAuthority(
                AuthorityIndex::new_for_test(0),
                AuthorityIndex::new_for_test(1),
            ),
            ConsensusError::MalformedHeader(bcs::Error::Custom("bad".to_string())),
            ConsensusError::MalformedSignature(FastCryptoError::InvalidSignature),
            ConsensusError::SignatureVerificationFailure(FastCryptoError::InvalidSignature),
            ConsensusError::TransactionCommitmentFailure {
                round: 3,
                author: AuthorityIndex::new_for_test(0),
                peer: AuthorityIndex::new_for_test(0),
            },
            ConsensusError::UnexpectedBlockHeaderForCommit {
                peer: AuthorityIndex::new_for_test(0),
                received: BlockRef::MIN,
            },
            ConsensusError::TooManyFetchedHeadersReturned {
                peer: AuthorityIndex::new_for_test(0),
                requested: 2,
                received: 3,
            },
        ];
        for e in cases {
            let (prov, unprov, bundle) = classify_via_record(e);
            assert_eq!(prov, 0, "expected no provable for {e:?}");
            assert_eq!(unprov, 1, "expected unprovable for {e:?}");
            assert_eq!(bundle, 0, "expected no bundle-part for {e:?}");
        }
    }

    #[tokio::test]
    async fn test_bundle_part_errors() {
        // Corrupt or invalid relayed bundle parts: charged to the relaying
        // peer under the dedicated bundle-part counter.
        let cases: &[ConsensusError] = &[
            ConsensusError::MalformedShard(bcs::Error::Custom("bad".to_string())),
            ConsensusError::TooBigHeaderRoundInABundle {
                header_round: 5,
                block_round: 5,
            },
            ConsensusError::TooBigShardRoundInABundle {
                shard_round: 5,
                block_round: 5,
            },
            ConsensusError::IncorrectShardProof {
                peer: AuthorityIndex::new_for_test(0),
                round: 3,
            },
            ConsensusError::UnrequestedHeaderOutOfWindow {
                peer: AuthorityIndex::new_for_test(0),
                author: AuthorityIndex::new_for_test(1),
                round: 10,
            },
            ConsensusError::SerializedShardTooLarge {
                peer: AuthorityIndex::new_for_test(0),
                size: 100,
                limit: 50,
            },
        ];
        for e in cases {
            let (prov, unprov, bundle) = classify_via_record(e);
            assert_eq!(prov, 0, "expected no provable for {e:?}");
            assert_eq!(unprov, 0, "expected no unprovable for {e:?}");
            assert_eq!(bundle, 1, "expected bundle-part for {e:?}");
        }
    }

    #[tokio::test]
    async fn test_untracked_errors() {
        // Subjective rejections, commit-chain inconsistencies, and fetch-shape
        // errors are not block header faults — they belong to separate metrics
        // and should not increment the faulty_blocks counters.
        let authority = AuthorityIndex::new_for_test(1);
        let cases: &[ConsensusError] = &[
            ConsensusError::BlockRejected {
                block_ref: BlockRef::MIN,
                reason: "test".to_string(),
            },
            // Commit-chain inconsistencies.
            ConsensusError::NoCommitReceived { peer: authority },
            ConsensusError::MalformedCommit(bcs::Error::Custom("bad".to_string())),
            // Fetch shortfalls (client-side truncation can produce them).
            ConsensusError::NotEnoughHeadersFetched {
                peer: authority,
                requested: 5,
                received: 3,
            },
            ConsensusError::FetchedTransactionsMismatch {
                peer: authority,
                expected: 3,
                received: 1,
            },
            // Version mismatch may be a benign upgrade misconfiguration, so it
            // is not charged as misbehavior.
            ConsensusError::WrongShardVersion { actual: "V1" },
        ];
        for e in cases {
            let (prov, unprov, bundle) = classify_via_record(e);
            assert_eq!(prov, 0, "expected no provable for {e:?}");
            assert_eq!(unprov, 0, "expected no unprovable (untracked) for {e:?}");
            assert_eq!(bundle, 0, "expected no bundle-part (untracked) for {e:?}");
        }
    }

    #[tokio::test]
    async fn test_snapshot_totals_sums_persisted_and_in_memory() {
        let committee_size = 4;
        let context = Arc::new(Context::new_for_test(committee_size).0);
        let store = MisbehaviorStore::new(&context);
        let a0 = AuthorityIndex::new_for_test(0);
        let a1 = AuthorityIndex::new_for_test(1);
        let provable = ConsensusError::TooManyAncestors(10, 5);

        // Seed in_memory provable counts for authority 0 (2) and 1 (1).
        store.record_faulty_block(a0, a0, &provable);
        store.record_faulty_block(a0, a0, &provable);
        store.record_faulty_block(a1, a1, &provable);

        // Flush faulty buffer for authority 0 into persisted; leave authority 1
        // unflushed so the snapshot must sum across both buckets.
        let _ =
            store.update_misbehavior_counts_on_eviction(a0, &BTreeSet::new(), 0, 0, 1, &context);

        // Record 3 more provable faults on authority 0 — these stay in_memory.
        for _ in 0..3 {
            store.record_faulty_block(a0, a0, &provable);
        }

        let snapshot = store.snapshot_totals();
        assert_eq!(snapshot.len(), committee_size);
        // Authority 0: 2 flushed into persisted + 3 still in_memory = 5 total.
        // Authority 1: 1 still in_memory (never flushed) = 1 total.
        // Untouched authorities are zero across both buckets.
        let provable_totals: Vec<u64> = snapshot
            .iter()
            .map(|c| c.as_v2().faulty_blocks_provable)
            .collect();
        assert_eq!(provable_totals, vec![5, 1, 0, 0]);
    }

    #[tokio::test]
    async fn test_recovery_upgrades_v1_rows() {
        // Persisted V1 rows (written before the bundle-part counter existed)
        // restore with `invalid_bundle_parts` at zero; V2 rows restore fully.
        let committee_size = 4;
        let context = Arc::new(Context::new_for_test(committee_size).0);
        let store = MisbehaviorStore::new(&context);

        let recovered = BTreeMap::from([
            (
                AuthorityIndex::new_for_test(0),
                MisbehaviorCounts::new_v1_for_test(1, 2, 3, 4),
            ),
            (
                AuthorityIndex::new_for_test(1),
                MisbehaviorCounts::new_v2_for_test(5, 6, 7, 8, 9),
            ),
        ]);
        let recent_refs_by_authority = vec![BTreeSet::new(); committee_size];
        let evicted_rounds = vec![0; committee_size];
        store.initialize_misbehavior_counts(
            recovered,
            &recent_refs_by_authority,
            &evicted_rounds,
            0,
            &context,
        );

        let totals = store.snapshot_totals();
        assert_eq!(
            *totals[0].as_v2(),
            MisbehaviorCountsV2 {
                faulty_blocks_provable: 1,
                faulty_blocks_unprovable: 2,
                missing_proposals: 3,
                equivocations: 4,
                invalid_bundle_parts: 0,
            }
        );
        assert_eq!(
            *totals[1].as_v2(),
            MisbehaviorCountsV2 {
                faulty_blocks_provable: 5,
                faulty_blocks_unprovable: 6,
                missing_proposals: 7,
                equivocations: 8,
                invalid_bundle_parts: 9,
            }
        );
    }

    #[tokio::test]
    async fn test_provable_fault_charges_both_author_and_serving_peer() {
        // When a peer serves us a block with a valid signature but a protocol
        // violation, both parties are at fault: the author for creating a bad
        // block (provable), and the serving peer for distributing it (unprovable).
        let e = ConsensusError::TooManyAncestors(10, 5);
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(&context);
        let author = AuthorityIndex::new_for_test(0);
        let peer = AuthorityIndex::new_for_test(1);
        store.record_faulty_block(peer, author, &e);
        let author_counts = store.in_memory.snapshot(0);
        let peer_counts = store.in_memory.snapshot(1);
        assert_eq!(author_counts.faulty_blocks_provable, 1);
        assert_eq!(author_counts.faulty_blocks_unprovable, 0);
        assert_eq!(peer_counts.faulty_blocks_provable, 0);
        assert_eq!(peer_counts.faulty_blocks_unprovable, 1);
    }

    #[tokio::test]
    async fn test_out_of_range_author_does_not_panic() {
        // Malicious peer sends a block claiming an author index outside the
        // committee: must not panic, and both provable+unprovable error paths
        // must fall back to charging the peer.
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(&context);
        let peer = AuthorityIndex::new_for_test(1);
        let bogus_author = AuthorityIndex::new_for_test(99);

        let provable = ConsensusError::TooManyAncestors(10, 5);
        store.record_faulty_block(peer, bogus_author, &provable);

        let unprovable = ConsensusError::InvalidAuthorityIndex {
            index: bogus_author,
            max: 3,
        };
        store.record_faulty_block(peer, bogus_author, &unprovable);

        let peer_counts = store.in_memory.snapshot(1);
        assert_eq!(peer_counts.faulty_blocks_provable, 0);
        assert_eq!(peer_counts.faulty_blocks_unprovable, 2);
    }
}
