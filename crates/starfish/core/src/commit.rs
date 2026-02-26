// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
    ops::{Deref, Range, RangeInclusive},
    sync::Arc,
};

use bytes::Bytes;
use enum_dispatch::enum_dispatch;
use fastcrypto::hash::{Digest, HashFunction as _};
use serde::{Deserialize, Serialize};
use starfish_config::{AuthorityIndex, DIGEST_LENGTH, DefaultHashFunction};
use tracing::debug;

use crate::{
    block_header::{
        BlockHeaderAPI, BlockRef, BlockTimestampMs, Round, Slot, VerifiedBlockHeader,
        VerifiedTransactions,
    },
    context::Context,
    leader_scoring::ReputationScores,
    storage::Store,
    transaction_ref::{GenericTransactionRef, GenericTransactionRefAPI as _, TransactionRef},
};

/// Index of a commit among all consensus commits.
pub type CommitIndex = u32;

pub(crate) const GENESIS_COMMIT_INDEX: CommitIndex = 0;

/// One wave consists of one leader round, one voting round, and one certifying
/// round.
pub(crate) const WAVE_LENGTH: Round = 3;

/// The consensus protocol operates in 'waves'. Each wave is composed of a
/// leader round, at least one voting round, and one certifying round.
pub(crate) type WaveNumber = u32;

/// [`Commit`] summarizes [`CommittedSubDag`] for storage and network
/// communications.
///
/// Validators should be able to reconstruct a sequence of CommittedSubDag from
/// the corresponding Commit and blocks referenced in the Commit.
/// A field must meet these requirements to be added to Commit:
/// - helps with recovery locally and for peers catching up.
/// - cannot be derived from a sequence of Commits and other persisted values.
///
/// For example, transactions in blocks should not be included in Commit,
/// because they can be retrieved from blocks specified in Commit. Last
/// committed round per authority also should not be included, because it can be
/// derived from the latest value in storage and the additional sequence of
/// Commits.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[enum_dispatch(CommitAPI)]
pub(crate) enum Commit {
    V1(CommitV1),
    V2(CommitV2),
}

impl Commit {
    /// Create a new commit. The variant (V1 or V2) is determined by the
    /// consensus_fast_commit_sync protocol flag.
    pub(crate) fn new(
        context: &Arc<Context>,
        index: CommitIndex,
        previous_digest: CommitDigest,
        timestamp_ms: BlockTimestampMs,
        leader: BlockRef,
        blocks: Vec<BlockRef>,
        committed_transactions: Vec<GenericTransactionRef>,
        reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
    ) -> Self {
        if context.protocol_config.consensus_fast_commit_sync() {
            debug!("Creating CommitV2 as consensus_fast_commit_sync is enabled");
            // Extract TransactionRefs from GenericTransactionRef
            let transaction_refs: Vec<TransactionRef> = committed_transactions
                .into_iter()
                .map(|gen_ref| match gen_ref {
                    GenericTransactionRef::TransactionRef(tr) => tr,
                    GenericTransactionRef::BlockRef(_) => {
                        panic!("Expected TransactionRef when consensus_fast_commit_sync is enabled")
                    }
                })
                .collect();

            Commit::V2(CommitV2 {
                index,
                previous_digest,
                timestamp_ms,
                leader,
                block_headers: blocks,
                committed_transactions: transaction_refs,
                reputation_scores_desc,
            })
        } else {
            debug!("Creating CommitV1 as consensus_fast_commit_sync is disabled");
            // Extract BlockRefs from GenericTransactionRef
            let block_refs: Vec<BlockRef> = committed_transactions
                .into_iter()
                .map(|gen_ref| match gen_ref {
                    GenericTransactionRef::BlockRef(br) => br,
                    GenericTransactionRef::TransactionRef(_) => {
                        panic!("Expected BlockRef when consensus_fast_commit_sync is disabled")
                    }
                })
                .collect();
            Commit::V1(CommitV1 {
                index,
                previous_digest,
                timestamp_ms,
                leader,
                blocks,
                committed_transactions: block_refs,
            })
        }
    }

    pub(crate) fn serialize(&self) -> Result<Bytes, bcs::Error> {
        let bytes = bcs::to_bytes(self)?;
        Ok(bytes.into())
    }
}

/// Accessors to Commit info.
#[enum_dispatch]
pub(crate) trait CommitAPI {
    fn round(&self) -> Round;
    fn index(&self) -> CommitIndex;
    fn previous_digest(&self) -> CommitDigest;
    fn timestamp_ms(&self) -> BlockTimestampMs;
    fn leader(&self) -> BlockRef;
    fn block_headers(&self) -> &[BlockRef];
    fn committed_transactions(&self) -> Vec<GenericTransactionRef>;
    fn reputation_scores(&self) -> &[(AuthorityIndex, u64)];
}

