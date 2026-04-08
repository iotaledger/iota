// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashSet,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::Arc,
};

use bytes::Bytes;
use enum_dispatch::enum_dispatch;
use fastcrypto::hash::{Digest, HashFunction};
use iota_sdk_types::crypto::{Intent, IntentMessage, IntentScope};
use rs_merkle::{MerkleProof, MerkleTree};
use serde::{Deserialize, Serialize};
use starfish_config::{
    AuthorityIndex, DIGEST_LENGTH, DefaultHashFunction, DefaultHashFunctionWrapper, Epoch,
    ProtocolKeyPair, ProtocolKeySignature, ProtocolPublicKey,
};
use tracing::instrument;

use crate::{
    commit::CommitVote,
    context::Context,
    encoder::ShardEncoder,
    error::{ConsensusError, ConsensusResult},
    transaction_ref::{GenericTransactionRef, TransactionRef},
};

/// Round number of a block.
pub type Round = u32;

// In consensus modification with encoding and decoding we divide data into a
// sequence of smaller pieces -- Shards, which then serve as smallest piece of
// information, being sent between validators and so on
pub type Shard = Vec<u8>;

pub(crate) const GENESIS_ROUND: Round = 0;

/// Block proposal as epoch UNIX timestamp in milliseconds.
pub type BlockTimestampMs = u64;

/// IOTA transaction is considered as serialised bytes inside consensus
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Default, Debug)]
pub struct Transaction {
    data: Bytes,
}

impl Transaction {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data: data.into() }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_data(self) -> Bytes {
        self.data
    }

    /// Serialises a vector of transactions using the bcs serializer
    pub(crate) fn serialize(transactions: &[Transaction]) -> Result<Bytes, ConsensusError> {
        let bytes = bcs::to_bytes(transactions).map_err(ConsensusError::SerializationFailure)?;
        Ok(bytes.into())
    }

    /// Create a vector of random transactions for testing.
    #[cfg(test)]
    pub fn random_transactions(count: usize, max_len: usize) -> Vec<Self> {
        (0..count)
            .map(|_| Self::random_transaction(max_len))
            .collect()
    }

    // Create one random transaction for testing
    #[cfg(test)]
    pub fn random_transaction(max_len: usize) -> Self {
        use rand::{Rng, RngCore};

        let mut rng = rand::thread_rng();
        let len = rng.gen_range(0..=max_len);
        let mut buf = vec![0u8; len];
        rng.fill_bytes(&mut buf);
        Transaction {
            data: Bytes::from(buf),
        }
    }
}

/// A block header includes references to previous round blocks and a commitment
/// to transactions that the authority considers valid.
/// Well behaved authorities produce at most one block header per round, but
/// malicious authorities can equivocate.
#[derive(Clone, Deserialize, Serialize, PartialOrd, PartialEq, Ord, Eq)]
pub enum BlockHeader {
    V1(BlockHeaderV1),
}

pub trait BlockHeaderAPI {
    fn epoch(&self) -> Epoch;
    fn round(&self) -> Round;
    fn author(&self) -> AuthorityIndex;
    fn slot(&self) -> Slot;
    fn acknowledgments(&self) -> &[BlockRef];
    fn timestamp_ms(&self) -> BlockTimestampMs;
    fn ancestors(&self) -> &[BlockRef];
    fn commit_votes(&self) -> &[CommitVote];
    fn transactions_commitment(&self) -> TransactionsCommitment;
}

#[derive(Clone, Default, Deserialize, Serialize, PartialOrd, PartialEq, Ord, Eq)]
pub struct BlockHeaderV1 {
    epoch: Epoch,
    round: Round,
    author: AuthorityIndex,
    timestamp_ms: BlockTimestampMs,
    // ancestors are BlockRefs such that there are at least 2f+1 BlockRefs (by stake) from the
    // previous round
    // acknowledgments are BlockRefs for blocks for which a validator acknowledges data
    // availability of transactions
    // references is a compressed vector that contains both the ancestors and acknowledgments
    // layout: |ancestors|overlap_without_ref0|acknowledgments|ref0?|
    references: Vec<BlockRef>,
    overlap_start_index: u8, // bounded by committee size <=256
    overlap_end_index: u8,   // bounded by committee size <=256
    transactions_commitment: TransactionsCommitment,
    commit_votes: Vec<CommitVote>,
}

impl BlockHeaderV1 {
    pub(crate) fn new(
        epoch: Epoch,
        round: Round,
        author: AuthorityIndex,
        timestamp_ms: BlockTimestampMs,
        ancestors: Vec<BlockRef>,
        acknowledgments: Vec<BlockRef>,
        commit_votes: Vec<CommitVote>,
        transactions_commitment: TransactionsCommitment,
    ) -> BlockHeaderV1 {
        let (references, overlap_start_index, overlap_end_index) =
            Self::compress_references(ancestors, acknowledgments);
        Self {
            epoch,
            round,
            author,
            timestamp_ms,
            references,
            overlap_start_index,
            overlap_end_index,
            transactions_commitment,
            commit_votes,
        }
    }
    /// Compresses ancestors and acknowledgments into a single references
    /// vector, and returns the overlap indices. The first ancestor is
    /// always the first reference (ref0). If it is also in acknowledgments,
    /// it is appended to the end of references.
    pub(crate) fn compress_references(
        ancestors: Vec<BlockRef>,
        acknowledgments: Vec<BlockRef>,
    ) -> (Vec<BlockRef>, u8, u8) {
        if ancestors.is_empty() {
            return (acknowledgments, 0, 0);
        }
        // Sets for membership checks
        let ancestor_set: HashSet<_> = ancestors.iter().cloned().collect();
        let ack_set: HashSet<_> = acknowledgments.into_iter().collect();
        // ref0 is the first ancestor, and is also always the first reference
        let ref0 = ancestors[0];
        // if it is also in acknowledgments, it is appended to the end of references
        let append_ref0 = ack_set.contains(&ref0);

        // partition ancestors into overlap and ancestors_only (excluding ref0)
        let (overlap, mut ancestors_only): (Vec<_>, Vec<_>) = ancestors
            .into_iter()
            .skip(1)
            .partition(|a| ack_set.contains(a));
        // insert ref0 back to the front of ancestors_only
        ancestors_only.insert(0, ref0);

        // acknowledgments_only excludes any overlap with ancestors
        let acknowledgments_only: Vec<_> = ack_set
            .into_iter()
            .filter(|a| !ancestor_set.contains(a))
            .collect();

        let overlap_start_index = ancestors_only.len();
        let overlap_end_index = overlap_start_index + overlap.len();
        // combine all parts into references
        // |ancestors_only|overlap|acknowledgments_only|ref0?|
        let mut references = ancestors_only;
        references.extend(overlap);
        references.extend(acknowledgments_only);
        if append_ref0 {
            references.push(ref0);
        }
        (
            references,
            overlap_start_index as u8,
            overlap_end_index as u8,
        )
    }

