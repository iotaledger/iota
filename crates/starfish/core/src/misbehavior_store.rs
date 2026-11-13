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
    block_header::{BlockRef, Round, Slot},
    context::Context,
    error::ConsensusError,
    metrics::NodeMetrics,
};

/// Per-authority misbehavior counters.
///
/// Two buckets track different lifecycle stages:
/// - `persisted`: missing proposals, equivocations and provable faults in rounds at or below the GC
///   round of the last commit at the last flush, plus flushed unprovable and bundle-part faults. No
///   header at or below that round can be accepted any more. Flush stores these counts in the same
///   batch as the commits, so after a restart the GC round of the last stored commit is the round
///   they reach.
/// - `in_memory`: missing proposals, equivocations and provable faults in rounds above that GC
///   round, recomputed on each flush from cached headers and recorded rounds, plus unprovable and
///   bundle-part faults not yet flushed.
pub(crate) struct MisbehaviorStore {
    in_memory: CommitteeMisbehaviorCounts,
    persisted: CommitteeMisbehaviorCounts,
    dag_fault_rounds: Vec<Mutex<DagFaultRounds>>,
}

/// The GC round and fault rounds of one authority. Kept under one lock, so no
/// fault round is added at or below the GC round.
#[derive(Default)]
struct DagFaultRounds {
    /// GC round of the last commit at the last flush.
    gc_round: Round,
    /// Far-future ceiling at the last flush. Provable faults in rounds above
    /// it are not recorded, so their set stays bounded.
    round_ceiling: Round,
    /// Rounds above `gc_round` in which a second verified header for an
    /// occupied slot was dropped at ingest.
    equivocating_rounds: BTreeSet<Round>,
    /// Rounds above `gc_round` in which a header signed by the authority, or
    /// the payload it commits to, failed a check.
    provable_fault_rounds: BTreeSet<Round>,
}

impl MisbehaviorStore {
    pub(crate) fn new(context: &Context) -> Self {
        let metrics = &context.metrics.node_metrics;
        Self {
            in_memory: CommitteeMisbehaviorCounts::new(&context.committee, metrics, "in_memory"),
            persisted: CommitteeMisbehaviorCounts::new(&context.committee, metrics, "persisted"),
            dag_fault_rounds: (0..context.committee.size())
                .map(|_| {
                    Mutex::new(DagFaultRounds {
                        round_ceiling: context.parameters.far_future_round_ceiling(0),
                        ..Default::default()
                    })
                })
                .collect(),
        }
    }

    /// Records that `author` signed more than one header for `round`.
    /// Idempotent. Rounds at or below the GC round or above `ceiling` are
    /// ignored.
    pub(crate) fn record_equivocating_slot(
        &self,
        author: AuthorityIndex,
        round: Round,
        ceiling: Round,
    ) {
        let Some(rounds) = self.dag_fault_rounds.get(author.value()) else {
            debug_fatal!("equivocating author {author} is outside the committee");
            return;
        };
        let mut rounds = rounds.lock().unwrap();
        if round > rounds.gc_round && round <= ceiling {
            rounds.equivocating_rounds.insert(round);
        }
    }

    /// Records that the author of `slot` signed a header, or a payload
    /// commitment, that failed a check. Counted once per round. Rounds at or
    /// below the GC round or above the far-future ceiling are ignored.
    fn record_provable_fault(&self, slot: Slot) {
        let author = slot.authority;
        let Some(rounds) = self.dag_fault_rounds.get(author.value()) else {
            debug_fatal!("author {author} of a verified header is outside the committee");
            return;
        };
        let mut rounds = rounds.lock().unwrap();
        if slot.round > rounds.gc_round
            && slot.round <= rounds.round_ceiling
            && rounds.provable_fault_rounds.insert(slot.round)
        {
            self.in_memory.record_block_fault_provable(author.value());
        }
    }