/// Specifies one consensus commit.
/// It is stored on disk, so it does not contain blocks which are stored
/// individually.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub(crate) struct CommitV1 {
    /// Index of the commit.
    /// First commit after genesis has an index of 1, then every next commit has
    /// an index incremented by 1.
    index: CommitIndex,
    /// Digest of the previous commit.
    /// Set to CommitDigest::MIN for the first commit after genesis.
    previous_digest: CommitDigest,
    /// Timestamp of the commit, max of the timestamp of the leader block and
    /// previous Commit timestamp.
    timestamp_ms: BlockTimestampMs,
    /// A reference to the commit leader.
    leader: BlockRef,
    /// Refs to committed blocks, in the commit order.
    blocks: Vec<BlockRef>,
    /// Refs to transactions in blocks for which quorum of acknowledgments has
    /// been collected in this and past commits.
    committed_transactions: Vec<BlockRef>,
}

impl CommitAPI for CommitV1 {
    fn round(&self) -> Round {
        self.leader.round
    }

    fn index(&self) -> CommitIndex {
        self.index
    }

    fn previous_digest(&self) -> CommitDigest {
        self.previous_digest
    }

    fn timestamp_ms(&self) -> BlockTimestampMs {
        self.timestamp_ms
    }

    fn leader(&self) -> BlockRef {
        self.leader
    }

    fn block_headers(&self) -> &[BlockRef] {
        &self.blocks
    }

    fn committed_transactions(&self) -> Vec<GenericTransactionRef> {
        self.committed_transactions
            .iter()
            .map(|b| GenericTransactionRef::BlockRef(*b))
            .collect()
    }

    fn reputation_scores(&self) -> &[(AuthorityIndex, u64)] {
        // CommitV1 does not have reputation scores
        &[]
    }
}

/// Specifies one consensus commit.
/// It is stored on disk, so it does not contain blocks which are stored
/// individually.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub(crate) struct CommitV2 {
    /// Index of the commit.
    /// First commit after genesis has an index of 1, then every next commit has
    /// an index incremented by 1.
    index: CommitIndex,
    /// Digest of the previous commit.
    /// Set to CommitDigest::MIN for the first commit after genesis.
    previous_digest: CommitDigest,
    /// Timestamp of the commit, max of the timestamp of the leader block and
    /// previous Commit timestamp.
    timestamp_ms: BlockTimestampMs,
    /// A reference to the commit leader.
    leader: BlockRef,
    /// Refs to committed headers, in the commit order.
    block_headers: Vec<BlockRef>,
    /// Refs to transactions in blocks for which quorum of acknowledgments has
    /// been collected in this and past commits.
    committed_transactions: Vec<TransactionRef>,
    /// Optional scores that are provided as part of the consensus output to
    /// IOTA that can then be used by IOTA for future transaction submission to
    /// consensus.
    reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
}

impl CommitAPI for CommitV2 {
    fn round(&self) -> Round {
        self.leader.round
    }

    fn index(&self) -> CommitIndex {
        self.index
    }

    fn previous_digest(&self) -> CommitDigest {
        self.previous_digest
    }

    fn timestamp_ms(&self) -> BlockTimestampMs {
        self.timestamp_ms
    }

    fn leader(&self) -> BlockRef {
        self.leader
    }

    fn block_headers(&self) -> &[BlockRef] {
        &self.block_headers
    }

    fn committed_transactions(&self) -> Vec<GenericTransactionRef> {
        self.committed_transactions
            .iter()
            .map(|t| GenericTransactionRef::TransactionRef(*t))
            .collect()
    }

    fn reputation_scores(&self) -> &[(AuthorityIndex, u64)] {
        &self.reputation_scores_desc
    }
}

/// A commit is trusted when it is produced locally or certified by a quorum of
/// authorities. Blocks referenced by TrustedCommit are assumed to be valid.
/// Only trusted Commit can be sent to execution.
///
/// Note: clone() is relatively cheap with the underlying data refcounted.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrustedCommit {
    inner: Arc<Commit>,

    // Cached digest and serialized value, to avoid re-computing these values.
    digest: CommitDigest,
    serialized: Bytes,
}