    fn genesis_block_header(context: &Context, author: AuthorityIndex) -> Self {
        Self {
            epoch: context.committee.epoch(),
            round: GENESIS_ROUND,
            author,
            timestamp_ms: context.epoch_start_timestamp_ms,
            references: vec![],
            overlap_start_index: 0,
            overlap_end_index: 0,
            commit_votes: vec![],
            transactions_commitment: TransactionsCommitment::default(),
        }
    }
}

impl BlockHeaderAPI for BlockHeaderV1 {
    fn epoch(&self) -> Epoch {
        self.epoch
    }

    fn round(&self) -> Round {
        self.round
    }

    fn author(&self) -> AuthorityIndex {
        self.author
    }

    fn slot(&self) -> Slot {
        Slot::new(self.round, self.author)
    }

    fn acknowledgments(&self) -> &[BlockRef] {
        &self.references[self.overlap_start_index as usize..]
    }

    fn timestamp_ms(&self) -> BlockTimestampMs {
        self.timestamp_ms
    }
    fn ancestors(&self) -> &[BlockRef] {
        &self.references[..self.overlap_end_index as usize]
    }

    fn commit_votes(&self) -> &[CommitVote] {
        &self.commit_votes
    }

    fn transactions_commitment(&self) -> TransactionsCommitment {
        self.transactions_commitment
    }
}

impl BlockHeaderAPI for BlockHeader {
    fn epoch(&self) -> Epoch {
        match self {
            BlockHeader::V1(header) => header.epoch(),
        }
    }

    fn round(&self) -> Round {
        match self {
            BlockHeader::V1(header) => header.round(),
        }
    }

    fn author(&self) -> AuthorityIndex {
        match self {
            BlockHeader::V1(header) => header.author(),
        }
    }

    fn slot(&self) -> Slot {
        match self {
            BlockHeader::V1(header) => header.slot(),
        }
    }

    fn acknowledgments(&self) -> &[BlockRef] {
        match self {
            BlockHeader::V1(header) => header.acknowledgments(),
        }
    }

    fn timestamp_ms(&self) -> BlockTimestampMs {
        match self {
            BlockHeader::V1(header) => header.timestamp_ms(),
        }
    }

    fn ancestors(&self) -> &[BlockRef] {
        match self {
            BlockHeader::V1(header) => header.ancestors(),
        }
    }

    fn commit_votes(&self) -> &[CommitVote] {
        match self {
            BlockHeader::V1(header) => header.commit_votes(),
        }
    }

    fn transactions_commitment(&self) -> TransactionsCommitment {
        match self {
            BlockHeader::V1(header) => header.transactions_commitment(),
        }
    }
}

impl From<BlockHeaderV1> for BlockHeader {
    fn from(header: BlockHeaderV1) -> Self {
        BlockHeader::V1(header)
    }
}

/// `BlockRef` uniquely identifies a `VerifiedBlock` via `digest`. It also
/// contains the slot info (round and author) so it can be used in logic such as
/// aggregating stakes for a round.
#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockRef {
    pub round: Round,
    pub author: AuthorityIndex,
    pub digest: BlockHeaderDigest,
}

impl BlockRef {
    pub const MIN: Self = Self {
        round: 0,
        author: AuthorityIndex::MIN,
        digest: BlockHeaderDigest::MIN,
    };

    pub const MAX: Self = Self {
        round: u32::MAX,
        author: AuthorityIndex::MAX,
        digest: BlockHeaderDigest::MAX,
    };

    pub fn new(round: Round, author: AuthorityIndex, digest: BlockHeaderDigest) -> Self {
        Self {
            round,
            author,
            digest,
        }
    }
}

impl fmt::Display for BlockRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "B{}({},{})", self.round, self.author, self.digest)
    }
}

impl fmt::Debug for BlockRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        fmt::Display::fmt(self, f)
    }
}

impl Hash for BlockRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest.0[..8]);
    }
}

/// Digest of a `VerifiedBlockHeader` or verified `SignedBlockHeader`, which
/// covers the `BlockHeader` and its signature.
///
/// Note: the signature algorithm is assumed to be non-malleable, so it is
/// impossible for another party to create an altered but valid signature,
/// producing an equivocating `BlockDigest`.
#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockHeaderDigest([u8; starfish_config::DIGEST_LENGTH]);

impl BlockHeaderDigest {
    /// Lexicographic min & max digest.
    pub const MIN: Self = Self([u8::MIN; starfish_config::DIGEST_LENGTH]);
    pub const MAX: Self = Self([u8::MAX; starfish_config::DIGEST_LENGTH]);
}

