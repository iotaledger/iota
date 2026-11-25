// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};

use iota_metrics::monitored_scope;
use iota_protocol_config::ConsensusTransactionOrdering;
use iota_sdk_types::TransactionDigest;
use iota_types::transaction::TransactionAPI;

use crate::consensus_handler::{
    SequencedConsensusTransactionKind, VerifiedSequencedConsensusTransaction,
};

pub struct PostConsensusTxReorder {}

impl PostConsensusTxReorder {
    pub fn reorder(
        transactions: &mut [VerifiedSequencedConsensusTransaction],
        kind: ConsensusTransactionOrdering,
    ) {
        // TODO: make the reordering algorithm richer and depend on object hotness as
        // well. Order transactions based on their gas prices. System
        // transactions without gas price are put to the beginning of the
        // sequenced_transactions vector.
        match kind {
            ConsensusTransactionOrdering::ByGasPrice => Self::order_by_gas_price(transactions),
            ConsensusTransactionOrdering::None => (),
        }
    }

    /// Orders the list so that every transaction comes after every copy of
    /// the transaction it depends on, when that one is in the list, by
    /// repeatedly placing the earliest transaction whose dependency is
    /// already placed; the list changes only as much as the dependencies
    /// require. A pure function of the list, so every validator orders a
    /// commit the same way.
    pub fn order_dependents_after_dependencies(
        transactions: &mut Vec<VerifiedSequencedConsensusTransaction>,
    ) {
        let _scope = monitored_scope("HandleConsensusOutput::order_dependents_after_dependencies");
        let dependency_of = |txn: &VerifiedSequencedConsensusTransaction| {
            txn.0.transaction.transaction_dependency()
        };
        if !transactions.iter().any(|txn| dependency_of(txn).is_some()) {
            return;
        }
        // Copies of a transaction share its dependency, so they are placed in
        // list order, and waiting for the last copy waits for all of them.
        let last_copy: HashMap<TransactionDigest, usize> = transactions
            .iter()
            .enumerate()
            .filter_map(|(position, txn)| {
                Some((txn.0.transaction.executable_transaction_digest()?, position))
            })
            .collect();
        let dependency_positions: Vec<Option<usize>> = transactions
            .iter()
            .map(|txn| dependency_of(txn).and_then(|digest| last_copy.get(&digest).copied()))
            .collect();
        if dependency_positions
            .iter()
            .enumerate()
            .all(|(position, dependency)| dependency.is_none_or(|dependency| dependency < position))
        {
            return;
        }

        let mut placed = vec![false; transactions.len()];
        let mut waiting: Vec<Vec<usize>> = vec![Vec::new(); transactions.len()];
        let mut ready = BinaryHeap::new();
        let mut order = Vec::with_capacity(transactions.len());
        for (position, dependency) in dependency_positions.into_iter().enumerate() {
            if let Some(dependency) = dependency.filter(|dependency| !placed[*dependency]) {
                waiting[dependency].push(position);
                continue;
            }
            ready.push(Reverse(position));
            while let Some(Reverse(next)) = ready.pop() {
                placed[next] = true;
                order.push(next);
                ready.extend(waiting[next].drain(..).map(Reverse));
            }
        }
        // Only a cycle of digests, which needs a hash collision, leaves a
        // transaction waiting; such transactions go last, in list order.
        order.extend((0..placed.len()).filter(|&position| !placed[position]));

        let mut slots: Vec<_> = std::mem::take(transactions).into_iter().map(Some).collect();
        *transactions = order
            .into_iter()
            .filter_map(|position| slots[position].take())
            .collect();
    }

    fn order_by_gas_price(transactions: &mut [VerifiedSequencedConsensusTransaction]) {
        let _scope = monitored_scope("HandleConsensusOutput::order_by_gas_price");
        transactions.sort_by_key(|txn| {
            // Reverse order, so transactions with higher gas price sort to the front.
            // Internal messages and system transactions carry no gas price and sort
            // to the front via `u64::MAX`.
            let gas_price = match &txn.0.transaction {
                SequencedConsensusTransactionKind::External(ext) => {
                    ext.kind.as_sender_signed_transaction()
                }
                SequencedConsensusTransactionKind::System(_) => None,
            }
            .map(|data| data.transaction().gas_price())
            .unwrap_or(u64::MAX);

            std::cmp::Reverse(gas_price)
        })
    }
}

#[cfg(test)]
mod dependency_order_tests {
    use iota_sdk_types::{ProgrammableTransaction, TransactionDigest};
    use iota_test_transaction_builder::TestTransactionBuilder;
    use iota_types::{
        base_types::random_object_ref, crypto::deterministic_random_account_private_key,
        messages_consensus::ConsensusTransaction,
    };

