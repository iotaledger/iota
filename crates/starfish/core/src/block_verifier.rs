// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use tracing::instrument;

use crate::{
    Transaction,
    block_header::{
        BlockHeader, BlockHeaderAPI, BlockRef, CommitmentVerifiedTransactions, GENESIS_ROUND,
        SignedBlockHeader, genesis_block_headers,
    },
    context::Context,
    encoder::shard_bytes,
    error::{ConsensusError, ConsensusResult},
    transaction::{
        BlockTransactionLimitError, BlockTransactionLimits, TransactionVerifier,
        check_transactions_fit,
    },
};

pub(crate) trait BlockVerifier: Send + Sync + 'static {
    /// Verifies a block's metadata and transactions.
    /// This is called before examining a block's causal history.
    fn verify(&self, block: &SignedBlockHeader) -> ConsensusResult<()>;

    /// Bounds and parses a serialized transaction payload. Call this before
    /// any expensive processing of the payload; failures cannot be attributed
    /// to the block author until the payload is bound to the header's
    /// transactions commitment, so they are sender faults.
    fn check_and_parse_transactions(
        &self,
        serialized_transactions: &[u8],
    ) -> ConsensusResult<Vec<Transaction>>;

    /// Checks protocol limits on the transactions and runs the application's
    /// transaction verifier.
    fn verify_transactions_validity(
        &self,
        transactions: &CommitmentVerifiedTransactions,
    ) -> ConsensusResult<()>;
}

/// `SignedBlockVerifier` checks the validity of a block.
///
/// Blocks that fail verification at one honest authority will be rejected by
/// all other honest authorities as well. The means invalid blocks, and blocks
/// with an invalid ancestor, will never be accepted into the DAG.
pub(crate) struct SignedBlockVerifier {
    context: Arc<Context>,
    genesis: BTreeSet<BlockRef>,
    transaction_verifier: Arc<dyn TransactionVerifier>,
}

impl SignedBlockVerifier {
    pub(crate) fn new(
        context: Arc<Context>,
        transaction_verifier: Arc<dyn TransactionVerifier>,
    ) -> Self {
        let genesis = genesis_block_headers(&context)
            .into_iter()
            .map(|b| b.reference())
            .collect();
        Self {
            context,
            genesis,
            transaction_verifier,
        }
    }

    /// Checks that the transactions of `batch` fit one block, by the rule of
    /// [`check_transactions_fit()`] and the limits of the verifier's protocol
    /// config.
    ///
    /// # Errors
    ///
    /// [`ConsensusError::TooManyTransactions`],
    /// [`ConsensusError::TransactionTooLarge`] or
    /// [`ConsensusError::TooManyTransactionBytes`], for the limit that
    /// [`check_transactions_fit()`] reports.
    pub(crate) fn check_transactions(&self, batch: &[&[u8]]) -> ConsensusResult<()> {
        let transaction_sizes: Vec<u64> = batch
            .iter()
            .map(|transaction| transaction.len() as u64)
            .collect();
        let limits = BlockTransactionLimits::new(&self.context.protocol_config);

        check_transactions_fit(&transaction_sizes, limits).map_err(to_consensus_error)
    }
}

/// Returns the [`ConsensusError`] for the limit that `error` reports, with the
/// same values.
fn to_consensus_error(error: BlockTransactionLimitError) -> ConsensusError {
    match error {
        BlockTransactionLimitError::TransactionTooLarge { size, limit } => {
            ConsensusError::TransactionTooLarge {
                size: size as usize,
                limit: limit as usize,
            }
        }
        BlockTransactionLimitError::TooManyTransactions { count, limit } => {
            ConsensusError::TooManyTransactions {
                count: count as usize,
                limit: limit as usize,
            }
        }
        BlockTransactionLimitError::TooManyTransactionBytes { size, limit } => {
            ConsensusError::TooManyTransactionBytes {
                size: size as usize,
                limit: limit as usize,
            }
        }
    }
}

/// Rejects a deserialized `BlockHeader` that is not V2, the only variant
/// honest peers propose.
pub(crate) fn check_block_header_version(header: &BlockHeader) -> ConsensusResult<()> {
    match header {
        BlockHeader::V1(_) => Err(ConsensusError::WrongBlockHeaderVersion { actual: "V1" }),
        BlockHeader::V2(_) => Ok(()),
    }
}