impl BlockHeaderDigest {
    #[cfg(test)]
    pub fn random<R: rand::RngCore + rand::CryptoRng>(mut rng: R) -> Self {
        let mut bytes = [0; DIGEST_LENGTH];
        rng.fill_bytes(&mut bytes);
        Self(bytes)
    }
}

impl Hash for BlockHeaderDigest {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.0[..8]);
    }
}

impl From<BlockHeaderDigest> for Digest<{ DIGEST_LENGTH }> {
    fn from(hd: BlockHeaderDigest) -> Self {
        Digest::new(hd.0)
    }
}

impl fmt::Display for BlockHeaderDigest {
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

impl fmt::Debug for BlockHeaderDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.0)
        )
    }
}

impl AsRef<[u8]> for BlockHeaderDigest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionsCommitment(pub(crate) [u8; starfish_config::DIGEST_LENGTH]);
pub type MerkleProofBytes = Vec<u8>;

/// Used when the protocol flag `consensus_fast_commit_sync` is disabled.
/// Contains block reference and separate transaction commitment field.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct ShardWithProofV1 {
    pub(crate) shard: Shard,
    pub(crate) transaction_commitment: TransactionsCommitment,
    pub(crate) proof: MerkleProofBytes,
    pub(crate) block_ref: BlockRef,
}

/// Used when the protocol flag `consensus_fast_commit_sync` is enabled.
/// Contains transaction reference which includes the transaction commitment.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct ShardWithProofV2 {
    pub(crate) shard: Shard,
    pub(crate) proof: MerkleProofBytes,
    pub(crate) transaction_ref: TransactionRef,
}

/// Accessors to shard with proof info.
#[enum_dispatch]
pub(crate) trait ShardWithProofAPI {
    fn shard(&self) -> &Shard;
    fn proof(&self) -> &MerkleProofBytes;
    fn transaction_commitment(&self) -> TransactionsCommitment;
    fn round(&self) -> Round;
    fn author(&self) -> AuthorityIndex;
    fn block_digest(&self) -> Option<BlockHeaderDigest>;
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[enum_dispatch(ShardWithProofAPI)]
pub(crate) enum ShardWithProof {
    V1(ShardWithProofV1),
    V2(ShardWithProofV2),
}

impl ShardWithProof {
    /// Creates a new ShardWithProof instance based on the protocol flag.
    /// If `consensus_fast_commit_sync` is true, creates V2 variant, otherwise
    /// V1.
    pub(crate) fn new(
        shard: Shard,
        proof: MerkleProofBytes,
        block_ref: BlockRef,
        transaction_commitment: TransactionsCommitment,
        consensus_fast_commit_sync: bool,
    ) -> Self {
        if consensus_fast_commit_sync {
            ShardWithProof::V2(ShardWithProofV2 {
                shard,
                proof,
                transaction_ref: TransactionRef {
                    round: block_ref.round,
                    author: block_ref.author,
                    transactions_commitment: transaction_commitment,
                },
            })
        } else {
            ShardWithProof::V1(ShardWithProofV1 {
                shard,
                transaction_commitment,
                proof,
                block_ref,
            })
        }
    }
}

impl ShardWithProofAPI for ShardWithProofV1 {
    fn shard(&self) -> &Shard {
        &self.shard
    }

    fn proof(&self) -> &MerkleProofBytes {
        &self.proof
    }

    fn transaction_commitment(&self) -> TransactionsCommitment {
        self.transaction_commitment
    }

    fn round(&self) -> Round {
        self.block_ref.round
    }

    fn author(&self) -> AuthorityIndex {
        self.block_ref.author
    }

    fn block_digest(&self) -> Option<BlockHeaderDigest> {
        Some(self.block_ref.digest)
    }
}

impl ShardWithProofAPI for ShardWithProofV2 {
    fn shard(&self) -> &Shard {
        &self.shard
    }

    fn proof(&self) -> &MerkleProofBytes {
        &self.proof
    }

    fn transaction_commitment(&self) -> TransactionsCommitment {
        self.transaction_ref.transactions_commitment
    }

    fn round(&self) -> Round {
        self.transaction_ref.round
    }

    fn author(&self) -> AuthorityIndex {
        self.transaction_ref.author
    }

    fn block_digest(&self) -> Option<BlockHeaderDigest> {
        None
    }
}

pub(crate) struct VerifiedOwnShard {
    pub(crate) serialized_shard: Bytes,
    pub(crate) gen_transaction_ref: GenericTransactionRef,
}

impl TransactionsCommitment {
    /// Lexicographic min & max digest.
    pub const MIN: Self = Self([u8::MIN; starfish_config::DIGEST_LENGTH]);
    pub const MAX: Self = Self([u8::MAX; starfish_config::DIGEST_LENGTH]);
    pub const DEFAULT_FOR_TEST: Self = Self([u8::MIN; starfish_config::DIGEST_LENGTH]);

    pub(crate) fn compute_merkle_root_shard_and_proof(
        serialized_transactions: &Bytes,
        context: &Arc<Context>,
        encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
    ) -> ConsensusResult<(TransactionsCommitment, Shard, MerkleProofBytes)> {
        let info_length = context.committee.info_length();
        let parity_length = context.committee.size() - info_length;
        let encoded_shards =
            encoder.encode_serialized_data(serialized_transactions, info_length, parity_length)?;
        let own_index = context.own_index;
        let (transactions_commitment, merkle_proof) =
            TransactionsCommitment::compute_merkle_root_and_proof(&encoded_shards, own_index)?;
        Ok((
            transactions_commitment,
            encoded_shards[own_index].clone(),
            merkle_proof,
        ))
    }