    /// Restores persisted counts from storage at startup and computes
    /// in-memory counts from the block refs already loaded into the DAG cache.
    /// `gc_round` is the GC round of the last stored commit; the cache must
    /// hold every header above it.
    pub(crate) fn initialize_misbehavior_counts(
        &self,
        recovered: BTreeMap<AuthorityIndex, MisbehaviorCounts>,
        recent_refs_by_authority: &[BTreeSet<BlockRef>],
        gc_round: Round,
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

            // Recorded rounds are not persisted, so after a restart only rounds
            // with more than one cached header count as equivocations, and no
            // provable fault is counted above the GC round.
            let mut rounds = self.dag_fault_rounds[idx].lock().unwrap();
            rounds.gc_round = gc_round;
            rounds.round_ceiling = context
                .parameters
                .far_future_round_ceiling(threshold_clock_round);
            self.set_in_memory_dag_faults(
                authority_index,
                &recent_refs_by_authority[idx],
                &rounds,
                threshold_clock_round,
            );
        }
    }

    /// Updates misbehavior counts for one authority during flush. `gc_round` is
    /// the GC round of the last commit. Must be called before cache eviction is
    /// applied, while the headers up to `gc_round` are still in `recent_refs`.
    /// Returns `Some(MisbehaviorCounts)` if persisted data changed and needs
    /// to be written to storage.
    pub(crate) fn update_misbehavior_counts(
        &self,
        authority_index: AuthorityIndex,
        recent_refs: &BTreeSet<BlockRef>,
        gc_round: Round,
        threshold_clock_round: Round,
        context: &Arc<Context>,
    ) -> Option<MisbehaviorCounts> {
        if threshold_clock_round == 0 {
            return None;
        }
        let idx = authority_index.value();
        if idx >= context.committee.size() {
            debug_fatal!("authority {authority_index} is outside the committee");
            return None;
        }

        // Move buffered faulty block counts to persisted.
        let mut persisted_changed = self.flush_faulty_block_buffer(idx);

        let mut rounds = self.dag_fault_rounds[idx].lock().unwrap();
        rounds.round_ceiling = context
            .parameters
            .far_future_round_ceiling(threshold_clock_round);
        if gc_round > rounds.gc_round {
            // Only fast sync moves the GC round to the threshold clock or
            // beyond, while the node does not follow the DAG. Its view of those
            // rounds is incomplete, so they are passed without being counted.
            if gc_round < threshold_clock_round {
                let start = rounds.gc_round + 1;
                let (eq, missing) = calculate_misbehavior_counts_for_range(
                    recent_refs.iter().map(|r| r.round).collect(),
                    &rounds.equivocating_rounds,
                    start,
                    gc_round,
                );
                let provable = rounds.provable_fault_rounds.range(..=gc_round).count() as u64;
                persisted_changed |= eq > 0 || missing > 0 || provable > 0;
                self.persisted.add_dag_faults(idx, missing, eq);
                self.persisted.add_block_faults(idx, provable, 0, 0);
            }
            rounds.equivocating_rounds = rounds.equivocating_rounds.split_off(&(gc_round + 1));
            rounds.provable_fault_rounds = rounds.provable_fault_rounds.split_off(&(gc_round + 1));
            rounds.gc_round = gc_round;
        }
        self.set_in_memory_dag_faults(authority_index, recent_refs, &rounds, threshold_clock_round);

        if persisted_changed {
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

    /// Sets the in-memory counts of one authority: missing proposals and
    /// equivocations in the rounds above its GC round and below the threshold
    /// clock, and its recorded provable faults.
    fn set_in_memory_dag_faults(
        &self,
        authority_index: AuthorityIndex,
        recent_refs: &BTreeSet<BlockRef>,
        rounds: &DagFaultRounds,
        threshold_clock_round: Round,
    ) {
        let start = rounds.gc_round + 1;
        let end = threshold_clock_round.saturating_sub(1);
        let (eq, missing) = calculate_misbehavior_counts_for_range(
            recent_refs.iter().map(|r| r.round).collect(),
            &rounds.equivocating_rounds,
            start,
            end,
        );
        let idx = authority_index.value();
        self.in_memory.set_dag_faults(idx, missing, eq);
        self.in_memory
            .set_provable_faults(idx, rounds.provable_fault_rounds.len() as u64);
    }

    /// Flush buffered unprovable and bundle-part counts from in_memory to
    /// persisted for one authority. Returns true if any counts were moved.
    fn flush_faulty_block_buffer(&self, idx: usize) -> bool {
        let (unprov, bundle) = self.in_memory.drain_block_faults(idx);
        if unprov == 0 && bundle == 0 {
            return false;
        }
        self.persisted.add_block_faults(idx, 0, unprov, bundle);
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

    /// Records a fault found while checking data from `peer`: a rejected block
    /// header, or a corrupt part of a relayed bundle. `header_slot` is the slot
    /// of the parsed header the error is about, if there is one.
    ///
    /// A provable error (valid signature, protocol violation) is charged to the
    /// header's author once per round, and to `peer` as an unprovable fault on
    /// every delivery when `peer` is not the author. Other errors charge
    /// `peer` only.
    pub(crate) fn record_faulty_block(
        &self,
        peer: AuthorityIndex,
        header_slot: Option<Slot>,
        error: &ConsensusError,
    ) {
        // `peer` is trusted (authenticated connection); the header's author is
        // attacker-controlled and may be out of committee range.
        let committee_size = self.in_memory.authorities.len();
        let peer_idx = peer.value();
        if peer_idx >= committee_size {
            debug_fatal!("peer {peer} is outside the committee");
            return;
        }
        match classify_block_error(error) {
            FaultType::Provable => {
                let Some(slot) = header_slot else {
                    debug_fatal!("provable error {error} recorded without its header");
                    self.in_memory.record_block_fault_unprovable(peer_idx);
                    return;
                };
                if slot.authority.value() >= committee_size {
                    // Can't credit a bogus author; charge the serving peer instead.
                    self.in_memory.record_block_fault_unprovable(peer_idx);
                    return;
                }
                self.record_provable_fault(slot);
                if peer != slot.authority {
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
    /// `authored` is the slot of a verified, author-signed block header that
    /// commits to this payload, if there is one. Only then are the invalid
    /// transactions provably the author's, and the author is charged a
    /// provable fault, counted once per round. Each peer in `relayers` is
    /// charged an unprovable fault for handing us a payload it should not have
    /// relayed; a relayer that is also the author is not double-charged when
    /// the author is charged provably.
    pub(crate) fn record_faulty_transactions(
        &self,
        authored: Option<Slot>,
        relayers: impl IntoIterator<Item = AuthorityIndex>,
    ) {
        let committee_size = self.in_memory.authorities.len();
        if let Some(slot) = authored {
            self.record_provable_fault(slot);
        }
        for peer in relayers {
            let idx = peer.value();
            if idx >= committee_size {
                debug_fatal!("relaying peer {peer} is outside the committee");
                continue;
            }
            if authored.is_some_and(|slot| slot.authority == peer) {
                continue;
            }
            self.in_memory.record_block_fault_unprovable(idx);
        }
    }

    /// Attributes a rejected fetch response to the peer that served it: the
    /// peer is both the sender and the only party the error can name. The
    /// block classifier decides the fault type, leaving transport failures
    /// untracked.
    pub(crate) fn record_fetch_fault(&self, peer: AuthorityIndex, error: &ConsensusError) {
        self.record_faulty_block(peer, None, error);
    }
}

/// Whether a block fault can be cryptographically proven.
enum FaultType {
    /// Block has a valid author signature but violates protocol rules.
    /// The signed block header itself is proof of misbehavior.
    Provable,
    /// Can't prove the fault from the signed data — either because the
    /// signature is bad or missing, or because the header is rejected by a
    /// pre-signature check (epoch / genesis / author-vs-peer mismatch) so its
    /// `author` field can't be trusted, or because the fault is in how the
    /// peer sent the block (stream order) rather than in the block itself.
    /// Charged to the sending peer, not the claimed author.
    Unprovable,
    /// Relayed bundle part (additional header or shard) that is corrupt,
    /// invalid, or one the peer had no right to relay, and isn't tied to a
    /// verified author. Charged to the sending peer. Errors in the bundle's
    /// framing and metadata, and a shard author outside the committee, are
    /// unprovable faults of the peer.
    BundlePart,
    /// Not counted as misbehavior.
    Untracked,
}

fn classify_block_error(error: &ConsensusError) -> FaultType {
    // Exhaustive on purpose: a new `ConsensusError` variant must be assigned a
    // fault type here rather than silently falling through to `Untracked`.
    match error {
        // Pre-signature / parsing errors — the header's author field can't
        // be trusted, so charge the sender, not the claimed author. A streamed
        // block that repeats or lowers the peer's own round is charged the
        // same way: the signed header proves authorship, not the send order.
        // So is a fetch response with more entries than the request allows,
        // with an entry larger than any valid one, or with commits after its
        // transactions. So is a commit-sync response an honest server never
        // sends: it serves only the certified commit chain it stores, with the
        // first commit's transactions whole. So is a reply to the last-own-header
        // probe with another author's header: the server returns only the
        // headers asked for.
        ConsensusError::WrongEpoch { .. }
        | ConsensusError::UnexpectedGenesisHeader
        | ConsensusError::UnexpectedAuthority(..)
        | ConsensusError::InvalidAuthorityIndex { .. }
        | ConsensusError::MalformedHeader(_)
        | ConsensusError::MalformedTransactions(_)
        | ConsensusError::MalformedSignature(_)
        | ConsensusError::SignatureVerificationFailure(_)
        | ConsensusError::DeserializationFailure(_)
        | ConsensusError::SerializedTransactionsTooLarge { .. }
        | ConsensusError::TransactionCommitmentFailure { .. }
        | ConsensusError::UnexpectedBlockHeaderForCommit { .. }
        | ConsensusError::TooManyFetchedHeadersReturned { .. }
        | ConsensusError::TooManyFetchedTransactionsReturned(_)
        | ConsensusError::TooManyCommitsFromPeer { .. }
        | ConsensusError::TooManyCommitVoteHeaders { .. }
        | ConsensusError::SerializedBlockHeaderTooLarge { .. }
        | ConsensusError::StreamedBlockRoundNotIncreasing { .. }
        | ConsensusError::CommitDataAfterTransactions { .. }
        | ConsensusError::MalformedCommit(_)
        | ConsensusError::WrongCommitVersion { .. }
        | ConsensusError::InvalidAuthorityIndexRequested { .. }
        | ConsensusError::UnexpectedStartCommit { .. }
        | ConsensusError::UnexpectedCommitSequence { .. }
        | ConsensusError::NotEnoughCommitVotes { .. }
        | ConsensusError::FetchedTransactionsMismatch { .. }
        | ConsensusError::TooManyCommitSyncEntries { .. }
        | ConsensusError::UnexpectedLastOwnHeader { .. } => FaultType::Unprovable,

        // Relayed bundle parts that are corrupt or invalid (additional-header
        // round, shard structure/proof) or that the peer had no right to relay
        // (a second shard for one slot). We know which peer relayed them but
        // can't tie them to a verified author.
        ConsensusError::MalformedShard(_)
        | ConsensusError::TooBigHeaderRoundInABundle { .. }
        | ConsensusError::TooBigShardRoundInABundle { .. }
        | ConsensusError::IncorrectShardProof { .. }
        | ConsensusError::UnrequestedHeaderOutOfWindow { .. }
        | ConsensusError::SerializedShardTooLarge { .. }
        | ConsensusError::SecondShardForSlot { .. } => FaultType::BundlePart,

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
        | ConsensusError::InvalidReferenceAuthority { .. }
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

        // Equivocation is counted once per slot when the header is dropped, so
        // the error that rejects the delivery carrying it is not charged again.
        ConsensusError::BlockHeaderEquivocation { .. }

        // Not attributable as misbehavior from a signed block alone: subjective
        // rejections, commit-sync inconsistencies, fetch-shape mismatches,
        // reconstruction and encoder failures, storage, network
        // transport, request-shape checks, local lifecycle signals, and version
        // mismatches that may stem from a benign upgrade misconfiguration rather
        // than a faulty peer.
        | ConsensusError::UnexpectedGenesisRequested { .. }
        | ConsensusError::NotEnoughHeadersFetched { .. }
        // Raised only when this node encodes its own data.
        | ConsensusError::SerializationFailure(_)
        // Transaction faults recorded against the serving peer where they are
        // detected, via `record_faulty_transactions` — deliberately not
        // tracked again here.
        | ConsensusError::UnrequestedTransactionFetched { .. }
        | ConsensusError::UnexpectedTransactionForCommit { .. }
        | ConsensusError::TransactionsOutOfCommitOrder { .. }
        | ConsensusError::TooManyAuthoritiesProvided(_)
        | ConsensusError::InvalidSizeOfHighestAcceptedRounds(..)
        | ConsensusError::TransactionCommitmentMismatch { .. }
        | ConsensusError::SynchronizerSaturated(_)
        | ConsensusError::TransactionSynchronizerSaturated
        | ConsensusError::BlockRejected { .. }
        | ConsensusError::EmptyMerkleTree
        | ConsensusError::MissingBlockHeader { .. }
        | ConsensusError::CommitRangeExceededAfterScanning { .. }
        | ConsensusError::NoCommitReceived { .. }
        | ConsensusError::TooManyCommitBytes { .. }
        | ConsensusError::InvalidCommitRange { .. }
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
        | ConsensusError::FailedToFetchBlockHeaders { .. }
        | ConsensusError::MissingVotingBlockHeaderInStorage { .. }
        | ConsensusError::WrongShardVersion { .. }
        | ConsensusError::WrongBlockHeaderVersion { .. }
        | ConsensusError::TransactionsNotAvailable { .. } => FaultType::Untracked,
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

    /// Atomically take the unprovable and bundle-part counters AND zero the
    /// matching gauges. The caller transfers the returned `(unprovable,
    /// bundle_parts)` into the persisted bucket via `add_block_faults`.
    fn drain_block_faults(&self, idx: usize) -> (u64, u64) {
        let mut c = self.authorities[idx].lock().unwrap();
        let unprov = std::mem::take(&mut c.faulty_blocks_unprovable);
        let bundle = std::mem::take(&mut c.invalid_bundle_parts);
        self.gauges[idx].block_fault_unprovable.set(0);
        self.gauges[idx].invalid_bundle_parts.set(0);
        (unprov, bundle)
    }

    /// Additive on block-fault counters and gauges.
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

    /// Overwrite the provable-fault counter and its gauge. Used on `in_memory`,
    /// where it is the number of recorded rounds above the GC round.
    fn set_provable_faults(&self, idx: usize, prov: u64) {
        let mut c = self.authorities[idx].lock().unwrap();
        c.faulty_blocks_provable = prov;
        self.gauges[idx].block_fault_provable.set(prov as i64);
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
    /// Used on `persisted` to add the rounds the GC round passed.
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
        self.dag_fault_rounds[author.value()]
            .lock()
            .unwrap()
            .equivocating_rounds
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
        commit::{Commit, CommitAPI, CommitV3},
        context::Context,
        dag_state::{DagState, DataSource},
        error::ConsensusError,
        leader_scoring::ReputationScores,
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
    async fn test_update_misbehavior_counts_edge_cases() {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(&context);
        let authority_index = AuthorityIndex::new_for_test(0);
        let recent_refs = BTreeSet::new();

        // threshold_clock_round=0 → always returns None
        let result = store.update_misbehavior_counts(authority_index, &recent_refs, 0, 0, &context);
        assert!(result.is_none());

        // The GC round did not advance AND no buffered faulty headers → None.
        let result = store.update_misbehavior_counts(authority_index, &recent_refs, 0, 5, &context);
        assert!(result.is_none());

        // The GC round advanced with empty refs → missing proposals counted.
        let result = store.update_misbehavior_counts(authority_index, &recent_refs, 3, 5, &context);
        assert_eq!(
            result,
            Some(MisbehaviorCounts::new_v2_for_test(0, 0, 3, 0, 0))
        );

        // The GC round reached the threshold clock, as in fast sync: rounds 4
        // to 6 are passed without being counted.
        let result = store.update_misbehavior_counts(authority_index, &recent_refs, 6, 5, &context);
        assert!(result.is_none());
        assert_eq!(store.persisted_missing_proposals()[0], 3);
        let result = store.update_misbehavior_counts(authority_index, &recent_refs, 6, 8, &context);
        assert!(result.is_none());
        assert_eq!(store.in_memory_missing_proposals()[0], 1);
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
        store.record_equivocating_slot(author, 5, Round::MAX);
        store.record_equivocating_slot(author, 5, Round::MAX);
        store.record_equivocating_slot(author, 7, Round::MAX);

        // No GC advance: the flags show up in the in-memory window and no
        // block fault is charged.
        let result = store.update_misbehavior_counts(author, &recent_refs, 0, 10, &context);
        assert!(result.is_none());
        assert_eq!(store.in_memory_equivocations()[idx], 2);
        assert_eq!(store.in_memory_faulty_blocks_provable()[idx], 0);

        // The GC round advances to 6: the flag at round 5 folds into persisted
        // and is pruned; round 7 stays in the in-memory window.
        let result = store.update_misbehavior_counts(author, &recent_refs, 6, 10, &context);
        assert!(result.is_some());
        assert_eq!(store.persisted_equivocations()[idx], 1);
        assert_eq!(store.in_memory_equivocations()[idx], 1);
        assert_eq!(store.equivocating_rounds(author), vec![7]);

        // The next advance folds the remaining flag; nothing is counted twice.
        let result = store.update_misbehavior_counts(author, &recent_refs, 9, 10, &context);
        assert!(result.is_some());
        assert_eq!(store.persisted_equivocations()[idx], 2);
        assert_eq!(store.in_memory_equivocations()[idx], 0);
        assert!(store.equivocating_rounds(author).is_empty());

        // Flags at or below the GC round, or above the ceiling, are
        // ignored.
        store.record_equivocating_slot(author, 9, Round::MAX);
        store.record_equivocating_slot(author, 21, 20);
        store.record_equivocating_slot(author, 20, 20);
        assert_eq!(store.equivocating_rounds(author), vec![20]);

        // A flag with no cached header is counted and dropped once the GC
        // round passes it.
        let result = store.update_misbehavior_counts(author, &BTreeSet::new(), 20, 21, &context);
        assert!(result.is_some());
        assert_eq!(store.persisted_equivocations()[idx], 3);
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
            None,
            &ConsensusError::WrongEpoch {
                expected: 1,
                actual: 2,
            },
        );
        store.record_faulty_block(
            authority,
            None,
            &ConsensusError::MalformedShard(bcs::Error::Custom("bad".to_string())),
        );

        let result = store.update_misbehavior_counts(authority, &BTreeSet::new(), 0, 1, &context);

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

    /// Repeated restarts give the totals of an uninterrupted run, also when
    /// the cache holds fewer rounds than the GC round trails the last commit
    /// by, and for an authority with no header before the restart.
    #[tokio::test]
    async fn test_restarts_keep_totals() {
        for (cached_rounds, gc_depth) in [(5, 5), (20, 2), (5, 0)] {
            let (mut context, _) = Context::new_for_test(4);
            context.parameters.dag_state_cached_rounds = cached_rounds;
            context.protocol_config.set_gc_depth_for_testing(gc_depth);
            let context = Arc::new(context);

            // Authority 3 is offline for rounds 1-20, authority 0 skips rounds
            // 25-27 and authority 1 equivocates at round 31.
            let a = AuthorityIndex::new_for_test;
            let mut dag_builder = DagBuilder::new(context.clone());
            dag_builder
                .layers(1..=20)
                .authorities(vec![a(3)])
                .skip_block()
                .build();
            dag_builder.layers(21..=24).build();
            dag_builder
                .layers(25..=27)
                .authorities(vec![a(0)])
                .skip_block()
                .build();
            dag_builder.layers(28..=30).build();
            dag_builder
                .layers(31..=31)
                .authorities(vec![a(1)])
                .equivocate(1)
                .build();
            dag_builder.layers(32..=40).build();
            let commits: Vec<_> = dag_builder
                .get_sub_dag_and_commits(1..=40)
                .into_iter()
                .map(|(_, commit)| commit)
                .collect();

            let feed = |dag_state: &mut DagState, rounds: std::ops::RangeInclusive<Round>| {
                dag_state.accept_block_headers(
                    dag_builder.block_headers(rounds.clone()),
                    DataSource::Test,
                );
                for commit in commits
                    .iter()
                    .filter(|commit| rounds.contains(&commit.leader().round))
                {
                    dag_state.add_commit(commit.clone());
                }
                dag_state.flush();
            };

            let mut uninterrupted = DagState::new(context.clone(), Arc::new(MemStore::new()));
            feed(&mut uninterrupted, 1..=40);
            let expected = vec![
                MisbehaviorCounts::new_v2_for_test(0, 0, 3, 0, 0),
                MisbehaviorCounts::new_v2_for_test(0, 0, 0, 1, 0),
                MisbehaviorCounts::new_v2_for_test(0, 0, 0, 0, 0),
                MisbehaviorCounts::new_v2_for_test(0, 0, 20, 0, 0),
            ];
            assert_eq!(
                uninterrupted.misbehavior_store().snapshot_totals(),
                expected,
                "config {cached_rounds}/{gc_depth}"
            );

            let store = Arc::new(MemStore::new());
            let mut dag_state = DagState::new(context.clone(), store.clone());
            for rounds in [1..=10, 11..=20, 21..=30, 31..=40] {
                feed(&mut dag_state, rounds);
                let before = dag_state.misbehavior_store().snapshot_totals();
                for _ in 0..2 {
                    drop(dag_state);
                    dag_state = DagState::new(context.clone(), store.clone());
                    dag_state.flush();
                    assert_eq!(
                        dag_state.misbehavior_store().snapshot_totals(),
                        before,
                        "config {cached_rounds}/{gc_depth}"
                    );
                }
            }
            assert_eq!(
                dag_state.misbehavior_store().snapshot_totals(),
                expected,
                "config {cached_rounds}/{gc_depth}"
            );
        }
    }

    // ── Error classification tests ────────────────────────────────────────────

    /// Classifies `error` and returns `(provable_delta, unprovable_delta,
    /// bundle_part_delta)` for authority 0 (used as both peer and author so
    /// there is no peer-vs-author split to consider).
    fn classify_via_record(error: &ConsensusError) -> (u64, u64, u64) {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(&context);
        let authority = AuthorityIndex::new_for_test(0);
        store.record_faulty_block(authority, Some(Slot::new(1, authority)), error);
        let counts = store.in_memory.snapshot(0);
        (
            counts.faulty_blocks_provable,
            counts.faulty_blocks_unprovable,
            counts.invalid_bundle_parts,
        )
    }

    /// A fetch response the collector rejected is the serving peer's
    /// unprovable fault; a transport failure is nobody's.
    #[tokio::test]
    async fn fetch_faults_are_charged_to_the_serving_peer() {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(&context);
        let peer = AuthorityIndex::new_for_test(1);

        store.record_fetch_fault(
            peer,
            &ConsensusError::TooManyFetchedTransactionsReturned(peer),
        );
        store.record_fetch_fault(
            peer,
            &ConsensusError::SerializedTransactionsTooLarge { size: 2, limit: 1 },
        );
        store.record_fetch_fault(
            peer,
            &ConsensusError::TooManyFetchedHeadersReturned {
                peer,
                requested: 1,
                received: 2,
            },
        );
        store.record_fetch_fault(
            peer,
            &ConsensusError::TooManyCommitsFromPeer {
                peer,
                count: 3,
                limit: 2,
            },
        );
        store.record_fetch_fault(
            peer,
            &ConsensusError::TooManyCommitVoteHeaders {
                peer,
                count: 9,
                limit: 8,
            },
        );
        store.record_fetch_fault(
            peer,
            &ConsensusError::SerializedBlockHeaderTooLarge {
                peer,
                size: 2,
                limit: 1,
            },
        );
        store.record_fetch_fault(peer, &ConsensusError::NetworkRequest("cut".to_string()));

        let counts = store.in_memory.snapshot(peer.value());
        assert_eq!(counts.faulty_blocks_unprovable, 6);
        assert_eq!(counts.faulty_blocks_provable, 0);
        assert_eq!(counts.invalid_bundle_parts, 0);
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
            ConsensusError::InvalidReferenceAuthority {
                index: AuthorityIndex::new_for_test(4),
                max: 4,
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
            ConsensusError::StreamedBlockRoundNotIncreasing {
                peer: AuthorityIndex::new_for_test(0),
                round: 4,
                last_round: 5,
            },
            ConsensusError::TooManyCommitSyncEntries {
                peer: AuthorityIndex::new_for_test(0),
                limit: 4,
            },
            // Commit lists an honest server never sends.
            ConsensusError::MalformedCommit(bcs::Error::Custom("bad".to_string())),
            ConsensusError::WrongCommitVersion { actual: "V2" },
            ConsensusError::InvalidAuthorityIndexRequested {
                index: AuthorityIndex::new_for_test(4),
                max: 4,
                peer: AuthorityIndex::new_for_test(0),
            },
            ConsensusError::UnexpectedStartCommit {
                peer: AuthorityIndex::new_for_test(0),
                start: 1,
                commit: Box::new(Commit::V3(CommitV3::default())),
            },
            ConsensusError::UnexpectedCommitSequence {
                peer: AuthorityIndex::new_for_test(0),
                prev_commit: Box::new(Commit::V3(CommitV3::default())),
                curr_commit: Box::new(Commit::V3(CommitV3::default())),
            },
            ConsensusError::NotEnoughCommitVotes {
                stake: 0,
                peer: AuthorityIndex::new_for_test(0),
                commit: Box::new(Commit::V3(CommitV3::default())),
            },
            ConsensusError::FetchedTransactionsMismatch {
                peer: AuthorityIndex::new_for_test(0),
                expected: 3,
                received: 1,
            },
            // Another author's header in a reply to the last-own-header probe.
            ConsensusError::UnexpectedLastOwnHeader {
                index: AuthorityIndex::new_for_test(0),
                block_ref: BlockRef::MIN,
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
            ConsensusError::SecondShardForSlot {
                peer: AuthorityIndex::new_for_test(0),
                author: AuthorityIndex::new_for_test(1),
                round: 10,
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
        // Subjective rejections, commit lists an honest server can send, and
        // fetch-shape errors are not block header faults — they belong to
        // separate metrics and should not increment the faulty_blocks counters.
        let authority = AuthorityIndex::new_for_test(1);
        let cases: &[ConsensusError] = &[
            ConsensusError::BlockRejected {
                block_ref: BlockRef::MIN,
                reason: "test".to_string(),
            },
            // Commit lists an honest server can send.
            ConsensusError::NoCommitReceived { peer: authority },
            ConsensusError::TooManyCommitBytes {
                peer: authority,
                size: 2,
                limit: 1,
            },
            // Fetch shortfall (client-side truncation can produce it).
            ConsensusError::NotEnoughHeadersFetched {
                peer: authority,
                requested: 5,
                received: 3,
            },
            // Version mismatch may be a benign upgrade misconfiguration, so it
            // is not charged as misbehavior.
            ConsensusError::WrongShardVersion { actual: "V1" },
            // Raised only when this node encodes its own data.
            ConsensusError::SerializationFailure(bcs::Error::Custom("bad".to_string())),
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
        for round in [1, 2] {
            store.record_faulty_block(a0, Some(Slot::new(round, a0)), &provable);
        }
        store.record_faulty_block(a1, Some(Slot::new(1, a1)), &provable);

        // Move authority 0's rounds into persisted; leave authority 1 in memory
        // so the snapshot must sum across both buckets.
        let _ = store.update_misbehavior_counts(a0, &BTreeSet::new(), 2, 3, &context);

        // Record 3 more provable faults on authority 0 — these stay in_memory.
        for round in [3, 4, 5] {
            store.record_faulty_block(a0, Some(Slot::new(round, a0)), &provable);
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
        store.initialize_misbehavior_counts(recovered, &recent_refs_by_authority, 0, 0, &context);

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
        store.record_faulty_block(peer, Some(Slot::new(1, author)), &e);
        let author_counts = store.in_memory.snapshot(0);
        let peer_counts = store.in_memory.snapshot(1);
        assert_eq!(author_counts.faulty_blocks_provable, 1);
        assert_eq!(author_counts.faulty_blocks_unprovable, 0);
        assert_eq!(peer_counts.faulty_blocks_provable, 0);
        assert_eq!(peer_counts.faulty_blocks_unprovable, 1);
    }

    #[tokio::test]
    async fn test_provable_faults_count_once_per_round() {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = MisbehaviorStore::new(&context);
        let author = AuthorityIndex::new_for_test(0);
        let peer = AuthorityIndex::new_for_test(1);
        let e = ConsensusError::TooManyAncestors(10, 5);
        let ceiling = context.parameters.far_future_round_ceiling(0);
        let provable = |store: &MisbehaviorStore| {
            let idx = author.value();
            (
                store.persisted.snapshot(idx).faulty_blocks_provable,
                store.in_memory.snapshot(idx).faulty_blocks_provable,
            )
        };

        for _ in 0..3 {
            store.record_faulty_block(peer, Some(Slot::new(5, author)), &e);
        }
        store.record_faulty_transactions(Some(Slot::new(5, author)), [peer]);
        store.record_faulty_block(peer, Some(Slot::new(6, author)), &e);
        store.record_faulty_block(peer, Some(Slot::new(ceiling + 1, author)), &e);
        assert_eq!(provable(&store), (0, 2));
        assert_eq!(
            store
                .in_memory
                .snapshot(peer.value())
                .faulty_blocks_unprovable,
            6
        );

        // The GC round passes round 5: it is stored and not counted again.
        let stored = store
            .update_misbehavior_counts(author, &BTreeSet::new(), 5, 7, &context)
            .unwrap();
        store.record_faulty_block(peer, Some(Slot::new(5, author)), &e);
        assert_eq!(provable(&store), (1, 1));

        // After a restart, round 6 was never stored, so it counts once.
        let store = MisbehaviorStore::new(&context);
        store.initialize_misbehavior_counts(
            BTreeMap::from([(author, stored)]),
            &vec![BTreeSet::new(); 4],
            5,
            7,
            &context,
        );
        for round in [5, 6, 6] {
            store.record_faulty_block(peer, Some(Slot::new(round, author)), &e);
        }
        assert_eq!(provable(&store), (1, 1));

        // Rounds that fast sync passes are dropped without being counted.
        store.record_faulty_block(peer, Some(Slot::new(8, author)), &e);
        store.update_misbehavior_counts(author, &BTreeSet::new(), 20, 20, &context);
        assert_eq!(provable(&store), (1, 0));
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
        store.record_faulty_block(peer, Some(Slot::new(1, bogus_author)), &provable);

        let unprovable = ConsensusError::InvalidAuthorityIndex {
            index: bogus_author,
            max: 3,
        };
        store.record_faulty_block(peer, Some(Slot::new(1, bogus_author)), &unprovable);

        let peer_counts = store.in_memory.snapshot(1);
        assert_eq!(peer_counts.faulty_blocks_provable, 0);
        assert_eq!(peer_counts.faulty_blocks_unprovable, 2);
    }

    /// An author whose last committed header is not fetched for
    /// reinitialization after fast sync has no header in memory. Its rounds
    /// must still count at most once, and never when its header is stored.
    #[tokio::test]
    async fn test_no_double_counting_after_fast_sync() {
        const CACHED_ROUNDS: Round = 5;
        const GC_DEPTH: Round = 2;
        let committee_size = 4;
        let (mut context, _) = Context::new_for_test(committee_size);
        context.parameters.dag_state_cached_rounds = CACHED_ROUNDS;
        context.protocol_config.set_gc_depth_for_testing(GC_DEPTH);
        let context = Arc::new(context);
        let store = Arc::new(MemStore::new());
        let mut dag_state = DagState::new(context.clone(), store.clone());

        // Authority 3 skips rounds 3-4 and is offline for rounds 21-40.
        let offline = AuthorityIndex::new_for_test(3);
        let mut dag_builder = DagBuilder::new(context.clone());
        dag_builder.layers(1..=2).build();
        dag_builder
            .layers(3..=4)
            .authorities(vec![offline])
            .skip_block()
            .build();
        dag_builder.layers(5..=20).build();
        dag_builder
            .layers(21..=40)
            .authorities(vec![offline])
            .skip_block()
            .build();
        dag_builder.layers(41..=60).build();

        let mut commits_before_sync = Vec::new();
        let mut synced_commits = Vec::new();
        let mut commits_after_sync = Vec::new();
        for (_, commit) in dag_builder.get_sub_dag_and_commits(1..=60) {
            match commit.leader().round {
                ..=10 => commits_before_sync.push(commit),
                11..=40 => synced_commits.push(commit),
                _ => commits_after_sync.push(commit),
            }
        }

        // The node follows the DAG up to round 10.
        dag_state.accept_block_headers(dag_builder.block_headers(1..=10), DataSource::Test);
        for commit in commits_before_sync {
            dag_state.add_commit(commit);
        }
        dag_state.flush();
        let gc_round_before_sync = dag_state.gc_round_for_last_commit();

        // Fast sync stores the commits up to round 40 without their headers.
        // The leader schedule was last updated at the last synced commit.
        dag_state.set_fast_sync_ongoing_flag(true);
        for commit in synced_commits {
            dag_state.add_commit(commit);
        }
        dag_state.add_commit_info(ReputationScores::default());
        dag_state.flush();
        let gc_round_after_sync = dag_state.gc_round_for_last_commit();

        // A restart during fast sync keeps the totals, although the flush that
        // moved the GC round wrote no counts.
        let totals_during_sync = dag_state.misbehavior_store().snapshot_totals();
        drop(dag_state);
        let mut dag_state = DagState::new(context, store);
        dag_state.flush();
        assert_eq!(
            dag_state.misbehavior_store().snapshot_totals(),
            totals_during_sync
        );

        // Reinitialization fetches the headers of the last commits, which hold
        // none of authority 3.
        let fetched_refs: BTreeSet<BlockRef> = dag_state
            .get_block_refs_for_recent_commits(10)
            .into_iter()
            .collect();
        assert!(fetched_refs.iter().all(|r| r.author != offline));
        let fetched_headers = dag_builder
            .block_headers(11..=40)
            .into_iter()
            .filter(|header| fetched_refs.contains(&header.reference()))
            .collect();
        dag_state.accept_block_headers(fetched_headers, DataSource::FastCommitSyncer);
        dag_state.set_fast_sync_ongoing_flag(false);
        dag_state.flush();
        dag_state.reinitialize();
        dag_state.flush();

        // Authority 3 is back.
        dag_state.accept_block_headers(dag_builder.block_headers(41..=60), DataSource::Test);
        for commit in commits_after_sync {
            dag_state.add_commit(commit);
        }
        dag_state.flush();

        // Absent rounds are counted once, except those that fast sync passed.
        let gc_round = dag_state.gc_round_for_last_commit();
        let expected = (3..=4)
            .filter(|&round| round <= gc_round_before_sync)
            .chain((21..=40).filter(|&round| round > gc_round_after_sync && round <= gc_round))
            .count() as u64;
        assert_eq!(
            dag_state.misbehavior_store().persisted_missing_proposals()[offline],
            expected
        );
    }
}