// All block verification logic are implemented below.
impl BlockVerifier for SignedBlockVerifier {
    #[instrument(level = "trace", skip_all)]
    fn verify(&self, block: &SignedBlockHeader) -> ConsensusResult<()> {
        let committee = &self.context.committee;
        // The block must belong to the current epoch and have valid authority index,
        // before having its signature verified.
        if block.epoch() != committee.epoch() {
            return Err(ConsensusError::WrongEpoch {
                expected: committee.epoch(),
                actual: block.epoch(),
            });
        }
        if block.round() == 0 {
            return Err(ConsensusError::UnexpectedGenesisHeader);
        }
        ConsensusError::quick_validation_authority_indices(&[block.author()], committee)?;

        check_block_header_version(block)?;

        // Verify the block's signature.
        block.verify_signature(&self.context)?;

        // Validate overlap indices before accessing ancestors() or acknowledgments()
        // to prevent panics from adversarial index values in deserialized headers.
        block.verify_references_indices()?;

        // Verify the block's ancestor refs are consistent with the block's round,
        // and total parent stakes reach quorum.
        if block.ancestors().len() > committee.size() {
            return Err(ConsensusError::TooManyAncestors(
                block.ancestors().len(),
                committee.size(),
            ));
        }

        if block.ancestors().is_empty() {
            return Err(ConsensusError::InsufficientParentStakes {
                parent_stakes: 0,
                quorum: committee.quorum_threshold(),
            });
        }
        let max_acknowledgments = self
            .context
            .protocol_config
            .max_acknowledgments_per_block(committee.size());
        if block.acknowledgments().len() > max_acknowledgments {
            return Err(ConsensusError::TooManyAcknowledgments {
                count: block.acknowledgments().len(),
                max: max_acknowledgments,
            });
        }
        let max_commit_votes = self
            .context
            .protocol_config
            .max_commit_votes_per_block(committee.size());
        if block.commit_votes().len() > max_commit_votes {
            return Err(ConsensusError::TooManyCommitVotes {
                count: block.commit_votes().len(),
                max: max_commit_votes,
            });
        }
        let gc_depth = self.context.protocol_config.gc_depth();
        let min_ref_round = self.context.min_ref_round(block.round());
        if let Some(reference) = block
            .acknowledgments()
            .iter()
            .chain(block.ancestors())
            .find(|reference| !committee.is_valid_index(reference.author))
        {
            return Err(ConsensusError::InvalidReferenceAuthority {
                index: reference.author,
                max: committee.size(),
            });
        }
        for acknowledgment in block.acknowledgments() {
            if acknowledgment.round >= block.round() {
                return Err(ConsensusError::InvalidAcknowledgmentRound {
                    acknowledgment: acknowledgment.round,
                    block: block.round(),
                });
            }
            if acknowledgment.round < min_ref_round {
                return Err(ConsensusError::AcknowledgmentRoundTooOld {
                    acknowledgment: acknowledgment.round,
                    block: block.round(),
                    gc_depth,
                });
            }
        }

        let mut seen_ancestors = vec![false; committee.size()];
        let mut parent_stakes = 0;
        for (i, ancestor) in block.ancestors().iter().enumerate() {
            if (i == 0 && ancestor.author != block.author())
                || (i > 0 && ancestor.author == block.author())
            {
                return Err(ConsensusError::InvalidAncestorPosition {
                    block_authority: block.author(),
                    ancestor_authority: ancestor.author,
                    position: i,
                });
            }
            if ancestor.round >= block.round() {
                return Err(ConsensusError::InvalidAncestorRound {
                    ancestor: ancestor.round,
                    block: block.round(),
                });
            }
            // Skip the gc_depth lower bound for the author's own ancestor
            // (i == 0): the proposer always includes its last proposed block
            // regardless of gap so a recovered node can catch up.
            if i > 0 && ancestor.round < min_ref_round {
                return Err(ConsensusError::AncestorRoundTooOld {
                    ancestor: ancestor.round,
                    block: block.round(),
                    gc_depth,
                });
            }
            if ancestor.round == GENESIS_ROUND && !self.genesis.contains(ancestor) {
                return Err(ConsensusError::InvalidGenesisAncestor(*ancestor));
            }
            if seen_ancestors[ancestor.author] {
                return Err(ConsensusError::DuplicatedAncestorsAuthority(
                    ancestor.author,
                ));
            }
            seen_ancestors[ancestor.author] = true;
            // Block must have round >= 1 so checked_sub(1) should be safe.
            if ancestor.round == block.round().checked_sub(1).unwrap() {
                parent_stakes += committee.stake(ancestor.author);
            }
        }
        if !committee.reached_quorum(parent_stakes) {
            return Err(ConsensusError::InsufficientParentStakes {
                parent_stakes,
                quorum: committee.quorum_threshold(),
            });
        }

        // Validate strong_vote field.
        if let Some(strong_vote) = block.strong_vote() {
            if !committee.is_valid_index(strong_vote.leader_authority) {
                return Err(ConsensusError::InvalidStrongVoteAuthority {
                    index: strong_vote.leader_authority,
                    max: committee.size(),
                });
            }
            for authority_index in strong_vote.missing.iter() {
                if !committee.is_valid_index(authority_index) {
                    return Err(ConsensusError::InvalidStrongVoteAuthority {
                        index: authority_index,
                        max: committee.size(),
                    });
                }
            }
            // Self-consistency: the pinned leader must appear in the block's
            // ancestors at round-1.
            let leader_round = block.round().saturating_sub(1);
            if leader_round != GENESIS_ROUND {
                let leader_seen = block
                    .ancestors()
                    .iter()
                    .any(|r| r.round == leader_round && r.author == strong_vote.leader_authority);
                if !leader_seen {
                    return Err(ConsensusError::StrongVoteLeaderNotInAncestors {
                        block_round: block.round(),
                        leader_round,
                        leader_authority: strong_vote.leader_authority,
                    });
                }
            }
        }
        Ok(())
    }