    pub(crate) fn compute_transactions_commitment(
        serialized_transactions: &Bytes,
        context: &Arc<Context>,
        encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
    ) -> ConsensusResult<TransactionsCommitment> {
        let info_length = context.committee.info_length();
        let parity_length = context.committee.size() - info_length;
        let encoded_shards = encoder
            .encode_serialized_data(serialized_transactions, info_length, parity_length)
            .expect("We should expect correct encoding of the shards");

        let (transactions_commitment, _) = TransactionsCommitment::compute_merkle_root_and_proof(
            &encoded_shards,
            context.own_index,
        )
        .expect("We should expect correct computation of the Merkle root for encoded transactions");
        Ok(transactions_commitment)
    }

    pub(crate) fn compute_merkle_root_and_proof(
        encoded_statements: &Vec<Shard>,
        own_index: AuthorityIndex,
    ) -> ConsensusResult<(TransactionsCommitment, MerkleProofBytes)> {
        let mut leaves: Vec<[u8; DefaultHashFunction::OUTPUT_SIZE]> = Vec::new();
        for shard in encoded_statements {
            let mut hasher = DefaultHashFunction::new();
            hasher.update(shard);
            let leaf = hasher.finalize().into();
            leaves.push(leaf);
        }
        let merkle_tree = MerkleTree::<DefaultHashFunctionWrapper>::from_leaves(&leaves);
        let merkle_root = merkle_tree
            .root()
            .ok_or("couldn't get the merkle root")
            .unwrap();

        let indices_to_prove = vec![own_index.value()];
        let merkle_proof = merkle_tree.proof(&indices_to_prove);
        let merkle_proof_bytes = merkle_proof.to_bytes();
        Ok((TransactionsCommitment(merkle_root), merkle_proof_bytes))
    }

    pub(crate) fn check_merkle_proof(
        shard: ShardWithProof,
        tree_size: usize,
        leaf_index: usize,
    ) -> bool {
        let mut hasher = DefaultHashFunction::new();
        hasher.update(shard.shard());
        let leaf = hasher.finalize().into();
        let proof =
            MerkleProof::<DefaultHashFunctionWrapper>::try_from(shard.proof().clone()).unwrap();
        proof.verify(
            shard.transaction_commitment().0,
            &[leaf_index],
            &[leaf],
            tree_size,
        )
    }
}

impl Hash for TransactionsCommitment {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.0[..8]);
    }
}

impl From<TransactionsCommitment> for Digest<{ DIGEST_LENGTH }> {
    fn from(hd: TransactionsCommitment) -> Self {
        Digest::new(hd.0)
    }
}

impl fmt::Display for TransactionsCommitment {
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

impl fmt::Debug for TransactionsCommitment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, self.0)
        )
    }
}

impl AsRef<[u8]> for TransactionsCommitment {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Slot is the position of blocks in the DAG. It can contain 0, 1 or multiple
/// blocks from the same authority at the same round.
#[derive(Clone, Copy, PartialEq, PartialOrd, Default, Hash)]
pub struct Slot {
    pub round: Round,
    pub authority: AuthorityIndex,
}

impl Slot {
    pub fn new(round: Round, authority: impl Into<AuthorityIndex>) -> Self {
        Self {
            round,
            authority: authority.into(),
        }
    }
}

impl From<BlockRef> for Slot {
    fn from(value: BlockRef) -> Self {
        Slot::new(value.round, value.author)
    }
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "S{}{}", self.round, self.authority)
    }
}

impl fmt::Debug for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// A BlockHeader with its signature, before they are verified.
///
/// Note: `BlockDigest` is computed over this struct, so any added field
/// (without `#[serde(skip)]`) will affect the values of `BlockDigest` and
/// `BlockRef`.
#[derive(Deserialize, Serialize, PartialOrd, PartialEq, Ord, Eq)]
pub(crate) struct SignedBlockHeader {
    inner: BlockHeader,
    signature: Bytes,
}

impl SignedBlockHeader {
    /// Should only be used when constructing the genesis block headers
    pub(crate) fn new_genesis(block_header: BlockHeader) -> Self {
        Self {
            inner: block_header,
            signature: Bytes::default(),
        }
    }

    pub(crate) fn new(
        block_header: BlockHeader,
        protocol_keypair: &ProtocolKeyPair,
    ) -> ConsensusResult<Self> {
        let signature = compute_block_header_signature(&block_header, protocol_keypair)?;
        Ok(Self {
            inner: block_header,
            signature: Bytes::copy_from_slice(signature.to_bytes()),
        })
    }

    pub(crate) fn signature(&self) -> &Bytes {
        &self.signature
    }

    /// This method only verifies this block header's signature. Verification of
    /// the full block header should be done via BlockHeaderVerifier.
    #[instrument(level = "trace", skip_all)]
    pub(crate) fn verify_signature(&self, context: &Context) -> ConsensusResult<()> {
        let block_header = &self.inner;
        ConsensusError::quick_validation_authority_indices(
            &[block_header.author()],
            &context.committee,
        )?;
        let authority = context.committee.authority(block_header.author());
        verify_block_header_signature(block_header, self.signature(), &authority.protocol_key)
    }

    /// Serialises the block header using the bcs serializer
    pub(crate) fn serialize(&self) -> Result<Bytes, bcs::Error> {
        let bytes = bcs::to_bytes(self)?;
        Ok(bytes.into())
    }

    /// Clears signature for testing.
    #[cfg(test)]
    pub(crate) fn clear_signature(&mut self) {
        self.signature = Bytes::default();
    }
}

/// Digest of a block header, covering all `BlockHeader` fields (no signature).
/// This is used during BlockHeader signing and signature verification.
/// This should never be used outside of this file, to avoid confusion with
/// `BlockDigest`.
#[derive(Serialize, Deserialize)]
struct InnerBlockHeaderDigest([u8; DIGEST_LENGTH]);

/// Computes the digest of a Block, only for signing and verifications.
fn compute_inner_block_header_digest(
    block_header: &BlockHeader,
) -> ConsensusResult<InnerBlockHeaderDigest> {
    let mut hasher = DefaultHashFunction::new();
    hasher.update(bcs::to_bytes(block_header).map_err(ConsensusError::SerializationFailure)?);
    Ok(InnerBlockHeaderDigest(hasher.finalize().into()))
}

