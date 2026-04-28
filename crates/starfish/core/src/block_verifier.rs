// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, sync::Arc};

use tracing::instrument;

use crate::{
    Transaction,
    block_header::{
        BlockHeaderAPI, BlockRef, GENESIS_ROUND, SignedBlockHeader, genesis_block_headers,
    },
    context::Context,
    error::{ConsensusError, ConsensusResult},
    transaction::TransactionVerifier,
};

pub(crate) trait BlockVerifier: Send + Sync + 'static {
    /// Verifies a block's metadata and transactions.
    /// This is called before examining a block's causal history.
    fn verify(&self, block: &SignedBlockHeader) -> ConsensusResult<()>;

    fn check_and_verify_transactions(&self, transactions: &[Transaction]) -> ConsensusResult<()>;
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
    pub(crate) fn check_transactions(&self, batch: &[&[u8]]) -> ConsensusResult<()> {
        let max_transaction_size_limit =
            self.context.protocol_config.max_transaction_size_bytes() as usize;
        for t in batch {
            if t.len() > max_transaction_size_limit && max_transaction_size_limit > 0 {
                return Err(ConsensusError::TransactionTooLarge {
                    size: t.len(),
                    limit: max_transaction_size_limit,
                });
            }
        }

        let max_num_transactions_limit =
            self.context.protocol_config.max_num_transactions_in_block() as usize;
        if batch.len() > max_num_transactions_limit && max_num_transactions_limit > 0 {
            return Err(ConsensusError::TooManyTransactions {
                count: batch.len(),
                limit: max_num_transactions_limit,
            });
        }

        let total_transactions_size_limit = self
            .context
            .protocol_config
            .max_transactions_in_block_bytes() as usize;
        let total_transaction_bytes = batch.iter().map(|t| t.len()).sum::<usize>();
        if total_transaction_bytes > total_transactions_size_limit
            && total_transactions_size_limit > 0
        {
            return Err(ConsensusError::TooManyTransactionBytes {
                size: total_transaction_bytes,
                limit: total_transactions_size_limit,
            });
        }
        Ok(())
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
        let block_restrictions = self.context.protocol_config.consensus_block_restrictions();
        if block_restrictions {
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
        }
        let gc_depth = self.context.protocol_config.gc_depth();
        let min_ref_round = self.context.min_ref_round(block.round());
        for acknowledgment in block.acknowledgments() {
            ConsensusError::quick_validation_authority_indices(
                &[acknowledgment.author],
                committee,
            )?;
            if block_restrictions {
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
        }

        let check_ancestor_lower_bound =
            block_restrictions && self.context.protocol_config.consensus_fast_commit_sync();
        let mut seen_ancestors = vec![false; committee.size()];
        let mut parent_stakes = 0;
        for (i, ancestor) in block.ancestors().iter().enumerate() {
            ConsensusError::quick_validation_authority_indices(&[ancestor.author], committee)?;
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
            if check_ancestor_lower_bound && i > 0 && ancestor.round < min_ref_round {
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
        Ok(())
    }

    fn check_and_verify_transactions(&self, transactions: &[Transaction]) -> ConsensusResult<()> {
        let batch: Vec<_> = transactions.iter().map(|t| t.data()).collect();
        self.check_transactions(&batch)?;
        self.transaction_verifier
            .verify_batch(&batch)
            .map_err(|e| ConsensusError::InvalidTransaction(format!("{e:?}")))
    }
}

#[cfg(test)]
pub(crate) struct NoopBlockVerifier;

#[cfg(test)]
impl BlockVerifier for NoopBlockVerifier {
    fn verify(&self, _block: &SignedBlockHeader) -> ConsensusResult<()> {
        Ok(())
    }

    fn check_and_verify_transactions(&self, _transactions: &[Transaction]) -> ConsensusResult<()> {
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod test {
    use starfish_config::AuthorityIndex;

    use super::*;
    use crate::{
        block_header::{BlockHeaderDigest, BlockRef, TestBlockHeader},
        context::Context,
        transaction::{TransactionVerifier, ValidationError},
    };

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
        let (mut context, keypairs) = Context::new_for_test(4);
        context
            .protocol_config
            .set_consensus_block_restrictions_for_testing(true);
        let context = Arc::new(context);
        let authority_2_protocol_keypair = &keypairs[2].1;
        let verifier = SignedBlockVerifier::new(context, Arc::new(TxnSizeVerifier {}));

        let test_block = TestBlockHeader::new(10, 2).set_ancestors(vec![
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
        // numbers. consensus_fast_commit_sync must be on for the ancestor
        // lower-bound check to fire.
        context
            .protocol_config
            .set_consensus_gc_depth_for_testing(5);
        context
            .protocol_config
            .set_consensus_fast_commit_sync_for_testing(true);
        context
            .protocol_config
            .set_consensus_block_restrictions_for_testing(true);
        let context = Arc::new(context);
        let authority_2_protocol_keypair = &keypairs[2].1;
        let verifier = SignedBlockVerifier::new(context, Arc::new(TxnSizeVerifier {}));

        let test_block = TestBlockHeader::new(10, 2).set_ancestors(vec![
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
}