impl TrustedCommit {
    pub(crate) fn new_trusted(commit: Commit, serialized: Bytes) -> Self {
        let digest = Self::compute_digest(&serialized);
        Self {
            inner: Arc::new(commit),
            digest,
            serialized,
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(
        context: &Arc<Context>,
        index: CommitIndex,
        previous_digest: CommitDigest,
        timestamp_ms: BlockTimestampMs,
        leader: BlockRef,
        blocks: Vec<BlockRef>,
        committed_transactions: Vec<GenericTransactionRef>,
    ) -> Self {
        let commit = Commit::new(
            context,
            index,
            previous_digest,
            timestamp_ms,
            leader,
            blocks,
            committed_transactions,
            vec![],
        );
        let serialized = commit.serialize().unwrap();
        Self::new_trusted(commit, serialized)
    }

    pub(crate) fn reference(&self) -> CommitRef {
        CommitRef {
            index: self.index(),
            digest: self.digest(),
        }
    }

    pub(crate) fn digest(&self) -> CommitDigest {
        self.digest
    }

    pub(crate) fn serialized(&self) -> &Bytes {
        &self.serialized
    }

    pub(crate) fn compute_digest(serialized: &[u8]) -> CommitDigest {
        let mut hasher = DefaultHashFunction::new();
        hasher.update(serialized);
        CommitDigest(hasher.finalize().into())
    }
}

/// Allow easy access on the underlying Commit.
impl Deref for TrustedCommit {
    type Target = Commit;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// `CertifiedCommits` keeps the synchronized certified commits along with the
/// corresponding votes received from the peer that provided these commits.
/// The `votes` contain the blocks as those provided by the peer, and certify
/// the tip of the synced commits.
#[derive(Clone, Debug)]
pub(crate) struct CertifiedCommits {
    commits: Vec<CertifiedCommit>,
}

impl CertifiedCommits {
    pub(crate) fn new(commits: Vec<CertifiedCommit>) -> Self {
        Self { commits }
    }

    pub(crate) fn commits(&self) -> &[CertifiedCommit] {
        &self.commits
    }
}

/// A commit that has been synced and certified by a quorum of authorities.
#[derive(Clone, Debug)]
pub(crate) struct CertifiedCommit {
    commit: Arc<TrustedCommit>,
    verified_block_headers: Vec<VerifiedBlockHeader>,
    verified_transactions: Vec<VerifiedTransactions>,
}

impl CertifiedCommit {
    pub(crate) fn new_certified(
        commit: TrustedCommit,
        verified_block_headers: Vec<VerifiedBlockHeader>,
        verified_transactions: Vec<VerifiedTransactions>,
    ) -> Self {
        Self {
            commit: Arc::new(commit),
            verified_block_headers,
            verified_transactions,
        }
    }

    pub fn block_headers(&self) -> &[VerifiedBlockHeader] {
        &self.verified_block_headers
    }

    pub fn transactions(&self) -> &[VerifiedTransactions] {
        &self.verified_transactions
    }
}

impl Deref for CertifiedCommit {
    type Target = TrustedCommit;

    fn deref(&self) -> &Self::Target {
        &self.commit
    }
}

/// Digest of a consensus commit.
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommitDigest([u8; DIGEST_LENGTH]);

impl CommitDigest {
    /// Lexicographic min & max digest.
    pub const MIN: Self = Self([u8::MIN; DIGEST_LENGTH]);
    pub const MAX: Self = Self([u8::MAX; DIGEST_LENGTH]);

    pub fn into_inner(self) -> [u8; starfish_config::DIGEST_LENGTH] {
        self.0
    }
}

impl Hash for CommitDigest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.0[..8]);
    }
}

impl From<CommitDigest> for Digest<{ DIGEST_LENGTH }> {
    fn from(hd: CommitDigest) -> Self {
        Digest::new(hd.0)
    }
}

impl fmt::Display for CommitDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.0)
                .get(0..4)
                .ok_or(fmt::Error)?
        )
    }
}

impl fmt::Debug for CommitDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.0)
        )
    }
}

/// Uniquely identifies a commit with its index and digest.
#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommitRef {
    pub index: CommitIndex,
    pub digest: CommitDigest,
}

impl CommitRef {
    pub fn new(index: CommitIndex, digest: CommitDigest) -> Self {
        Self { index, digest }
    }
}

impl fmt::Display for CommitRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "C{}({})", self.index, self.digest)
    }
}

impl fmt::Debug for CommitRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "C{}({:?})", self.index, self.digest)
    }
}

// Represents a vote on a Commit.
pub type CommitVote = CommitRef;

/// Base struct containing common fields shared between PendingSubDag and
/// CommittedSubDag
#[derive(Clone, PartialEq)]
pub struct SubDagBase {
    /// A reference to the leader of the sub-dag
    pub leader: BlockRef,