/// Wrap a InnerBlockDigest in the intent message.
fn to_consensus_block_header_intent(
    digest: InnerBlockHeaderDigest,
) -> IntentMessage<InnerBlockHeaderDigest> {
    IntentMessage::new(Intent::consensus_app(IntentScope::ConsensusBlock), digest)
}

/// Process for signing & verifying a block signature:
/// 1. Compute the digest of `BlockHeader`.
/// 2. Wrap the digest in `IntentMessage`.
/// 3. Sign the serialized `IntentMessage`, or verify the signature against it.
#[tracing::instrument(level = "trace", skip_all)]
fn compute_block_header_signature(
    block_header: &BlockHeader,
    protocol_keypair: &ProtocolKeyPair,
) -> ConsensusResult<ProtocolKeySignature> {
    let digest = compute_inner_block_header_digest(block_header)?;
    let message = bcs::to_bytes(&to_consensus_block_header_intent(digest))
        .map_err(ConsensusError::SerializationFailure)?;
    Ok(protocol_keypair.sign(&message))
}
#[tracing::instrument(level = "trace", skip_all)]
fn verify_block_header_signature(
    block_header: &BlockHeader,
    signature: &[u8],
    protocol_pubkey: &ProtocolPublicKey,
) -> ConsensusResult<()> {
    let digest = compute_inner_block_header_digest(block_header)?;
    let message = bcs::to_bytes(&to_consensus_block_header_intent(digest))
        .map_err(ConsensusError::SerializationFailure)?;
    let sig =
        ProtocolKeySignature::from_bytes(signature).map_err(ConsensusError::MalformedSignature)?;
    protocol_pubkey
        .verify(&message, &sig)
        .map_err(ConsensusError::SignatureVerificationFailure)
}

/// Allow quick access on the underlying BlockHeader without having to always
/// refer to the inner block ref.
impl Deref for SignedBlockHeader {
    type Target = BlockHeader;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// VerifiedBlock allows full access to its content.
/// Note: clone() is relatively cheap with most underlying data refcounted.
#[derive(Clone, PartialOrd, Ord, Eq)]
pub struct VerifiedBlockHeader {
    signed_block_header: Arc<SignedBlockHeader>,

    // Cached Block digest and serialized SignedBlock, to avoid re-computing these values.
    digest: BlockHeaderDigest,
    serialized: Bytes,
}

impl VerifiedBlockHeader {
    /// Creates VerifiedBlockHeader from a verified SignedBlockHeader and its
    /// serialized bytes.
    pub(crate) fn new_verified(signed_block_header: SignedBlockHeader, serialized: Bytes) -> Self {
        let digest = Self::compute_digest(&serialized);
        VerifiedBlockHeader {
            signed_block_header: Arc::new(signed_block_header),
            digest,
            serialized,
        }
    }

    pub(crate) fn new_verified_with_digest(
        signed_block_header: SignedBlockHeader,
        serialized: Bytes,
        digest: BlockHeaderDigest,
    ) -> Self {
        VerifiedBlockHeader {
            signed_block_header: Arc::new(signed_block_header),
            digest,
            serialized,
        }
    }

    pub(crate) fn new_from_bytes(serialized_block_header: Bytes) -> ConsensusResult<Self> {
        let signed_block_header: SignedBlockHeader =
            bcs::from_bytes(&serialized_block_header).map_err(ConsensusError::MalformedHeader)?;

        // Only accepted blocks should have been written to storage.
        Ok(VerifiedBlockHeader::new_verified(
            signed_block_header,
            serialized_block_header,
        ))
    }

    /// This method is public for testing in other crates.
    pub fn new_for_test(block_header: BlockHeader) -> Self {
        let signed_block_header = SignedBlockHeader {
            inner: block_header,
            signature: Default::default(),
        };
        let serialized: Bytes = bcs::to_bytes(&signed_block_header)
            .expect("Serialization should not fail")
            .into();
        let digest = Self::compute_digest(&serialized);
        VerifiedBlockHeader {
            signed_block_header: Arc::new(signed_block_header),
            digest,
            serialized,
        }
    }

    #[cfg(test)]
    pub fn new_from_header_with_signature(
        block_header: BlockHeader,
        protocol_keypair: &ProtocolKeyPair,
    ) -> Self {
        let signed_block_header = SignedBlockHeader::new(block_header, protocol_keypair).unwrap();
        let serialized: Bytes = bcs::to_bytes(&signed_block_header)
            .expect("Serialization should not fail")
            .into();
        let digest = Self::compute_digest(&serialized);
        VerifiedBlockHeader {
            signed_block_header: Arc::new(signed_block_header),
            digest,
            serialized,
        }
    }

    /// Returns reference to the block.
    pub fn reference(&self) -> BlockRef {
        BlockRef {
            round: self.round(),
            author: self.author(),
            digest: self.digest(),
        }
    }

    /// Returns transaction reference to transactions from the block.
    pub fn transaction_ref(&self) -> TransactionRef {
        TransactionRef {
            round: self.round(),
            author: self.author(),
            transactions_commitment: self.signed_block_header.inner.transactions_commitment(),
        }
    }

    pub(crate) fn digest(&self) -> BlockHeaderDigest {
        self.digest
    }

    pub(crate) fn transactions_commitment(&self) -> TransactionsCommitment {
        self.signed_block_header.inner.transactions_commitment()
    }

    /// Returns the serialization of the signed block header.
    pub(crate) fn serialized(&self) -> &Bytes {
        &self.serialized
    }