    fn check_and_parse_transactions(
        &self,
        serialized_transactions: &[u8],
    ) -> ConsensusResult<Vec<Transaction>> {
        let limit = serialized_transactions_size_limit(&self.context);
        if serialized_transactions.len() > limit {
            return Err(ConsensusError::SerializedTransactionsTooLarge {
                size: serialized_transactions.len(),
                limit,
            });
        }
        bcs::from_bytes(serialized_transactions).map_err(ConsensusError::MalformedTransactions)
    }

    fn verify_transactions_validity(
        &self,
        transactions: &CommitmentVerifiedTransactions,
    ) -> ConsensusResult<()> {
        let batch: Vec<_> = transactions
            .transactions()
            .iter()
            .map(|t| t.data())
            .collect();
        self.check_transactions(&batch)?;
        self.transaction_verifier
            .verify_batch(&batch)
            .map_err(|e| ConsensusError::InvalidTransaction(format!("{e:?}")))
    }
}

// BCS encodes sequence lengths as ULEB128 values. Ten bytes covers any usize
// length on supported 64-bit targets.
pub(crate) const MAX_BCS_LENGTH_PREFIX_BYTES: usize = 10;

/// Upper bound on the BCS-serialized size of any transaction batch that
/// passes `check_transactions`: the maximum payload bytes plus a length
/// prefix per transaction and one for the enclosing sequence.
pub(crate) fn serialized_transactions_size_limit(context: &Context) -> usize {
    let max_bytes = context.protocol_config.max_transactions_in_block_bytes() as usize;
    let max_count = context.protocol_config.max_num_transactions_in_block() as usize;

    max_bytes
        .saturating_add(max_count.saturating_mul(MAX_BCS_LENGTH_PREFIX_BYTES))
        .saturating_add(MAX_BCS_LENGTH_PREFIX_BYTES)
}

/// Upper bound on the length of a shard encoding a transaction payload that
/// passes `check_transactions`. `usize::MAX` when
/// [`serialized_transactions_size_limit()`] returns `usize::MAX`.
pub(crate) fn max_shard_bytes(context: &Context) -> usize {
    let payload_limit = serialized_transactions_size_limit(context);
    // `shard_bytes()` would overflow on this payload length.
    if payload_limit == usize::MAX {
        return usize::MAX;
    }
    shard_bytes(payload_limit, context.committee.info_length())
}

