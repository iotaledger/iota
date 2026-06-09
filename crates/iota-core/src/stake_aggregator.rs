// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashMap, hash_map::Entry},
    hash::Hash,
    sync::Arc,
};

use iota_sdk_types::crypto::Intent;
use iota_types::{
    base_types::{AuthorityName, ConciseableName},
    committee::{Committee, CommitteeTrait, StakeUnit},
    crypto::{AuthorityQuorumSignInfo, AuthoritySignInfo, AuthoritySignInfoTrait},
    error::{IotaError, IotaResult},
    message_envelope::{Envelope, Message},
};
use serde::Serialize;
use tracing::warn;
use typed_store::TypedStoreError;

/// StakeAggregator allows us to keep track of the total stake of a set of
/// validators. STRENGTH indicates whether we want a strong quorum (2f+1) or a
/// weak quorum (f+1).
#[derive(Debug)]
pub struct StakeAggregator<S, const STRENGTH: bool> {
    data: HashMap<AuthorityName, S>,
    total_votes: StakeUnit,
    committee: Arc<Committee>,
}

/// StakeAggregator is a utility data structure that allows us to aggregate a
/// list of validator signatures over time. A committee is used to determine
/// whether we have reached sufficient quorum (defined based on `STRENGTH`). The
/// generic implementation does not require `S` to be an actual signature, but
/// just an indication that a specific validator has voted. A specialized
/// implementation for `AuthoritySignInfo` is followed below.
impl<S: Clone + Eq, const STRENGTH: bool> StakeAggregator<S, STRENGTH> {
    pub fn new(committee: Arc<Committee>) -> Self {
        Self {
            data: Default::default(),
            total_votes: Default::default(),
            committee,
        }
    }

    pub fn from_iter<I: Iterator<Item = Result<(AuthorityName, S), TypedStoreError>>>(
        committee: Arc<Committee>,
        data: I,
    ) -> IotaResult<Self> {
        let mut this = Self::new(committee);
        for item in data {
            let (authority, s) = item?;
            this.insert_generic(authority, s);
        }
        Ok(this)
    }

    /// A generic version of inserting arbitrary type of V (e.g. void type).
    /// If V is AuthoritySignInfo, the `insert` function should be used instead
    /// since it does extra checks and aggregations in the end.
    /// Returns Map authority -> S, without aggregating it.
    /// If you want to get an aggregated signature instead, use
    /// `StakeAggregator::insert`
    pub fn insert_generic(
        &mut self,
        authority: AuthorityName,
        s: S,
    ) -> InsertResult<&HashMap<AuthorityName, S>> {
        match self.data.entry(authority) {
            Entry::Occupied(oc) => {
                return InsertResult::Failed {
                    error: IotaError::StakeAggregatorRepeatedSigner {
                        signer: authority,
                        conflicting_sig: oc.get() != &s,
                    },
                };
            }
            Entry::Vacant(va) => {
                va.insert(s);
            }
        }
        let votes = self.committee.weight(&authority);
        if votes > 0 {
            self.total_votes += votes;
            if self.total_votes >= self.committee.threshold::<STRENGTH>() {
                InsertResult::QuorumReached(&self.data)
            } else {
                InsertResult::NotEnoughVotes {
                    bad_votes: 0,
                    bad_authorities: vec![],
                }
            }
        } else {
            InsertResult::Failed {
                error: IotaError::InvalidAuthenticator,
            }
        }
    }

    pub fn contains_key(&self, authority: &AuthorityName) -> bool {
        self.data.contains_key(authority)
    }

    pub fn keys(&self) -> impl Iterator<Item = &AuthorityName> {
        self.data.keys()
    }

    pub fn committee(&self) -> &Committee {
        &self.committee
    }

    pub fn total_votes(&self) -> StakeUnit {
        self.total_votes
    }

    pub fn validator_sig_count(&self) -> usize {
        self.data.len()
    }
}