    /// All the block headers that are traversed on DAG starting with the
    /// leader.
    ///
    /// Required for leader scoring (ancestors) and metrics (timestamp_ms).
    /// In the future, it may be optional when syncing via FastCommitSyncer.
    pub headers: Vec<VerifiedBlockHeader>,

    /// Block references of committed headers.
    ///
    /// Used for operations that only need reference data (round, author,
    /// digest) without full header details. Removes dependency on the
    /// optional headers field in ConsensusOutputAPI trait implementation.
    pub committed_header_refs: Vec<BlockRef>,
    /// The timestamp of the commit, obtained from the timestamp of the leader
    /// block.
    pub timestamp_ms: BlockTimestampMs,
    /// The reference of the commit.
    /// First commit after genesis has a index of 1, then every next commit has
    /// an index incremented by 1.
    pub commit_ref: CommitRef,
    /// Optional scores that are provided as part of the consensus output to
    /// IOTA that can then be used by IOTA for future transaction submission to
    /// consensus.
    pub reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
}

impl SubDagBase {
    /// Helper method to retrieve transaction acknowledgment references for each
    /// AuthorityIndex from the committed block headers.
    pub(crate) fn transaction_acknowledgments(
        &self,
    ) -> HashMap<(Round, AuthorityIndex), Vec<BlockRef>> {
        self.headers
            .iter()
            .map(|header| {
                (
                    (header.round(), header.author()),
                    header.acknowledgments().to_vec(),
                )
            })
            .fold(
                HashMap::new(),
                |mut acc: HashMap<(Round, AuthorityIndex), HashSet<BlockRef>>,
                 ((round, auth), vec)| {
                    acc.entry((round, auth)).or_default().extend(vec);
                    acc
                },
            )
            .into_iter()
            .map(|(round_auth, set)| (round_auth, set.into_iter().collect()))
            .collect()
    }
}

impl Display for SubDagBase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CommittedSubDag(leader={}, ref={}, blocks=[{}])",
            self.leader,
            self.commit_ref,
            format_block_digests(&self.committed_header_refs),
        )
    }
}

impl fmt::Debug for SubDagBase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{} ([{}];{}ms;rs{:?})",
            self.leader,
            self.commit_ref,
            format_block_digests(&self.committed_header_refs),
            self.timestamp_ms,
            self.reputation_scores_desc
        )
    }
}

/// The output of consensus to execution is an ordered list of
/// [`CommittedSubDag`]. Each CommittedSubDag contains the information needed to
/// execution transactions in the consensus commit.
///
/// The application processing CommittedSubDag can arbitrarily sort the blocks
/// within each sub-dag (but using a deterministic algorithm).
#[derive(Clone, PartialEq)]
pub struct CommittedSubDag {
    /// Common fields shared with PendingSubDag
    pub base: SubDagBase,
    /// All the committed blocks that are part of this sub-dag
    pub transactions: Vec<VerifiedTransactions>,
}

impl CommittedSubDag {
    /// Creates a new committed sub dag.
    pub fn new(
        leader: BlockRef,
        headers: Vec<VerifiedBlockHeader>,
        committed_header_refs: Vec<BlockRef>,
        transactions: Vec<VerifiedTransactions>,
        timestamp_ms: BlockTimestampMs,
        commit_ref: CommitRef,
        reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
    ) -> Self {
        Self {
            base: SubDagBase {
                leader,
                headers,
                committed_header_refs,
                timestamp_ms,
                commit_ref,
                reputation_scores_desc,
            },
            transactions,
        }
    }

    /// Helper method to format transaction refs in a consistent way
    fn format_transaction_refs(&self) -> String {
        format_transaction_ref_digests(
            &self
                .transactions
                .iter()
                .map(|t| GenericTransactionRef::TransactionRef(t.transaction_ref()))
                .collect::<Vec<_>>(),
        )
    }
}

impl Display for CommittedSubDag {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CommittedSubDag(leader={}, ref={}, blocks=[{}], committed_transactions=[{}])",
            self.leader,
            self.commit_ref,
            format_block_digests(&self.committed_header_refs),
            self.format_transaction_refs(),
        )
    }
}

impl fmt::Debug for CommittedSubDag {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{} ([{}];[{}];{}ms;rs{:?})",
            self.leader,
            self.commit_ref,
            format_block_digests(&self.committed_header_refs),
            self.format_transaction_refs(),
            self.timestamp_ms,
            self.reputation_scores_desc
        )
    }
}