    /// Computes digest from the serialization of the signed block header.
    pub(crate) fn compute_digest(serialized: &[u8]) -> BlockHeaderDigest {
        let mut hasher = DefaultHashFunction::new();
        hasher.update(serialized);
        BlockHeaderDigest(hasher.finalize().into())
    }
}

/// Allow quick access on the underlying Block header without having to always
/// refer to the inner block ref.
impl Deref for VerifiedBlockHeader {
    type Target = BlockHeader;

    fn deref(&self) -> &Self::Target {
        &self.signed_block_header.inner
    }
}

impl PartialEq for VerifiedBlockHeader {
    fn eq(&self, other: &Self) -> bool {
        self.digest() == other.digest()
    }
}

impl fmt::Display for VerifiedBlockHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}", self.reference())
    }
}

impl fmt::Debug for VerifiedBlockHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{:?}({}ms;{:?}r;{:?}a;{}c)",
            self.reference(),
            self.timestamp_ms(),
            self.ancestors(),
            self.acknowledgments(),
            self.commit_votes().len(),
        )
    }
}

/// VerifiedTransactions are transactions that correspond to an existing block
#[derive(Clone, Debug)]
pub struct VerifiedTransactions {
    transactions: Vec<Transaction>,

    /// Commitment of transactions in the block
    transaction_ref: TransactionRef,

    /// Digest of the block this transaction batch belongs to.
    /// Present (`Some`) whenever the block header is available at
    /// construction time, regardless of the `consensus_fast_commit_sync` flag.
    /// `None` only when transactions were received without an accompanying
    /// block header (e.g., fast sync or store loading via TransactionRef).
    block_digest: Option<BlockHeaderDigest>,

    /// The serialized bytes of the transactions.
    serialized: Bytes,
}

impl PartialEq for VerifiedTransactions {
    fn eq(&self, other: &Self) -> bool {
        self.transactions_commitment() == other.transactions_commitment()
    }
}

impl VerifiedTransactions {
    pub(crate) fn new(
        transactions: Vec<Transaction>,
        transaction_ref: TransactionRef,
        block_digest: Option<BlockHeaderDigest>,
        serialized: Bytes,
    ) -> Self {
        Self {
            transactions,
            transaction_ref,
            block_digest,
            serialized,
        }
    }

    pub fn transactions_commitment(&self) -> TransactionsCommitment {
        self.transaction_ref.transactions_commitment
    }

    pub fn round(&self) -> Round {
        self.transaction_ref.round
    }

    pub fn author(&self) -> AuthorityIndex {
        self.transaction_ref.author
    }

    /// Returns the block ref if `block_digest` is set (i.e., when using
    /// BlockRef-based fetching).
    pub fn block_ref(&self) -> Option<BlockRef> {
        self.block_digest.map(|digest| BlockRef {
            round: self.transaction_ref.round,
            author: self.transaction_ref.author,
            digest,
        })
    }

    /// Returns the block digest if available; `None` when using
    /// TransactionRef-based fetching.
    pub fn block_digest(&self) -> Option<BlockHeaderDigest> {
        self.block_digest
    }

    pub fn transaction_ref(&self) -> TransactionRef {
        self.transaction_ref
    }

    /// Returns the leader round of the sub-dag.
    pub fn transactions(&self) -> Vec<Transaction> {
        self.transactions.clone()
    }

    pub fn serialized(&self) -> &Bytes {
        &self.serialized
    }

    pub fn has_transactions(&self) -> bool {
        !self.transactions.is_empty()
    }
}

/// VerifiedBlock is a pair of verified block header and transactions. It is
/// used for streaming and storing
#[derive(Clone, Debug, PartialEq)]
pub struct VerifiedBlock {
    /// The block header.
    pub verified_block_header: VerifiedBlockHeader,

    /// The transactions in the block.
    pub verified_transactions: VerifiedTransactions,
}

impl VerifiedBlock {
    pub fn new(
        verified_block_header: VerifiedBlockHeader,
        verified_transactions: VerifiedTransactions,
    ) -> Self {
        Self {
            verified_block_header,
            verified_transactions,
        }
    }

    #[cfg(test)]
    pub fn new_for_test(block_header: BlockHeader) -> Self {
        let verified_block_header = VerifiedBlockHeader::new_for_test(block_header);
        let verified_transactions = VerifiedTransactions::new(
            vec![],
            verified_block_header.transaction_ref(),
            Some(verified_block_header.digest()),
            Bytes::from(bcs::to_bytes::<Vec<Transaction>>(&vec![]).unwrap()),
        );
        Self {
            verified_block_header,
            verified_transactions,
        }
    }

    #[cfg(test)]
    pub fn new_with_transaction_for_test(block_header: BlockHeader, tx: u8) -> Self {
        let verified_block_header = VerifiedBlockHeader::new_for_test(block_header);
        let verified_transactions = VerifiedTransactions::new(
            vec![],
            verified_block_header.transaction_ref(),
            Some(verified_block_header.digest()),
            Bytes::from(
                bcs::to_bytes::<Vec<Transaction>>(
                    &vec![vec![tx; 16]]
                        .into_iter()
                        .map(Transaction::new)
                        .collect(),
                )
                .unwrap(),
            ),
        );
        Self {
            verified_block_header,
            verified_transactions,
        }
    }

    // This function returns a pair of serialized block header and serialized
    // transactions
    pub fn serialized(&self) -> (&Bytes, &Bytes) {
        (
            &self.verified_block_header.serialized,
            &self.verified_transactions.serialized,
        )
    }
}

/// Allow quick access to the underlying BlockHeader without having to always
/// refer to the inner block ref.
impl Deref for VerifiedBlock {
    type Target = VerifiedBlockHeader;

    fn deref(&self) -> &Self::Target {
        &self.verified_block_header
    }
}

