// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeSet, marker::PhantomData};

use starfish_config::{AuthorityIndex, Committee, Stake};

pub(crate) trait CommitteeThreshold {
    fn is_threshold(committee: &Committee, amount: Stake) -> bool;
    fn threshold(committee: &Committee) -> Stake;
}

pub(crate) struct QuorumThreshold;

#[cfg(test)]
pub(crate) struct ValidityThreshold;

impl CommitteeThreshold for QuorumThreshold {
    fn is_threshold(committee: &Committee, amount: Stake) -> bool {
        committee.reached_quorum(amount)
    }
    fn threshold(committee: &Committee) -> Stake {
        committee.quorum_threshold()
    }
}

#[cfg(test)]
impl CommitteeThreshold for ValidityThreshold {
    fn is_threshold(committee: &Committee, amount: Stake) -> bool {
        committee.reached_validity(amount)
    }
    fn threshold(committee: &Committee) -> Stake {
        committee.validity_threshold()
    }
}

pub(crate) struct StakeAggregator<T> {
    votes: BTreeSet<AuthorityIndex>,
    stake: Stake,
    /// External marker that forces `reached_threshold` to return true
    /// regardless of stake. Used by callers that have an out-of-band reason
    /// to consider this aggregator settled (e.g. starfish-speed Optimistic
    /// commits that bypass the 2f+1 ack-quorum).
    committed: bool,
    _phantom: PhantomData<T>,
}

impl<T: CommitteeThreshold> StakeAggregator<T> {
    pub(crate) fn new() -> Self {
        Self {
            votes: Default::default(),
            stake: 0,
            committed: false,
            _phantom: Default::default(),
        }
    }

    /// Adds a vote for the specified authority index to the aggregator. It is
    /// guaranteed to count the vote only once for an authority. The method
    /// returns true when the required threshold has been reached.
    pub(crate) fn add(&mut self, vote: AuthorityIndex, committee: &Committee) -> bool {
        if self.votes.insert(vote) {
            self.stake += committee.stake(vote);
        }
        self.reached_threshold(committee)
    }

    pub(crate) fn stake(&self) -> Stake {
        self.stake
    }

    pub(crate) fn votes(&self) -> BTreeSet<AuthorityIndex> {
        self.votes.clone()
    }

    pub(crate) fn reached_threshold(&self, committee: &Committee) -> bool {
        self.committed || T::is_threshold(committee, self.stake)
    }

    /// Force `reached_threshold` to true without recording any votes.
    pub(crate) fn mark_committed(&mut self) {
        self.committed = true;
    }

    pub(crate) fn threshold(&self, committee: &Committee) -> Stake {
        T::threshold(committee)
    }

    pub(crate) fn clear(&mut self) {
        self.votes.clear();
        self.stake = 0;
        self.committed = false;
    }
}

#[cfg(test)]
mod tests {
    use starfish_config::{AuthorityIndex, local_committee_and_keys};

    use super::*;

    #[test]
    fn test_aggregator_quorum_threshold() {
        let committee = local_committee_and_keys(0, vec![1, 1, 1, 1]).0;
        let mut aggregator = StakeAggregator::<QuorumThreshold>::new();

        assert!(!aggregator.add(AuthorityIndex::new_for_test(0), &committee));
        assert!(!aggregator.add(AuthorityIndex::new_for_test(1), &committee));
        assert!(aggregator.add(AuthorityIndex::new_for_test(2), &committee));
        assert!(aggregator.add(AuthorityIndex::new_for_test(3), &committee));
    }

    #[test]
    fn test_aggregator_validity_threshold() {
        let committee = local_committee_and_keys(0, vec![1, 1, 1, 1]).0;
        let mut aggregator = StakeAggregator::<ValidityThreshold>::new();

        assert!(!aggregator.add(AuthorityIndex::new_for_test(0), &committee));
        assert!(aggregator.add(AuthorityIndex::new_for_test(1), &committee));
    }

    #[test]
    fn test_aggregator_clear() {
        let committee = local_committee_and_keys(0, vec![1, 1, 1, 1]).0;
        let mut aggregator = StakeAggregator::<ValidityThreshold>::new();

        assert!(!aggregator.add(AuthorityIndex::new_for_test(0), &committee));
        assert!(aggregator.add(AuthorityIndex::new_for_test(1), &committee));

        // clear the aggregator
        aggregator.clear();

        // now add them again
        assert!(!aggregator.add(AuthorityIndex::new_for_test(0), &committee));
        assert!(aggregator.add(AuthorityIndex::new_for_test(1), &committee));
    }
}