// Implementation of common traits with field delegation
impl std::ops::Deref for PendingSubDag {
    type Target = SubDagBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

/// PendingSubDag is a structure used after creating a commit and making sure
/// that all committed transactions are available. This struct is used to update
/// the leader schedule as well as produce a CommittedSubDag once all the
/// committed_transactions become available.
#[derive(Clone, PartialEq)]
pub struct PendingSubDag {
    /// Common fields shared with CommittedSubDag
    pub base: SubDagBase,
    /// References to blocks whose transactions were committed as part of this
    /// SubDag.
    pub committed_transaction_refs: Vec<GenericTransactionRef>,
}

impl PendingSubDag {
    /// Creates a new pending sub dag.
    pub fn new(
        leader: BlockRef,
        headers: Vec<VerifiedBlockHeader>,
        committed_header_refs: Vec<BlockRef>,
        committed_transaction_refs: Vec<GenericTransactionRef>,
        timestamp_ms: BlockTimestampMs,
        commit_ref: CommitRef,
        reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
    ) -> Self {
        Self {
            base: SubDagBase {
                leader,
                headers,
                committed_header_refs,
                timestamp_ms,
                commit_ref,
                reputation_scores_desc,
            },
            committed_transaction_refs,
        }
    }
}

impl std::ops::Deref for CommittedSubDag {
    type Target = SubDagBase;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl Display for PendingSubDag {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PendingSubDag(leader={}, ref={}, blocks=[{}], committed_transactions=[{}])",
            self.leader,
            self.commit_ref,
            format_block_digests(&self.committed_header_refs),
            format_transaction_ref_digests(&self.committed_transaction_refs),
        )
    }
}

impl fmt::Debug for PendingSubDag {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{} ([{}];[{}];{}ms;rs{:?})",
            self.leader,
            self.commit_ref,
            format_block_digests(&self.committed_header_refs),
            format_transaction_ref_digests(&self.committed_transaction_refs),
            self.timestamp_ms,
            self.reputation_scores_desc
        )
    }
}

// Sort the block headers of the sub-dag blocks by round number then authority
// index. Any deterministic and stable algorithm works.
pub(crate) fn sort_sub_dag_blocks(block_headers: &mut [VerifiedBlockHeader]) {
    block_headers.sort_by(|a, b| {
        a.round()
            .cmp(&b.round())
            .then_with(|| a.author().cmp(&b.author()))
    })
}

// Recovers PendingSubDAG from block store, based on Commit.
pub fn load_pending_subdag_from_store(
    store: &dyn Store,
    commit: TrustedCommit,
    reputation_scores_desc: Vec<(AuthorityIndex, u64)>,
) -> PendingSubDag {
    let mut leader_block_idx = None;
    let commit_block_headers = store
        .read_verified_block_headers(commit.block_headers())
        .expect("Block headers referenced in commit data should exist");
    let block_headers = commit_block_headers
        .into_iter()
        .enumerate()
        .map(|(idx, commit_block_opt)| {
            let commit_block = commit_block_opt.expect(
                "Block header referenced in commit data should exist. \
                 This could be due to unfinished fast syncing.",
            );
            if commit_block.reference() == commit.leader() {
                leader_block_idx = Some(idx);
            }
            commit_block
        })
        .collect::<Vec<_>>();
    let leader_block_idx = leader_block_idx.expect("Leader block must be in the sub-dag");
    let leader_block_ref = block_headers[leader_block_idx].reference();
    PendingSubDag::new(
        leader_block_ref,
        block_headers,
        commit.block_headers().to_vec(),
        commit.committed_transactions(),
        commit.timestamp_ms(),
        commit.reference(),
        reputation_scores_desc,
    )
}

fn format_transaction_ref_digests(transaction_refs: &[GenericTransactionRef]) -> String {
    let mut result = String::new();
    for (idx, block) in transaction_refs.iter().enumerate() {
        if idx > 0 {
            result.push_str(", ");
        }
        result.push_str(&block.digest().to_string());
        result.push('@');
        result.push_str(&block.round().to_string());
    }
    result
}

fn format_block_digests(blocks: &[BlockRef]) -> String {
    let mut result = String::new();
    for (idx, block) in blocks.iter().enumerate() {
        if idx > 0 {
            result.push_str(", ");
        }
        result.push_str(&block.digest.to_string());
        result.push('@');
        result.push_str(&block.round.to_string());
    }
    result
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum Decision {
    Direct,
    Indirect,
}

/// The status of a leader slot from the direct and indirect commit rules.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LeaderStatus {
    Commit(VerifiedBlockHeader),
    Skip(Slot),
    Undecided(Slot),
}