#[cfg(test)]
pub(crate) struct NoopBlockVerifier;

#[cfg(test)]
impl BlockVerifier for NoopBlockVerifier {
    fn verify(&self, _block: &SignedBlockHeader) -> ConsensusResult<()> {
        Ok(())
    }

    fn check_and_parse_transactions(
        &self,
        serialized_transactions: &[u8],
    ) -> ConsensusResult<Vec<Transaction>> {
        bcs::from_bytes(serialized_transactions).map_err(ConsensusError::MalformedTransactions)
    }

    fn verify_transactions_validity(
        &self,
        _transactions: &CommitmentVerifiedTransactions,
    ) -> ConsensusResult<()> {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test {
    use iota_protocol_config::ProtocolConfig;
    use starfish_config::AuthorityIndex;

    use super::*;
    use crate::{
        authority_set::AuthoritySet,
        block_header::{
            BlockHeaderDigest, BlockRef, StrongVote, TestBlockHeader, TestBlockHeaderVersion,
        },
        context::Context,
        transaction::{TransactionVerifier, ValidationError},
    };

    /// Pins the wire layout `serialized_transactions_size_limit` is derived
    /// from: a maximal batch accepted by `check_transactions` must serialize
    /// to exactly one ULEB128 length prefix per sequence plus the raw bytes,
    /// and fit within the limit. Fails if the BCS framing or the
    /// `Transaction` layout changes.
    #[tokio::test]
    async fn serialized_transactions_size_limit_bounds_maximal_valid_batch() {
        fn uleb128_len(mut value: usize) -> usize {
            let mut len = 1;
            while value >= 0x80 {
                value >>= 7;
                len += 1;
            }
            len
        }

        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let max_bytes = context.protocol_config.max_transactions_in_block_bytes() as usize;
        let max_count = context.protocol_config.max_num_transactions_in_block() as usize;
        let tx_size = max_bytes / max_count;
        let transactions: Vec<Transaction> = (0..max_count)
            .map(|_| Transaction::new(vec![0; tx_size]))
            .collect();

        let verifier = SignedBlockVerifier::new(context.clone(), Arc::new(TxnSizeVerifier {}));
        let batch: Vec<_> = transactions.iter().map(|t| t.data()).collect();
        verifier.check_transactions(&batch).unwrap();

        let serialized = bcs::to_bytes(&transactions).unwrap();
        let expected = uleb128_len(max_count) + max_count * (uleb128_len(tx_size) + tx_size);
        assert_eq!(serialized.len(), expected);
        assert!(serialized.len() <= serialized_transactions_size_limit(&context));
    }

    /// Pins `max_shard_bytes` to what the encoder actually produces for a
    /// maximal payload, so a change to the encoder's chunking is caught here
    /// rather than by shards being refused at ingress.
    #[tokio::test]
    async fn max_shard_bytes_bounds_maximal_shard() {
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let payload_limit = serialized_transactions_size_limit(&context);
        let info_length = context.committee.info_length();
        let parity_length = context.committee.parity_length();

        let mut encoder = crate::encoder::create_encoder(&context);
        let shards = encoder
            .encode_serialized_data(
                &bytes::Bytes::from(vec![0u8; payload_limit]),
                info_length,
                parity_length,
            )
            .unwrap();

        let longest = shards.iter().map(|shard| shard.len()).max().unwrap();
        assert_eq!(longest, max_shard_bytes(&context));
    }

    #[tokio::test]
    async fn check_and_parse_transactions_rejects_oversized_payload() {
        let (context, _) = Context::new_for_test(4);
        let context = Arc::new(context);
        let size_limit = serialized_transactions_size_limit(&context);
        let verifier = SignedBlockVerifier::new(context, Arc::new(TxnSizeVerifier {}));

        let result = verifier.check_and_parse_transactions(&vec![0u8; size_limit + 1]);
        assert!(matches!(
            result,
            Err(ConsensusError::SerializedTransactionsTooLarge { size, limit })
                if size == size_limit + 1 && limit == size_limit
        ));
    }

    /// Returns a verifier whose protocol config limits the transactions of a
    /// block to 10 bytes each, 4 in number and 25 bytes in total.
    fn build_verifier_with_small_transaction_limits() -> SignedBlockVerifier {
        build_verifier_with_transaction_limits(10, 4, 25)
    }

    /// Returns a verifier whose protocol config limits the transactions of a
    /// block to `max_size` bytes each, `max_count` in number and `max_total`
    /// bytes in total.
    fn build_verifier_with_transaction_limits(
        max_size: u64,
        max_count: u64,
        max_total: u64,
    ) -> SignedBlockVerifier {
        let _guard = ProtocolConfig::apply_overrides_for_testing(move |_, mut config| {
            config.set_consensus_max_transaction_size_bytes_for_testing(max_size);
            config.set_consensus_max_num_transactions_in_block_for_testing(max_count);
            config.set_consensus_max_transactions_in_block_bytes_for_testing(max_total);
            config
        });
        let (context, _) = Context::new_for_test(4);

        SignedBlockVerifier::new(Arc::new(context), Arc::new(TxnSizeVerifier {}))
    }

    /// Returns what [`SignedBlockVerifier::check_transactions()`] of
    /// `verifier` answers for transactions of `sizes` bytes.
    fn check_transactions_of_sizes(
        verifier: &SignedBlockVerifier,
        sizes: &[usize],
    ) -> ConsensusResult<()> {
        let transactions: Vec<Vec<u8>> = sizes.iter().map(|&size| vec![0; size]).collect();
        let batch: Vec<&[u8]> = transactions.iter().map(Vec::as_slice).collect();

        verifier.check_transactions(&batch)
    }

    /// A block exactly at the three limits is accepted. The error for a limit
    /// carries the value that is over it and the limit.
    #[tokio::test]
    async fn block_over_one_transaction_limit_is_refused_with_error_of_that_limit() {
        let verifier = build_verifier_with_small_transaction_limits();

        check_transactions_of_sizes(&verifier, &[10, 10, 5, 0])
            .expect("a block at every limit should be accepted");
        assert!(matches!(
            check_transactions_of_sizes(&verifier, &[11]),
            Err(ConsensusError::TransactionTooLarge {
                size: 11,
                limit: 10
            })
        ));
        assert!(matches!(
            check_transactions_of_sizes(&verifier, &[0; 5]),
            Err(ConsensusError::TooManyTransactions { count: 5, limit: 4 })
        ));
        assert!(matches!(
            check_transactions_of_sizes(&verifier, &[10, 10, 6]),
            Err(ConsensusError::TooManyTransactionBytes {
                size: 26,
                limit: 25
            })
        ));
    }

    /// The order is that of [`check_transactions_fit()`]: the number of
    /// transactions comes before the size of a transaction, and the total up
    /// to a transaction before the size of a later one.
    #[tokio::test]
    async fn block_over_two_transaction_limits_is_refused_for_limit_checked_first() {
        let verifier = build_verifier_with_small_transaction_limits();

        // Five transactions, the first of 11 bytes.
        assert!(matches!(
            check_transactions_of_sizes(&verifier, &[11, 0, 0, 0, 0]),
            Err(ConsensusError::TooManyTransactions { count: 5, limit: 4 })
        ));
        // The third transaction takes the total to 30 bytes, and the fourth
        // is of 11 bytes.
        assert!(matches!(
            check_transactions_of_sizes(&verifier, &[10, 10, 10, 11]),
            Err(ConsensusError::TooManyTransactionBytes {
                size: 30,
                limit: 25
            })
        ));
    }

    /// A limit of 0 means that nothing fits, not that there is no limit. With
    /// 0 as the number of transactions, only an empty block is accepted, and
    /// with 0 as a size, only empty transactions are.
    #[tokio::test]
    async fn block_over_limit_of_zero_is_refused() {
        let verifier = build_verifier_with_transaction_limits(10, 0, 25);
        check_transactions_of_sizes(&verifier, &[]).expect("an empty block should be accepted");
        assert!(matches!(
            check_transactions_of_sizes(&verifier, &[0]),
            Err(ConsensusError::TooManyTransactions { count: 1, limit: 0 })
        ));

        let verifier = build_verifier_with_transaction_limits(0, 4, 25);
        check_transactions_of_sizes(&verifier, &[0, 0])
            .expect("empty transactions should be accepted");
        assert!(matches!(
            check_transactions_of_sizes(&verifier, &[1]),
            Err(ConsensusError::TransactionTooLarge { size: 1, limit: 0 })
        ));

        let verifier = build_verifier_with_transaction_limits(10, 4, 0);
        check_transactions_of_sizes(&verifier, &[0, 0])
            .expect("empty transactions should be accepted");
        assert!(matches!(
            check_transactions_of_sizes(&verifier, &[1]),
            Err(ConsensusError::TooManyTransactionBytes { size: 1, limit: 0 })
        ));
    }

    /// With 0 as the number of transactions, the limit on a block's
    /// serialized transactions is still the computed bound, so a payload over
    /// it is refused before it is decoded.
    #[tokio::test]
    async fn oversized_payload_is_refused_with_limit_of_zero() {
        let verifier = build_verifier_with_transaction_limits(10, 0, 25);
        let size_limit = serialized_transactions_size_limit(&verifier.context);
        assert_eq!(size_limit, 25 + MAX_BCS_LENGTH_PREFIX_BYTES);

        let result = verifier.check_and_parse_transactions(&vec![0u8; size_limit + 1]);
        assert!(matches!(
            result,
            Err(ConsensusError::SerializedTransactionsTooLarge { size, limit })
                if size == size_limit + 1 && limit == size_limit
        ));
    }

    pub(crate) struct TxnSizeVerifier {}

    impl TransactionVerifier for TxnSizeVerifier {
        // Fails verification if any transaction is < 4 bytes.
        fn verify_batch(&self, transactions: &[&[u8]]) -> Result<(), ValidationError> {
            for txn in transactions {
                if txn.len() < 4 {
                    return Err(ValidationError::InvalidTransaction(format!(
                        "Length {} is too short!",
                        txn.len()
                    )));
                }
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_verify_block() {
        let (context, keypairs) = Context::new_for_test(4);
        let context = Arc::new(context);
        let version = TestBlockHeaderVersion::V2;
        let authority_2_protocol_keypair = &keypairs[2].1;
        let verifier = SignedBlockVerifier::new(context, Arc::new(TxnSizeVerifier {}));

        let test_block = TestBlockHeader::new(10, 2)
            .set_version(version)
            .set_ancestors(vec![
                BlockRef::new(9, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                BlockRef::new(7, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
            ]);

        // Valid SignedBlock.
        {
            let block = test_block.clone().build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            verifier.verify(&signed_block).unwrap();
        }

        // Block with wrong epoch.
        {
            let block = test_block.clone().set_epoch(1).build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::WrongEpoch {
                    expected: _,
                    actual: _
                })
            ));
        }

        // Block at genesis round.
        {
            let block = test_block.clone().set_round(0).build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::UnexpectedGenesisHeader)
            ));
        }

        // Block with invalid authority index.
        {
            let block = test_block
                .clone()
                .set_author(AuthorityIndex::new_for_test(4))
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::InvalidAuthorityIndex { index: _, max: _ })
            ));
        }

        // Block with mismatched authority index and signature.
        {
            let block = test_block
                .clone()
                .set_author(AuthorityIndex::new_for_test(1))
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::SignatureVerificationFailure(_))
            ));
        }

        // Block with wrong key.
        {
            let block = test_block.clone().build();
            let signed_block = SignedBlockHeader::new(block, &keypairs[3].1).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::SignatureVerificationFailure(_))
            ));
        }

        // Block without signature.
        {
            let block = test_block.clone().build();
            let mut signed_block =
                SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            signed_block.clear_signature();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::MalformedSignature(_))
            ));
        }

        // Block with invalid ancestor round.
        {
            let block = test_block
                .clone()
                .set_ancestors(vec![
                    BlockRef::new(9, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                    BlockRef::new(10, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
                ])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::InvalidAncestorRound {
                    ancestor: _,
                    block: _
                })
            ));
        }

        // Block with parents not reaching quorum.
        {
            let block = test_block
                .clone()
                .set_ancestors(vec![
                    BlockRef::new(9, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
                ])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::InsufficientParentStakes {
                    parent_stakes: _,
                    quorum: _
                })
            ));
        }

        // Block with too many ancestors.
        {
            let block = test_block
                .clone()
                .set_ancestors(vec![
                    BlockRef::new(9, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
                ])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::TooManyAncestors(_, _))
            ));
        }

        // Block with too many acknowledgments.
        {
            let committee_size = 4u8;
            let max_acknowledgments = 2 * committee_size;
            // Build acknowledgments at rounds (1..) that don't overlap with the
            // ancestors above (rounds 7 and 9).
            let acknowledgments = (0..=max_acknowledgments)
                .map(|i| {
                    BlockRef::new(
                        (i / committee_size + 1) as u32,
                        AuthorityIndex::new_for_test(i % committee_size),
                        BlockHeaderDigest::MIN,
                    )
                })
                .collect::<Vec<_>>();
            let block = test_block
                .clone()
                .set_acknowledgments(acknowledgments)
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::TooManyAcknowledgments { .. })
            ));
        }

        // Block with too many commit votes.
        {
            let committee_size = 4u32;
            let commit_votes = (0..=committee_size)
                .map(|i| crate::commit::CommitVote::new(i, crate::commit::CommitDigest::MIN))
                .collect::<Vec<_>>();
            let block = test_block.clone().set_commit_votes(commit_votes).build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::TooManyCommitVotes { .. })
            ));
        }

        // Block without own ancestor.
        {
            let block = test_block
                .clone()
                .set_ancestors(vec![
                    BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
                ])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::InvalidAncestorPosition {
                    block_authority: _,
                    ancestor_authority: _,
                    position: _
                })
            ));
        }

        // Block with own ancestor at wrong position.
        {
            let block = test_block
                .clone()
                .set_ancestors(vec![
                    BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
                ])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::InvalidAncestorPosition {
                    block_authority: _,
                    ancestor_authority: _,
                    position: _
                })
            ));
        }

        // Block with ancestors from the same authority.
        {
            let block = test_block
                .set_ancestors(vec![
                    BlockRef::new(8, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                    BlockRef::new(8, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                ])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::DuplicatedAncestorsAuthority(_))
            ));
        }
    }

    #[tokio::test]
    async fn test_verify_block_round_gap() {
        let (mut context, keypairs) = Context::new_for_test(4);
        // Small gc_depth so we can construct violations without huge round
        // numbers.
        context
            .protocol_config
            .set_consensus_gc_depth_for_testing(5);
        let context = Arc::new(context);
        let version = TestBlockHeaderVersion::V2;
        let authority_2_protocol_keypair = &keypairs[2].1;
        let verifier = SignedBlockVerifier::new(context, Arc::new(TxnSizeVerifier {}));

        let test_block = TestBlockHeader::new(10, 2)
            .set_version(version)
            .set_ancestors(vec![
                BlockRef::new(9, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                BlockRef::new(7, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
            ]);

        // Acknowledgment at the block's round.
        {
            let block = test_block
                .clone()
                .set_acknowledgments(vec![BlockRef::new(
                    10,
                    AuthorityIndex::new_for_test(0),
                    BlockHeaderDigest::MIN,
                )])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::InvalidAcknowledgmentRound { .. })
            ));
        }

        // Acknowledgment older than gc_depth (block 10 - gc_depth 5 = 5).
        {
            let block = test_block
                .clone()
                .set_acknowledgments(vec![BlockRef::new(
                    4,
                    AuthorityIndex::new_for_test(0),
                    BlockHeaderDigest::MIN,
                )])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::AcknowledgmentRoundTooOld { .. })
            ));
        }

        // Acknowledgment and ancestor from an authority outside the committee.
        for block in [
            test_block.clone().set_acknowledgments(vec![BlockRef::new(
                9,
                AuthorityIndex::new_for_test(4),
                BlockHeaderDigest::MIN,
            )]),
            test_block.clone().set_ancestors(vec![
                BlockRef::new(9, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(4), BlockHeaderDigest::MIN),
            ]),
        ] {
            let signed_block =
                SignedBlockHeader::new(block.build(), authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::InvalidReferenceAuthority { .. })
            ));
        }

        // Non-own ancestor older than gc_depth.
        {
            let block = test_block
                .clone()
                .set_ancestors(vec![
                    BlockRef::new(9, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                    BlockRef::new(2, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
                ])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::AncestorRoundTooOld { .. })
            ));
        }

        // Author's own ancestor older than gc_depth is allowed (catch-up).
        {
            let block = test_block
                .set_ancestors(vec![
                    BlockRef::new(2, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                    BlockRef::new(9, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
                ])
                .build();
            let signed_block = SignedBlockHeader::new(block, authority_2_protocol_keypair).unwrap();
            verifier.verify(&signed_block).unwrap();
        }
    }

    #[tokio::test]
    async fn test_verify_block_rejects_v1_header() {
        let (context, keypairs) = Context::new_for_test(4);
        let verifier = SignedBlockVerifier::new(Arc::new(context), Arc::new(TxnSizeVerifier {}));

        let block = TestBlockHeader::new(10, 2)
            .set_ancestors(vec![
                BlockRef::new(9, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(0), BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                BlockRef::new(7, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
            ])
            .build();
        let signed_block = SignedBlockHeader::new(block, &keypairs[2].1).unwrap();
        assert!(matches!(
            verifier.verify(&signed_block),
            Err(ConsensusError::WrongBlockHeaderVersion { actual: "V1" })
        ));
    }

    #[tokio::test]
    async fn test_verify_block_strong_vote() {
        let (context, keypairs) = Context::new_for_test(4);

        let keypair = &keypairs[2].1;
        let leader_authority = AuthorityIndex::new_for_test(0);
        let verifier = SignedBlockVerifier::new(Arc::new(context), Arc::new(TxnSizeVerifier {}));

        let base = TestBlockHeader::new(10, 2)
            .set_version(TestBlockHeaderVersion::V2)
            .set_ancestors(vec![
                BlockRef::new(9, AuthorityIndex::new_for_test(2), BlockHeaderDigest::MIN),
                BlockRef::new(9, leader_authority, BlockHeaderDigest::MIN),
                BlockRef::new(9, AuthorityIndex::new_for_test(1), BlockHeaderDigest::MIN),
                BlockRef::new(7, AuthorityIndex::new_for_test(3), BlockHeaderDigest::MIN),
            ]);
        let well_formed = StrongVote {
            leader_authority,
            missing: AuthoritySet::new(),
        };

        // Flag on + missing contains an index >= committee.size() ->
        // InvalidStrongVoteAuthority.
        {
            let mut missing = AuthoritySet::new();
            missing.insert(AuthorityIndex::new_for_test(99));
            let block = base
                .clone()
                .set_strong_vote(Some(StrongVote {
                    leader_authority,
                    missing,
                }))
                .build();
            let signed_block = SignedBlockHeader::new(block, keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::InvalidStrongVoteAuthority { .. })
            ));
        }

        // Flag on + leader_authority >= committee.size() ->
        // InvalidStrongVoteAuthority. Trips the leader_authority bound check
        // before the leader-in-ancestors check below.
        {
            let block = base
                .clone()
                .set_strong_vote(Some(StrongVote {
                    leader_authority: AuthorityIndex::new_for_test(99),
                    missing: AuthoritySet::new(),
                }))
                .build();
            let signed_block = SignedBlockHeader::new(block, keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::InvalidStrongVoteAuthority { .. })
            ));
        }

        // Flag on + valid leader_authority but pinned leader not in ancestors
        // at round-1 -> StrongVoteLeaderNotInAncestors. Authority 3 is in the
        // base block's ancestors only at round 7, not at round 9
        // (= leader_round).
        {
            let block = base
                .clone()
                .set_strong_vote(Some(StrongVote {
                    leader_authority: AuthorityIndex::new_for_test(3),
                    missing: AuthoritySet::new(),
                }))
                .build();
            let signed_block = SignedBlockHeader::new(block, keypair).unwrap();
            assert!(matches!(
                verifier.verify(&signed_block),
                Err(ConsensusError::StrongVoteLeaderNotInAncestors { .. })
            ));
        }

        // Flag on + well-formed strong_vote (empty missing, leader at R-1 in
        // ancestors) -> Ok.
        {
            let block = base.set_strong_vote(Some(well_formed)).build();
            let signed_block = SignedBlockHeader::new(block, keypair).unwrap();
            verifier.verify(&signed_block).unwrap();
        }
    }
}