    use super::*;

    /// A user transaction that depends on `dependency`, if any, as sequenced
    /// by consensus.
    fn sequenced(dependency: Option<TransactionDigest>) -> VerifiedSequencedConsensusTransaction {
        let (sender, key) = deterministic_random_account_private_key();
        let mut builder = TestTransactionBuilder::new(sender, random_object_ref(), 1000)
            .programmable(ProgrammableTransaction {
                inputs: vec![],
                commands: vec![],
            });
        if let Some(dependency) = dependency {
            builder = builder.with_transaction_dependency(dependency, 0);
        }
        VerifiedSequencedConsensusTransaction::new_test(ConsensusTransaction::new_user_transaction(
            builder.build_and_sign(&key),
        ))
    }

    fn digests(transactions: &[VerifiedSequencedConsensusTransaction]) -> Vec<TransactionDigest> {
        transactions
            .iter()
            .map(|txn| txn.0.transaction.executable_transaction_digest().unwrap())
            .collect()
    }

    fn digest(transaction: &VerifiedSequencedConsensusTransaction) -> TransactionDigest {
        transaction
            .0
            .transaction
            .executable_transaction_digest()
            .unwrap()
    }

    #[test]
    fn dependent_moves_behind_its_dependency() {
        let dependency = sequenced(None);
        let dependent = sequenced(Some(digest(&dependency)));
        let other = sequenced(None);
        let expected = digests(&[dependency.clone(), dependent.clone(), other.clone()]);

        let mut transactions = vec![dependent, dependency, other];
        PostConsensusTxReorder::order_dependents_after_dependencies(&mut transactions);

        assert_eq!(digests(&transactions), expected);
    }

    #[test]
    fn chain_in_reverse_order_is_reversed() {
        let first = sequenced(None);
        let second = sequenced(Some(digest(&first)));
        let third = sequenced(Some(digest(&second)));
        let expected = digests(&[first.clone(), second.clone(), third.clone()]);

        let mut transactions = vec![third, second, first];
        PostConsensusTxReorder::order_dependents_after_dependencies(&mut transactions);

        assert_eq!(digests(&transactions), expected);
    }

    #[test]
    fn dependency_outside_the_list_keeps_the_order() {
        let dependent = sequenced(Some(TransactionDigest::random()));
        let other = sequenced(None);
        let expected = digests(&[dependent.clone(), other.clone()]);

        let mut transactions = vec![dependent, other];
        PostConsensusTxReorder::order_dependents_after_dependencies(&mut transactions);

        assert_eq!(digests(&transactions), expected);
    }

    #[test]
    fn ordered_input_is_unchanged() {
        let dependency = sequenced(None);
        let dependent = sequenced(Some(digest(&dependency)));
        let expected = digests(&[dependency.clone(), dependent.clone()]);

        let mut transactions = vec![dependency, dependent];
        PostConsensusTxReorder::order_dependents_after_dependencies(&mut transactions);

        assert_eq!(digests(&transactions), expected);
    }

    #[test]
    fn dependent_follows_the_last_copy_of_its_dependency() {
        let dependency = sequenced(None);
        let dependent = sequenced(Some(digest(&dependency)));
        let expected = digests(&[dependency.clone(), dependency.clone(), dependent.clone()]);

        let mut transactions = vec![dependent, dependency.clone(), dependency];
        PostConsensusTxReorder::order_dependents_after_dependencies(&mut transactions);

        assert_eq!(digests(&transactions), expected);
    }

    #[test]
    fn dependents_keep_their_relative_order() {
        let dependency = sequenced(None);
        let first_dependent = sequenced(Some(digest(&dependency)));
        let second_dependent = sequenced(Some(digest(&dependency)));
        let expected = digests(&[
            dependency.clone(),
            first_dependent.clone(),
            second_dependent.clone(),
        ]);

        let mut transactions = vec![first_dependent, second_dependent, dependency];
        PostConsensusTxReorder::order_dependents_after_dependencies(&mut transactions);

        assert_eq!(digests(&transactions), expected);
    }

    #[test]
    fn released_dependents_are_placed_by_position() {
        let first = sequenced(None);
        let second = sequenced(Some(digest(&first)));
        let third = sequenced(Some(digest(&second)));
        let other_dependent = sequenced(Some(digest(&first)));
        let expected = digests(&[
            first.clone(),
            second.clone(),
            other_dependent.clone(),
            third.clone(),
        ]);

        let mut transactions = vec![second, other_dependent, third, first];
        PostConsensusTxReorder::order_dependents_after_dependencies(&mut transactions);

        assert_eq!(digests(&transactions), expected);
    }
}
