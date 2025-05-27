// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    sync::Arc,
};

use bytes::Bytes;
use enum_dispatch::enum_dispatch;
use fastcrypto::hash::{Digest, HashFunction};
use serde::{Deserialize, Serialize};
use shared_crypto::intent::{Intent, IntentMessage, IntentScope};
use starfish_config::{
    AuthorityIndex, DIGEST_LENGTH, DefaultHashFunction, Epoch, ProtocolKeyPair,
    ProtocolKeySignature, ProtocolPublicKey,
};

use crate::{
    commit::CommitVote,
    context::Context,
    ensure,
    error::{ConsensusError, ConsensusResult},
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
    pub(crate) fn serialize(transactions: &[Transaction]) -> Result<Bytes, bcs::Error> {
        let bytes = bcs::to_bytes(transactions)?;
        Ok(bytes.into())
    }
}

/// A block header includes references to previous round blocks and a commitment
/// to transactions that the authority considers valid.
/// Well behaved authorities produce at most one block header per round, but
/// malicious authorities can equivocate.
#[derive(Clone, Deserialize, Serialize)]
#[enum_dispatch(BlockHeaderAPI)]
pub enum BlockHeader {
    V1(BlockHeaderV1),
}

#[enum_dispatch]
pub trait BlockHeaderAPI {
    fn epoch(&self) -> Epoch;
    fn round(&self) -> Round;
    fn author(&self) -> AuthorityIndex;
    fn slot(&self) -> Slot;
    fn acknowledgments(&self) -> &[BlockRef];
    fn timestamp_ms(&self) -> BlockTimestampMs;
    fn ancestors(&self) -> &[BlockRef];
    fn commit_votes(&self) -> &[CommitVote];

    // TODO: we should remove this method from the API of the block header and use
    // another type of a full block in the consensus output
    fn transactions(&self) -> &[Transaction];
    fn transactions_commitment(&self) -> TransactionsCommitment;
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct BlockHeaderV1 {
    epoch: Epoch,
    round: Round,
    author: AuthorityIndex,
    // TODO: during verification ensure that timestamp_ms >= ancestors.timestamp
    timestamp_ms: BlockTimestampMs,
    // ancestors are BlockRefs such that there are at least 2f+1 BlockRefs (by stake) from the
    // previous round
    ancestors: Vec<BlockRef>,
    // acknowledgments are BlockRefs for blocks for which a validator acknowledges data
    // availability of transactions
    // TODO: we should compress it together with ancestors to
    // avoid duplications since in most cases these sets have a big overlap
    acknowledgments: Vec<BlockRef>,
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
        Self {
            epoch,
            round,
            author,
            timestamp_ms,
            ancestors,
            acknowledgments,
            transactions_commitment,
            commit_votes,
        }
    }

    fn genesis_block(epoch: Epoch, author: AuthorityIndex) -> Self {
        Self {
            epoch,
            round: GENESIS_ROUND,
            author,
            timestamp_ms: 0,
            ancestors: vec![],
            acknowledgments: vec![],
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

    fn timestamp_ms(&self) -> BlockTimestampMs {
        self.timestamp_ms
    }

    fn ancestors(&self) -> &[BlockRef] {
        &self.ancestors
    }
    fn acknowledgments(&self) -> &[BlockRef] {
        &self.acknowledgments
    }

    fn commit_votes(&self) -> &[CommitVote] {
        &self.commit_votes
    }

    fn transactions_commitment(&self) -> TransactionsCommitment {
        self.transactions_commitment
    }

    fn transactions(&self) -> &[Transaction] {
        unimplemented!();
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

// TODO: re-evaluate formats for production debugging.
impl fmt::Display for BlockRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "B{}({},{})", self.round, self.author, self.digest)
    }
}

impl fmt::Debug for BlockRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "B{}({},{:?})", self.round, self.author, self.digest)
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

// TODO: we might need to join TransactionDigest with BlockDigest since we use
// the same parameters for both structures. TransactionDigest is used for
// including a commitment for a transaction data to a block header. This digest
// is used for BlockDigest computations of BlockHeader does not include
// explicitly the transaction data.
#[derive(Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TransactionsCommitment([u8; starfish_config::DIGEST_LENGTH]);