impl LeaderStatus {
    pub(crate) fn round(&self) -> Round {
        match self {
            Self::Commit(block) => block.round(),
            Self::Skip(leader) => leader.round,
            Self::Undecided(leader) => leader.round,
        }
    }

    pub(crate) fn is_decided(&self) -> bool {
        match self {
            Self::Commit(_) => true,
            Self::Skip(_) => true,
            Self::Undecided(_) => false,
        }
    }

    pub(crate) fn into_decided_leader(self) -> Option<DecidedLeader> {
        match self {
            Self::Commit(block) => Some(DecidedLeader::Commit(block)),
            Self::Skip(slot) => Some(DecidedLeader::Skip(slot)),
            Self::Undecided(..) => None,
        }
    }
}

impl Display for LeaderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Commit(block) => write!(f, "Commit({})", block.reference()),
            Self::Skip(slot) => write!(f, "Skip({slot})"),
            Self::Undecided(slot) => write!(f, "Undecided({slot})"),
        }
    }
}

/// Decision of each leader slot.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DecidedLeader {
    Commit(VerifiedBlockHeader),
    Skip(Slot),
}

impl DecidedLeader {
    // Slot where the leader is decided.
    pub(crate) fn slot(&self) -> Slot {
        match self {
            Self::Commit(block) => block.reference().into(),
            Self::Skip(slot) => *slot,
        }
    }

    // Converts to committed block if the decision is to commit. Returns None
    // otherwise.
    pub(crate) fn into_committed_block(self) -> Option<VerifiedBlockHeader> {
        match self {
            Self::Commit(block) => Some(block),
            Self::Skip(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn round(&self) -> Round {
        match self {
            Self::Commit(block) => block.round(),
            Self::Skip(leader) => leader.round,
        }
    }

    #[cfg(test)]
    pub(crate) fn authority(&self) -> AuthorityIndex {
        match self {
            Self::Commit(block) => block.author(),
            Self::Skip(leader) => leader.authority,
        }
    }
}

impl Display for DecidedLeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Commit(block) => write!(f, "Commit({})", block.reference()),
            Self::Skip(slot) => write!(f, "Skip({slot})"),
        }
    }
}

/// Per-commit properties that can be regenerated from past values, and do not
/// need to be part of the Commit struct.
/// Only the latest version is needed for recovery, but more versions are stored
/// for debugging, and potentially restoring from an earlier state.
// TODO: version this struct.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CommitInfo {
    pub(crate) committed_rounds: Vec<Round>,
    pub(crate) reputation_scores: ReputationScores,
}

/// CommitRange stores a range of CommitIndex. The range contains the start
/// (inclusive) and end (inclusive) commit indices and can be ordered for use as
/// the key of a table.
///
/// NOTE: using Range<CommitIndex> for internal representation for backward
/// compatibility. The external semantics of CommitRange is closer to
/// RangeInclusive<CommitIndex>.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CommitRange(Range<CommitIndex>);

impl CommitRange {
    pub(crate) fn new(range: RangeInclusive<CommitIndex>) -> Self {
        // When end is CommitIndex::MAX, the range can be considered as unbounded
        // so it is ok to saturate at the end.
        Self(*range.start()..(*range.end()).saturating_add(1))
    }

    // Inclusive
    pub(crate) fn start(&self) -> CommitIndex {
        self.0.start
    }

    // Inclusive
    pub(crate) fn end(&self) -> CommitIndex {
        self.0.end.saturating_sub(1)
    }

    pub(crate) fn extend_to(&mut self, other: CommitIndex) {
        let new_end = other.saturating_add(1);
        assert!(self.0.end <= new_end);
        self.0 = self.0.start..new_end;
    }

    pub(crate) fn size(&self) -> usize {
        self.0
            .end
            .checked_sub(self.0.start)
            .expect("Range should never have end < start") as usize
    }

    /// Check whether the two ranges have the same size.
    pub(crate) fn is_equal_size(&self, other: &Self) -> bool {
        self.size() == other.size()
    }

    /// Check if the provided range is sequentially after this range.
    pub(crate) fn is_next_range(&self, other: &Self) -> bool {
        self.0.end == other.0.start
    }
}

impl Ord for CommitRange {
    fn cmp(&self, other: &Self) -> Ordering {
        self.start()
            .cmp(&other.start())
            .then_with(|| self.end().cmp(&other.end()))
    }
}

impl PartialOrd for CommitRange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<RangeInclusive<CommitIndex>> for CommitRange {
    fn from(range: RangeInclusive<CommitIndex>) -> Self {
        Self::new(range)
    }
}