/// Generates the genesis blocks for the current Committee.
/// The blocks are returned in authority index order.
pub(crate) fn genesis_blocks(context: &Context) -> Vec<VerifiedBlock> {
    context
        .committee
        .authorities()
        .map(|(authority_index, _)| {
            let signed_block = SignedBlockHeader::new_genesis(BlockHeader::V1(
                BlockHeaderV1::genesis_block_header(context, authority_index),
            ));
            let serialized = signed_block
                .serialize()
                .expect("Genesis block serialization failed.");
            // Unnecessary to verify genesis block headers.
            let verified_block_header = VerifiedBlockHeader::new_verified(signed_block, serialized);
            VerifiedBlock {
                verified_block_header: verified_block_header.clone(),
                verified_transactions: VerifiedTransactions::new(
                    vec![],
                    verified_block_header.transaction_ref(),
                    Some(verified_block_header.digest()),
                    Bytes::from(bcs::to_bytes::<Vec<Transaction>>(&vec![]).unwrap()),
                ),
            }
        })
        .collect::<Vec<VerifiedBlock>>()
}

pub(crate) fn genesis_block_headers(context: &Context) -> Vec<VerifiedBlockHeader> {
    context
        .committee
        .authorities()
        .map(|(authority_index, _)| {
            let signed_block = SignedBlockHeader::new_genesis(BlockHeader::V1(
                BlockHeaderV1::genesis_block_header(context, authority_index),
            ));
            let serialized = signed_block
                .serialize()
                .expect("Genesis block serialization failed.");
            // Unnecessary to verify genesis block headers.
            VerifiedBlockHeader::new_verified(signed_block, serialized)
        })
        .collect::<Vec<VerifiedBlockHeader>>()
}

/// This struct is public for testing in other crates.
#[derive(Clone)]
pub struct TestBlockHeader {
    ancestors: Vec<BlockRef>,
    acknowledgments: Vec<BlockRef>,
    block_header: BlockHeaderV1,
}

impl TestBlockHeader {
    /// Creates a simple block with no transactions and without real computation
    /// of transactions commitment. Use it when you don't need to check the
    /// commitment and don't want to create and pass the encoder.
    pub fn new(round: Round, author: u8) -> Self {
        Self {
            block_header: BlockHeaderV1 {
                round,
                author: author.into(),
                transactions_commitment: TransactionsCommitment::DEFAULT_FOR_TEST,
                ..Default::default()
            },
            ancestors: vec![],
            acknowledgments: vec![],
        }
    }

    #[cfg(test)]
    pub(crate) fn new_with_commitment(
        round: Round,
        author: u8,
        context: &Arc<Context>,
        encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
    ) -> Self {
        let txs = vec![];
        let serialized_transactions = Transaction::serialize(&txs)
            .expect("We should expect correct serialization of the transactions");
        Self {
            block_header: BlockHeaderV1 {
                round,
                author: author.into(),
                transactions_commitment: TransactionsCommitment::compute_transactions_commitment(
                    &serialized_transactions,
                    context,
                    encoder,
                )
                .unwrap(),
                ..Default::default()
            },
            ancestors: vec![],
            acknowledgments: vec![],
        }
    }

    #[cfg(test)]
    pub(crate) fn new_with_transaction(
        round: Round,
        author: u8,
        tx: u8,
        context: &Arc<Context>,
        encoder: &mut Box<dyn ShardEncoder + Send + Sync>,
    ) -> Self {
        let txs = vec![vec![tx; 16]]
            .into_iter()
            .map(Transaction::new)
            .collect::<Vec<Transaction>>();
        let serialized_transactions = Transaction::serialize(&txs)
            .expect("We should expect correct serialization of the transactions for sharding");
        Self {
            block_header: BlockHeaderV1 {
                round,
                author: author.into(),
                transactions_commitment: TransactionsCommitment::compute_transactions_commitment(
                    &serialized_transactions,
                    context,
                    encoder,
                )
                .unwrap(),
                ..Default::default()
            },
            ancestors: vec![],
            acknowledgments: vec![],
        }
    }

    pub fn set_epoch(mut self, epoch: Epoch) -> Self {
        self.block_header.epoch = epoch;
        self
    }

    pub fn set_round(mut self, round: Round) -> Self {
        self.block_header.round = round;
        self
    }

    pub fn set_author(mut self, author: AuthorityIndex) -> Self {
        self.block_header.author = author;
        self
    }

    pub fn set_timestamp_ms(mut self, timestamp_ms: BlockTimestampMs) -> Self {
        self.block_header.timestamp_ms = timestamp_ms;
        self
    }

    pub fn set_ancestors(mut self, ancestors: Vec<BlockRef>) -> Self {
        self.ancestors = ancestors;
        self
    }

    pub fn set_acknowledgments(mut self, acknowledgments: Vec<BlockRef>) -> Self {
        self.acknowledgments = acknowledgments;
        self
    }

    pub fn set_commit_votes(mut self, commit_votes: Vec<CommitVote>) -> Self {
        self.block_header.commit_votes = commit_votes;
        self
    }

    pub fn set_commitment(mut self, commitment: TransactionsCommitment) -> Self {
        self.block_header.transactions_commitment = commitment;
        self
    }

    pub fn build(mut self) -> BlockHeader {
        let (references, overlap_start_index, overlap_end_index) =
            BlockHeaderV1::compress_references(self.ancestors, self.acknowledgments);
        self.block_header.references = references;
        self.block_header.overlap_start_index = overlap_start_index;
        self.block_header.overlap_end_index = overlap_end_index;

        BlockHeader::V1(self.block_header)
    }
}