impl<const STRENGTH: bool> StakeAggregator<AuthoritySignInfo, STRENGTH> {
    /// Insert an authority signature. This is the primary way to use the
    /// aggregator and a few dedicated checks are performed to make sure
    /// things work. If quorum is reached, we return AuthorityQuorumSignInfo
    /// directly.
    pub fn insert<T: Message + Serialize>(
        &mut self,
        envelope: Envelope<T, AuthoritySignInfo>,
    ) -> InsertResult<AuthorityQuorumSignInfo<STRENGTH>> {
        let (data, sig) = envelope.into_data_and_sig();
        if self.committee.epoch != sig.epoch {
            return InsertResult::Failed {
                error: IotaError::WrongEpoch {
                    expected_epoch: self.committee.epoch,
                    actual_epoch: sig.epoch,
                },
            };
        }
        match self.insert_generic(sig.authority, sig) {
            InsertResult::QuorumReached(_) => {
                match AuthorityQuorumSignInfo::<STRENGTH>::new_from_auth_sign_infos(
                    self.data.values().cloned().collect(),
                    self.committee(),
                ) {
                    Ok(aggregated) => {
                        match aggregated.verify_secure(
                            &data,
                            Intent::iota_app(T::SCOPE),
                            self.committee(),
                        ) {
                            // In the happy path, the aggregated signature verifies ok and no need
                            // to verify individual.
                            Ok(_) => InsertResult::QuorumReached(aggregated),
                            Err(_) => {
                                // If the aggregated signature fails to verify, fallback to
                                // iterating through all signatures
                                // and verify individually. Decrement total votes and continue
                                // to find new authority for signature to reach the quorum.
                                //
                                // TODO(joyqvq): It is possible for the aggregated signature to fail
                                // every time when the latest one
                                // single signature fails to verify repeatedly, and trigger
                                // this for loop to run. This can be optimized by caching single sig
                                // verification result only verify
                                // the net new ones.
                                let mut bad_votes = 0;
                                let mut bad_authorities = vec![];
                                for (name, sig) in &self.data.clone() {
                                    if let Err(err) = sig.verify_secure(
                                        &data,
                                        Intent::iota_app(T::SCOPE),
                                        self.committee(),
                                    ) {
                                        // TODO(joyqvq): Currently, the aggregator cannot do much
                                        // with an authority that
                                        // always returns an invalid signature other than saving to
                                        // errors in state. It
                                        // is possible to add the authority to a denylist or  punish
                                        // the byzantine authority.
                                        warn!(name=?name.concise(), "Bad stake from validator: {:?}", err);
                                        self.data.remove(name);
                                        let votes = self.committee.weight(name);
                                        self.total_votes -= votes;
                                        bad_votes += votes;
                                        bad_authorities.push(*name);
                                    }
                                }
                                // After evicting invalid sigs, the remaining valid sigs may
                                // still constitute a quorum on their own.
                                if self.total_votes >= self.committee.threshold::<STRENGTH>() {
                                    match AuthorityQuorumSignInfo::<STRENGTH>::new_from_auth_sign_infos(
                                        self.data.values().cloned().collect(),
                                        self.committee(),
                                    ) {
                                        Ok(aggregated) => InsertResult::QuorumReached(aggregated),
                                        Err(error) => InsertResult::Failed { error },
                                    }
                                } else {
                                    InsertResult::NotEnoughVotes {
                                        bad_votes,
                                        bad_authorities,
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => InsertResult::Failed { error },
                }
            }
            // The following is necessary to change the template type of InsertResult.
            InsertResult::Failed { error } => InsertResult::Failed { error },
            InsertResult::NotEnoughVotes {
                bad_votes,
                bad_authorities,
            } => InsertResult::NotEnoughVotes {
                bad_votes,
                bad_authorities,
            },
        }
    }
}

pub enum InsertResult<CertT> {
    QuorumReached(CertT),
    Failed {
        error: IotaError,
    },
    NotEnoughVotes {
        bad_votes: u64,
        bad_authorities: Vec<AuthorityName>,
    },
}

impl<CertT> InsertResult<CertT> {
    pub fn is_quorum_reached(&self) -> bool {
        matches!(self, Self::QuorumReached(..))
    }
}

/// MultiStakeAggregator is a utility data structure that tracks the stake
/// accumulation of potentially multiple different values (usually due to
/// byzantine/corrupted responses). Each value is tracked using a
/// StakeAggregator and determine whether it has reached a quorum. Once quorum
/// is reached, the aggregated signature is returned.
#[derive(Debug)]
pub struct MultiStakeAggregator<K, V, const STRENGTH: bool> {
    committee: Arc<Committee>,
    stake_maps: HashMap<K, (V, StakeAggregator<AuthoritySignInfo, STRENGTH>)>,
}

impl<K, V, const STRENGTH: bool> MultiStakeAggregator<K, V, STRENGTH> {
    pub fn new(committee: Arc<Committee>) -> Self {
        Self {
            committee,
            stake_maps: Default::default(),
        }
    }

    pub fn unique_key_count(&self) -> usize {
        self.stake_maps.len()
    }

    pub fn total_votes(&self) -> StakeUnit {
        self.stake_maps
            .values()
            .map(|(_, stake_aggregator)| stake_aggregator.total_votes())
            .sum()
    }
}

impl<K, V, const STRENGTH: bool> MultiStakeAggregator<K, V, STRENGTH>
where
    K: Hash + Eq,
    V: Message + Serialize + Clone,
{
    pub fn insert(
        &mut self,
        k: K,
        envelope: Envelope<V, AuthoritySignInfo>,
    ) -> InsertResult<AuthorityQuorumSignInfo<STRENGTH>> {
        if let Some(entry) = self.stake_maps.get_mut(&k) {
            entry.1.insert(envelope)
        } else {
            let mut new_entry = StakeAggregator::new(self.committee.clone());
            let result = new_entry.insert(envelope.clone());
            if !matches!(result, InsertResult::Failed { .. }) {
                // This is very important: ensure that if the insert fails, we don't even add
                // the new entry to the map.
                self.stake_maps.insert(k, (envelope.into_data(), new_entry));
            }
            result
        }
    }
}

impl<K, V, const STRENGTH: bool> MultiStakeAggregator<K, V, STRENGTH>
where
    K: Clone + Ord,
{
    pub fn get_all_unique_values(&self) -> BTreeMap<K, (Vec<AuthorityName>, StakeUnit)> {
        self.stake_maps
            .iter()
            .map(|(k, (_, s))| (k.clone(), (s.data.keys().copied().collect(), s.total_votes)))
            .collect()
    }
}

impl<K, V, const STRENGTH: bool> MultiStakeAggregator<K, V, STRENGTH>
where
    K: Hash + Eq,
{
    #[expect(dead_code)]
    pub fn authorities_for_key(&self, k: &K) -> Option<impl Iterator<Item = &AuthorityName>> {
        self.stake_maps.get(k).map(|(_, agg)| agg.keys())
    }

    /// The sum of all remaining stake, i.e. all stake not yet
    /// committed by vote to a specific value
    pub fn uncommitted_stake(&self) -> StakeUnit {
        self.committee.total_votes() - self.total_votes()
    }

    /// Total stake of the largest faction
    pub fn plurality_stake(&self) -> StakeUnit {
        self.stake_maps
            .values()
            .map(|(_, agg)| agg.total_votes())
            .max()
            .unwrap_or_default()
    }

    /// If true, there isn't enough uncommitted stake to reach quorum for any
    /// value
    pub fn quorum_unreachable(&self) -> bool {
        self.uncommitted_stake() + self.plurality_stake() < self.committee.threshold::<STRENGTH>()
    }
}

#[cfg(test)]
mod stake_aggregator_insert_tests {
    use std::{collections::BTreeMap, sync::Arc};

    use fastcrypto::{
        hash::{HashFunction, Sha3_256},
        traits::KeyPair,
    };
    use iota_sdk_types::crypto::IntentScope;
    use iota_types::{
        base_types::AuthorityName,
        committee::Committee,
        crypto::{AuthoritySignInfo, random_committee_key_pairs_of_size},
        message_envelope::{Envelope, Message},
    };
    use serde::Serialize;

    use super::*;

    #[derive(Clone, Debug, Serialize, PartialEq, Eq, Hash)]
    struct TestMessage {
        value: String,
    }

    impl Message for TestMessage {
        type DigestType = [u8; 32];
        const SCOPE: IntentScope = IntentScope::SenderSignedTransaction;

        fn digest(&self) -> Self::DigestType {
            let mut hasher = Sha3_256::default();
            hasher.update(self.value.as_bytes());
            hasher.finalize().digest
        }
    }

    /// Regression test: `StakeAggregator::insert` must not return
    /// `NotEnoughVotes` when the remaining valid sigs (after bad-sig
    /// eviction) still form a quorum.
    #[test]
    fn test_quorum_not_lost_after_bad_sig_eviction() {
        // Two-validator committee: first sorted authority has ~7000 weight
        // (> QUORUM_THRESHOLD ~6667), second has ~3000 weight.
        let key_pairs = random_committee_key_pairs_of_size(2);
        let mut names: Vec<AuthorityName> = key_pairs
            .iter()
            .map(|kp| AuthorityName::from(kp.public()))
            .collect();
        names.sort();

        let voting_rights: BTreeMap<AuthorityName, u64> = names
            .iter()
            .enumerate()
            .map(|(i, name)| (*name, if i == 0 { 7 } else { 3 }))
            .collect();
        let committee = Arc::new(Committee::new_for_testing_with_normalized_voting_power(
            0,
            voting_rights,
        ));

        let find_kp = |name: &AuthorityName| {
            key_pairs
                .iter()
                .find(|kp| &AuthorityName::from(kp.public()) == name)
                .unwrap()
        };
        let (auth0, key0) = (names[0], find_kp(&names[0]));
        let (auth1, key1) = (names[1], find_kp(&names[1]));

        let mut agg: StakeAggregator<AuthoritySignInfo, true> = StakeAggregator::new(committee);

        let msg = TestMessage {
            value: "real".to_string(),
        };
        let msg_bad = TestMessage {
            value: "wrong".to_string(),
        };

        // auth1 signs the wrong message — weight (~3000) < threshold, no quorum yet.
        let envelope_bad = Envelope::<TestMessage, AuthoritySignInfo>::new(0, msg_bad, key1, auth1);
        assert!(matches!(
            agg.insert(envelope_bad),
            InsertResult::NotEnoughVotes { .. }
        ));

        // auth0 signs the real message — total weight crosses threshold, triggering
        // batch verify. Batch fails (auth1's sig is for the wrong message);
        // individual verify evicts auth1. auth0's weight alone (~7000) still
        // exceeds the threshold, so the result must be QuorumReached, not
        // NotEnoughVotes.
        let envelope_good = Envelope::<TestMessage, AuthoritySignInfo>::new(0, msg, key0, auth0);
        assert!(
            agg.insert(envelope_good).is_quorum_reached(),
            "valid sig with weight > quorum threshold must yield QuorumReached after bad sig eviction"
        );
    }
}