/// Display CommitRange as an inclusive range.
impl Debug for CommitRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "CommitRange({}..={})", self.start(), self.end())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        BlockHeaderAPI, BlockTimestampMs, CommitDigest, VerifiedBlockHeader,
        block_header::{TestBlockHeader, VerifiedBlock},
        commit::{CommitRange, TrustedCommit, WAVE_LENGTH, load_pending_subdag_from_store},
        context::Context,
        encoder::create_encoder,
        storage::{Store, WriteBatch, mem_store::MemStore},
        transaction_ref::convert_block_refs_to_generic_transaction_refs,
    };

    #[tokio::test]
    async fn test_new_committed_subdag_from_commit() {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = Arc::new(MemStore::new(context.clone()));
        let mut encoder = create_encoder(&context);

        // Populate fully connected test blocks for round 0 ~ 3, authorities 0 ~ 3.
        let first_wave_rounds: u32 = WAVE_LENGTH;
        let num_authorities: u8 = 4;

        let mut blocks = Vec::new();
        let (first_round_references, first_round_blocks): (Vec<_>, Vec<_>) = context
            .committee
            .authorities()
            .map(|index| {
                let author_idx = index.0.value() as u8;
                let tx = index.0.value() as u8;
                let block = TestBlockHeader::new_with_transaction(
                    0,
                    author_idx,
                    tx,
                    &context,
                    &mut encoder,
                )
                .build();
                VerifiedBlock::new_with_transaction_for_test(block, tx)
            })
            .map(|block| {
                (
                    block.reference(),
                    (block.verified_block_header, block.verified_transactions),
                )
            })
            .unzip();

        let (first_round_headers, first_round_transactions) =
            first_round_blocks.into_iter().unzip();
        store
            .write(
                WriteBatch::default()
                    .block_headers(first_round_headers)
                    .transactions(first_round_transactions),
                context.clone(),
            )
            .unwrap();
        blocks.append(&mut first_round_references.clone());

        let mut ancestors = first_round_references.clone();
        let mut leader = None;
        for round in 1..=first_wave_rounds {
            let mut new_ancestors = vec![];
            for author in 0..num_authorities {
                let base_ts = round as BlockTimestampMs * 1000;
                let block = VerifiedBlock::new_for_test(
                    TestBlockHeader::new_with_commitment(round, author, &context, &mut encoder)
                        .set_timestamp_ms(base_ts + (author as u32 + round) as u64)
                        .set_ancestors(ancestors.clone())
                        .set_acknowledgments(ancestors.clone())
                        .build(),
                );
                store
                    .write(
                        WriteBatch::default()
                            .block_headers(vec![block.verified_block_header.clone()])
                            .transactions(vec![block.verified_transactions.clone()]),
                        context.clone(),
                    )
                    .unwrap();
                new_ancestors.push(block.reference());
                blocks.push(block.reference());

                // only write one block for the final round, which is the leader
                // of the committed subdag.
                if round == first_wave_rounds {
                    leader = Some(block.clone());
                    break;
                }
            }
            ancestors = new_ancestors;
        }

        let leader_block = leader.unwrap();
        let leader_ref = leader_block.reference();
        let commit_index = 1;

        // Convert BlockRefs to GenericTransactionRefs based on protocol flag
        let generic_committed_transactions = convert_block_refs_to_generic_transaction_refs(
            &context,
            store.as_ref(),
            &first_round_references,
        );

        let commit = TrustedCommit::new_for_test(
            &context,
            commit_index,
            CommitDigest::MIN,
            leader_block.timestamp_ms(),
            leader_ref,
            blocks.clone(),
            generic_committed_transactions.clone(),
        );

        let subdag = load_pending_subdag_from_store(store.as_ref(), commit.clone(), vec![]);
        assert_eq!(subdag.leader, leader_ref);
        assert_eq!(subdag.timestamp_ms, leader_block.timestamp_ms());
        assert_eq!(
            subdag.headers.len(),
            (num_authorities as u32 * WAVE_LENGTH) as usize + 1
        );
        assert_eq!(subdag.commit_ref, commit.reference());
        assert_eq!(
            subdag.committed_transaction_refs,
            generic_committed_transactions
        );
        assert_eq!(subdag.reputation_scores_desc, vec![]);
        let transactions = store
            .read_verified_transactions(&subdag.committed_transaction_refs)
            .expect("We should have the transactions referenced in the commit data")
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(transactions.len(), 4);
    }

    #[tokio::test]
    async fn test_new_pending_subdag_from_commit() {
        let context = Arc::new(Context::new_for_test(4).0);
        let store = Arc::new(MemStore::new(context.clone()));
        // Populate fully connected test blocks for round 0 ~ 3, authorities 0 ~ 3.
        let first_wave_rounds: u32 = WAVE_LENGTH;
        let num_authorities: u8 = 4;

        let mut blocks = Vec::new();
        let (first_round_references, first_round_headers): (Vec<_>, Vec<_>) = context
            .committee
            .authorities()
            .map(|index| {
                let author_idx = index.0.value() as u8;
                let block = TestBlockHeader::new(0, author_idx).build();
                VerifiedBlockHeader::new_for_test(block)
            })
            .map(|block| (block.reference(), block))
            .unzip();
        store
            .write(
                WriteBatch::default().block_headers(first_round_headers),
                context.clone(),
            )
            .unwrap();
        blocks.append(&mut first_round_references.clone());

        let mut ancestors = first_round_references.clone();
        let mut leader = None;
        for round in 1..=first_wave_rounds {
            let mut new_ancestors = vec![];
            for author in 0..num_authorities {
                let base_ts = round as BlockTimestampMs * 1000;
                let block = VerifiedBlockHeader::new_for_test(
                    TestBlockHeader::new(round, author)
                        .set_timestamp_ms(base_ts + (author as u32 + round) as u64)
                        .set_ancestors(ancestors.clone())
                        .set_acknowledgments(ancestors.clone())
                        .build(),
                );
                store
                    .write(
                        WriteBatch::default().block_headers(vec![block.clone()]),
                        context.clone(),
                    )
                    .unwrap();
                new_ancestors.push(block.reference());
                blocks.push(block.reference());

                // only write one block for the final round, which is the leader
                // of the committed subdag.
                if round == first_wave_rounds {
                    leader = Some(block.clone());
                    break;
                }
            }
            ancestors = new_ancestors;
        }

        let leader_block = leader.unwrap();
        let leader_ref = leader_block.reference();
        let commit_index = 1;

        // Convert BlockRefs to GenericTransactionRefs based on protocol flag
        let generic_committed_transactions = convert_block_refs_to_generic_transaction_refs(
            &context,
            store.as_ref(),
            &first_round_references,
        );

        let commit = TrustedCommit::new_for_test(
            &context,
            commit_index,
            CommitDigest::MIN,
            leader_block.timestamp_ms(),
            leader_ref,
            blocks.clone(),
            generic_committed_transactions.clone(),
        );

        let pending_subdag = load_pending_subdag_from_store(store.as_ref(), commit.clone(), vec![]);
        assert_eq!(pending_subdag.leader, leader_ref);
        assert_eq!(pending_subdag.timestamp_ms, leader_block.timestamp_ms());
        assert_eq!(
            pending_subdag.headers.len(),
            (num_authorities as u32 * WAVE_LENGTH) as usize + 1
        );
        assert_eq!(pending_subdag.commit_ref, commit.reference());
        assert_eq!(
            pending_subdag.committed_transaction_refs,
            generic_committed_transactions
        );
        assert_eq!(pending_subdag.reputation_scores_desc, vec![]);
    }

    #[tokio::test]
    async fn test_commit_range() {
        telemetry_subscribers::init_for_testing();
        let mut range1 = CommitRange::new(1..=5);
        let range2 = CommitRange::new(2..=6);
        let range3 = CommitRange::new(5..=10);
        let range4 = CommitRange::new(6..=10);
        let range5 = CommitRange::new(6..=9);
        let range6 = CommitRange::new(1..=1);

        assert_eq!(range1.start(), 1);
        assert_eq!(range1.end(), 5);

        // Test range size
        assert_eq!(range1.size(), 5);
        assert_eq!(range3.size(), 6);
        assert_eq!(range6.size(), 1);

        // Test next range check
        assert!(!range1.is_next_range(&range2));
        assert!(!range1.is_next_range(&range3));
        assert!(range1.is_next_range(&range4));
        assert!(range1.is_next_range(&range5));

        // Test equal size range check
        assert!(range1.is_equal_size(&range2));
        assert!(!range1.is_equal_size(&range3));
        assert!(range1.is_equal_size(&range4));
        assert!(!range1.is_equal_size(&range5));

        // Test range ordering
        assert!(range1 < range2);
        assert!(range2 < range3);
        assert!(range3 < range4);
        assert!(range5 < range4);

        // Test extending range
        range1.extend_to(10);
        assert_eq!(range1.start(), 1);
        assert_eq!(range1.end(), 10);
        assert_eq!(range1.size(), 10);
        range1.extend_to(20);
        assert_eq!(range1.start(), 1);
        assert_eq!(range1.end(), 20);
        assert_eq!(range1.size(), 20);
    }
}