// TODO: add basic verification for BlockRef and BlockDigest.
// TODO: add tests for SignedBlock and VerifiedBlock conversion.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fastcrypto::error::FastCryptoError;

    use crate::{
        BlockHeaderAPI,
        block_header::{
            BlockHeaderDigest, SignedBlockHeader, TestBlockHeader, genesis_block_headers,
        },
        context::Context,
        error::ConsensusError,
    };

    #[tokio::test]
    async fn test_sign_and_verify() {
        let (context, key_pairs) = Context::new_for_test(4);
        let context = Arc::new(context);

        // Create a block header by authority 2
        let block_header = TestBlockHeader::new(10, 2).build();

        // Create a signed block with authority's 2 private key
        let author_two_key = &key_pairs[2].1;
        let signed_block_header =
            SignedBlockHeader::new(block_header, author_two_key).expect("Shouldn't fail signing");

        // Now verify the block's signature
        let result = signed_block_header.verify_signature(&context);
        assert!(result.is_ok());

        // Try to sign authority's 2 block header with authority's 1 key
        let block_header = TestBlockHeader::new(10, 2).build();
        let author_one_key = &key_pairs[1].1;
        let signed_block_header =
            SignedBlockHeader::new(block_header, author_one_key).expect("Shouldn't fail signing");

        // Now verify the block, it should fail
        let result = signed_block_header.verify_signature(&context);
        match result.err().unwrap() {
            ConsensusError::SignatureVerificationFailure(err) => {
                assert_eq!(err, FastCryptoError::InvalidSignature);
            }
            err => panic!("Unexpected error: {err:?}"),
        }
    }

    #[tokio::test]
    async fn test_genesis_block_headers() {
        let (context, _) = Context::new_for_test(4);
        const TIMESTAMP_MS: u64 = 1000;
        let context = Arc::new(context.with_epoch_start_timestamp_ms(TIMESTAMP_MS));
        let block_headers = genesis_block_headers(&context);
        for (i, header) in block_headers.into_iter().enumerate() {
            assert_eq!(header.author().value(), i);
            assert_eq!(header.round(), 0);
            assert_eq!(header.timestamp_ms(), TIMESTAMP_MS);
        }
    }

    #[tokio::test]
    async fn test_compress_references() {
        use crate::block_header::BlockRef;
        let rng = &mut rand::thread_rng();

        let ref_a = BlockRef::new(1, 0.into(), BlockHeaderDigest::random(&mut *rng));
        let ref_b = BlockRef::new(1, 1.into(), BlockHeaderDigest::random(&mut *rng));
        let ref_c = BlockRef::new(1, 2.into(), BlockHeaderDigest::random(&mut *rng));
        let ref_d = BlockRef::new(1, 3.into(), BlockHeaderDigest::random(&mut *rng));
        let ref_e = BlockRef::new(1, 4.into(), BlockHeaderDigest::random(&mut *rng));

        // Test case 1: No overlap
        let ancestors = vec![ref_a, ref_b];
        let acknowledgments = vec![ref_c, ref_d];
        let (references, overlap_start_index, overlap_end_index) =
            crate::block_header::BlockHeaderV1::compress_references(ancestors, acknowledgments);
        let expected = [ref_a, ref_b, ref_c, ref_d];
        assert_eq!(references.len(), expected.len());
        for r in references.iter() {
            assert!(expected.contains(r));
        }
        assert_eq!(overlap_start_index, 2);
        assert_eq!(overlap_end_index, 2);
        assert_eq!(*references.first().unwrap(), ref_a);

        // Test case 2: Some overlap
        let ancestors = vec![ref_a, ref_b, ref_c];
        let acknowledgments = vec![ref_c, ref_d];
        let (references, overlap_start_index, overlap_end_index) =
            crate::block_header::BlockHeaderV1::compress_references(ancestors, acknowledgments);
        let expected = [ref_a, ref_b, ref_c, ref_d];
        assert_eq!(references.len(), expected.len());
        for r in references.iter() {
            assert!(expected.contains(r));
        }
        assert_eq!(overlap_start_index, 2);
        assert_eq!(overlap_end_index, 3);
        assert_eq!(*references.first().unwrap(), ref_a);

        // Some Overlap with ref0 in ack
        let ancestors = vec![ref_a, ref_b, ref_c, ref_d];
        let acknowledgments = vec![ref_a, ref_c, ref_d, ref_e];

        let (references, overlap_start_index, overlap_end_index) =
            crate::block_header::BlockHeaderV1::compress_references(ancestors, acknowledgments);

        let expected = [ref_a, ref_b, ref_c, ref_d, ref_e, ref_a];
        assert_eq!(references.len(), expected.len());
        for r in references.iter() {
            assert!(expected.contains(r));
        }

        assert_eq!(overlap_start_index, 2);
        assert_eq!(overlap_end_index, 4);
        assert_eq!(*references.first().unwrap(), ref_a);
        assert_eq!(*references.last().unwrap(), ref_a);

        // Test case 3: Full overlap
        let ancestors = vec![ref_a, ref_b, ref_c];
        let acknowledgments = vec![ref_a, ref_b, ref_c];
        let (references, overlap_start_index, overlap_end_index) =
            crate::block_header::BlockHeaderV1::compress_references(
                ancestors.clone(),
                acknowledgments.clone(),
            );

        let expected = [ref_a, ref_b, ref_c, ref_a];
        assert_eq!(references.len(), expected.len());
        for r in references.iter() {
            assert!(expected.contains(r));
        }
        assert_eq!(overlap_start_index, 1);
        assert_eq!(overlap_end_index, 3);
        assert_eq!(*references.first().unwrap(), ref_a);
        assert_eq!(*references.last().unwrap(), ref_a);

        // Verify that decompressing references gives back the original ancestors and
        // acknowledgments
        let compressed_ancestors = &references[..overlap_end_index as usize];
        let compressed_acknowledgments = &references[overlap_start_index as usize..];
        assert_eq!(compressed_ancestors, ancestors.as_slice());
        assert_eq!(compressed_acknowledgments.len(), acknowledgments.len());
        // ordering of acknowledgments may not be preserved
        for ack in acknowledgments.iter() {
            assert!(compressed_acknowledgments.contains(ack));
        }
    }
}