impl TransactionsCommitment {
    /// Lexicographic min & max digest.
    pub const MIN: Self = Self([u8::MIN; starfish_config::DIGEST_LENGTH]);
    pub const MAX: Self = Self([u8::MAX; starfish_config::DIGEST_LENGTH]);
    pub(crate) fn compute_transactions_commitment(
        serialized_transactions: &Bytes,
    ) -> ConsensusResult<TransactionsCommitment> {
        let mut hasher = DefaultHashFunction::new();
        hasher.update(serialized_transactions);
        Ok(TransactionsCommitment(hasher.finalize().into()))
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
    pub fn new(round: Round, authority: AuthorityIndex) -> Self {
        Self { round, authority }
    }

    #[cfg(test)]
    pub fn new_for_test(round: Round, authority: u32) -> Self {
        Self {
            round,
            authority: AuthorityIndex::new_for_test(authority),
        }
    }
}

impl From<BlockRef> for Slot {
    fn from(value: BlockRef) -> Self {
        Slot::new(value.round, value.author)
    }
}

// TODO: re-evaluate formats for production debugging.
impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.authority, self.round,)
    }
}

impl fmt::Debug for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}

/// A BlockHeader with its signature, before they are verified.
///
/// Note: `BlockDigest` is computed over this struct, so any added field
/// (without `#[serde(skip)]`) will affect the values of `BlockDigest` and
/// `BlockRef`.
#[derive(Deserialize, Serialize)]
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
    pub(crate) fn verify_signature(&self, context: &Context) -> ConsensusResult<()> {
        let block_header = &self.inner;
        let committee = &context.committee;
        ensure!(
            committee.is_valid_index(block_header.author()),
            ConsensusError::InvalidAuthorityIndex {
                index: block_header.author(),
                max: committee.size() - 1
            }
        );
        let authority = committee.authority(block_header.author());
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
struct InnerBlockHeaderDigest([u8; starfish_config::DIGEST_LENGTH]);

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
fn compute_block_header_signature(
    block_header: &BlockHeader,
    protocol_keypair: &ProtocolKeyPair,
) -> ConsensusResult<ProtocolKeySignature> {
    let digest = compute_inner_block_header_digest(block_header)?;
    let message = bcs::to_bytes(&to_consensus_block_header_intent(digest))
        .map_err(ConsensusError::SerializationFailure)?;
    Ok(protocol_keypair.sign(&message))
}
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
#[derive(Clone)]
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

    /// Returns reference to the block.
    pub fn reference(&self) -> BlockRef {
        BlockRef {
            round: self.round(),
            author: self.author(),
            digest: self.digest(),
        }
    }

    pub(crate) fn digest(&self) -> BlockHeaderDigest {
        self.digest
    }

    #[cfg_attr(not(test), expect(unused))]
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

// TODO: re-evaluate formats for production debugging.
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
#[derive(Clone)]
pub struct VerifiedTransactions {
    #[expect(dead_code)]
    transactions: Vec<Transaction>,

    /// The block reference of the block that contains the transactions.
    block_ref: BlockRef,

    /// Commitment of transactions in the block
    transactions_commitment: TransactionsCommitment,

    /// The serialized bytes of the transactions.
    #[expect(dead_code)]
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
        block_ref: BlockRef,
        transactions_commitment: TransactionsCommitment,
        serialized: Bytes,
    ) -> Self {
        Self {
            transactions,
            block_ref,
            transactions_commitment,
            serialized,
        }
    }

    pub fn transactions_commitment(&self) -> TransactionsCommitment {
        self.transactions_commitment
    }

    pub fn block_ref(&self) -> BlockRef {
        self.block_ref
    }
}

/// Generates the genesis blocks for the current Committee.
/// The blocks are returned in authority index order.
pub(crate) fn genesis_block_headers(context: Arc<Context>) -> Vec<VerifiedBlockHeader> {
    context
        .committee
        .authorities()
        .map(|(authority_index, _)| {
            let signed_block = SignedBlockHeader::new_genesis(BlockHeader::V1(
                BlockHeaderV1::genesis_block(context.committee.epoch(), authority_index),
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
    block_header: BlockHeaderV1,
}

impl TestBlockHeader {
    pub fn new(round: Round, author: u32) -> Self {
        Self {
            block_header: BlockHeaderV1 {
                round,
                author: AuthorityIndex::new_for_test(author),
                ..Default::default()
            },
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
        self.block_header.ancestors = ancestors;
        self
    }

    pub fn set_commit_votes(mut self, commit_votes: Vec<CommitVote>) -> Self {
        self.block_header.commit_votes = commit_votes;
        self
    }

    pub fn set_acknowledgments(mut self, acknowledgments: Vec<BlockRef>) -> Self {
        self.block_header.acknowledgments = acknowledgments;
        self
    }

    pub fn build(self) -> BlockHeader {
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
        block_header::{SignedBlockHeader, TestBlockHeader},
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
}
