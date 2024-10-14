// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    num::NonZeroUsize,
    ops::RangeInclusive,
};

use config::{
    Authority, AuthorityIdentifier, Committee, CommitteeBuilder, Epoch, Stake, WorkerCache,
    WorkerId, WorkerIndex, WorkerInfo, utils::get_available_port,
};
use crypto::{
    KeyPair, NarwhalAuthoritySignature, NetworkKeyPair, NetworkPublicKey, PublicKey, Signature,
    to_intent_message,
};
use fastcrypto::{
    hash::Hash as _,
    traits::{AllowedRng, KeyPair as _},
};
use indexmap::IndexMap;
use iota_network_stack::Multiaddr;
use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use once_cell::sync::OnceCell;
use rand::{
    Rng, RngCore, SeedableRng,
    distributions::{Bernoulli, Distribution},
    rngs::{OsRng, StdRng},
    thread_rng,
};
use types::{
    Batch, BatchDigest, Certificate, CertificateAPI, CertificateDigest, Header, HeaderAPI,
    HeaderV1Builder, Round, TimestampMs, Transaction, Vote, VoteAPI,
};

pub const VOTES_CF: &str = "votes";
pub const HEADERS_CF: &str = "headers";
pub const CERTIFICATES_CF: &str = "certificates";
pub const CERTIFICATE_DIGEST_BY_ROUND_CF: &str = "certificate_digest_by_round";
pub const CERTIFICATE_DIGEST_BY_ORIGIN_CF: &str = "certificate_digest_by_origin";
pub const PAYLOAD_CF: &str = "payload";

pub fn latest_protocol_version() -> ProtocolConfig {
    ProtocolConfig::get_for_version(ProtocolVersion::max(), Chain::Unknown)
}

pub fn get_protocol_config(version_number: u64) -> ProtocolConfig {
    ProtocolConfig::get_for_version(ProtocolVersion::new(version_number), Chain::Unknown)
}

pub fn temp_dir() -> std::path::PathBuf {
    tempfile::tempdir()
        .expect("Failed to open temporary directory")
        .into_path()
}

pub fn ensure_test_environment() {
    // One common issue when running tests on Mac is that the default ulimit is too
    // low, leading to I/O errors such as "Too many open files". Raising fdlimit
    // to bypass it. Also we can't do this in Windows, apparently.
    #[cfg(not(target_os = "windows"))]
    fdlimit::raise_fd_limit().expect("Could not raise ulimit");
}

#[macro_export]
macro_rules! test_channel {
    ($e:expr) => {
        iota_metrics::metered_channel::channel(
            $e,
            &prometheus::IntGauge::new("TEST_COUNTER", "test counter").unwrap(),
        );
    };
}

// Note: use the following macros to initialize your Primary / Consensus
// channels if your test is spawning a primary and you encounter an `AllReg`
// error.
//
// Rationale:
// The primary initialization will try to edit a specific metric in its registry
// for its new_certificates and committeed_certificates channel. The gauge
// situated in the channel you're passing as an argument to the primary
// initialization is the replacement. If that gauge is a dummy gauge, such as
// the one above, the initialization of the primary will panic (to protect the
// production code against an erroneous mistake in editing this bootstrap
// logic).
#[macro_export]
macro_rules! test_committed_certificates_channel {
    ($e:expr) => {
        iota_metrics::metered_channel::channel(
            $e,
            &prometheus::IntGauge::new(
                primary::PrimaryChannelMetrics::NAME_COMMITTED_CERTS,
                primary::PrimaryChannelMetrics::DESC_COMMITTED_CERTS,
            )
            .unwrap(),
        );
    };
}

#[macro_export]
macro_rules! test_new_certificates_channel {
    ($e:expr) => {
        iota_metrics::metered_channel::channel(
            $e,
            &prometheus::IntGauge::new(
                primary::PrimaryChannelMetrics::NAME_NEW_CERTS,
                primary::PrimaryChannelMetrics::DESC_NEW_CERTS,
            )
            .unwrap(),
        );
    };
}

////////////////////////////////////////////////////////////////
/// Keys, Committee
////////////////////////////////////////////////////////////////

pub fn random_key() -> KeyPair {
    KeyPair::generate(&mut thread_rng())
}

////////////////////////////////////////////////////////////////
/// Headers, Votes, Certificates
////////////////////////////////////////////////////////////////
pub fn fixture_payload(number_of_batches: u8) -> IndexMap<BatchDigest, (WorkerId, TimestampMs)> {
    let mut payload: IndexMap<BatchDigest, (WorkerId, TimestampMs)> = IndexMap::new();

    for _ in 0..number_of_batches {
        let batch_digest = batch().digest();

        payload.insert(batch_digest, (0, 0));
    }

    payload
}

// will create a batch with randomly formed transactions
// dictated by the parameter number_of_transactions
pub fn fixture_batch_with_transactions(number_of_transactions: u32) -> Batch {
    let transactions = (0..number_of_transactions)
        .map(|_v| transaction())
        .collect();

    Batch::new(transactions)
}

pub fn fixture_payload_with_rand<R: Rng + ?Sized>(
    number_of_batches: u8,
    rand: &mut R,
) -> IndexMap<BatchDigest, (WorkerId, TimestampMs)> {
    let mut payload: IndexMap<BatchDigest, (WorkerId, TimestampMs)> = IndexMap::new();

    for _ in 0..number_of_batches {
        let batch_digest = batch_with_rand(rand).digest();

        payload.insert(batch_digest, (0, 0));
    }

    payload
}

pub fn transaction_with_rand<R: Rng + ?Sized>(rand: &mut R) -> Transaction {
    // generate random value transactions, but the length will be always 100 bytes
    (0..100)
        .map(|_v| rand.gen_range(u8::MIN..=u8::MAX))
        .collect()
}

pub fn batch_with_rand<R: Rng + ?Sized>(rand: &mut R) -> Batch {
    Batch::new(vec![
        transaction_with_rand(rand),
        transaction_with_rand(rand),
    ])
}

// Fixture
pub fn transaction() -> Transaction {
    // generate random value transactions, but the length will be always 100 bytes
    (0..100).map(|_v| rand::random::<u8>()).collect()
}

////////////////////////////////////////////////////////////////
/// Batches
////////////////////////////////////////////////////////////////

// Fixture
pub fn batch() -> Batch {
    let transactions = vec![transaction(), transaction()];
    Batch::new(transactions)
}

/// generate multiple fixture batches. The number of generated batches
/// are dictated by the parameter num_of_batches.
pub fn batches(num_of_batches: usize) -> Vec<Batch> {
    let mut batches = Vec::new();

    for i in 1..num_of_batches + 1 {
        batches.push(batch_with_transactions(i));
    }

    batches
}

pub fn batch_with_transactions(num_of_transactions: usize) -> Batch {
    let mut transactions = Vec::new();

    for _ in 0..num_of_transactions {
        transactions.push(transaction());
    }

    Batch::new(transactions)
}

// Creates one certificate per authority starting and finishing at the specified
// rounds (inclusive). Outputs a VecDeque of certificates (the certificate with
// higher round is on the front) and a set of digests to be used as parents for
// the certificates of the next round. Note : the certificates are unsigned
pub fn make_optimal_certificates(
    committee: &Committee,
    range: RangeInclusive<Round>,
    initial_parents: &BTreeSet<CertificateDigest>,
    ids: &[AuthorityIdentifier],
) -> (VecDeque<Certificate>, BTreeSet<CertificateDigest>) {
    make_certificates(committee, range, initial_parents, ids, 0.0)
}

// Outputs rounds worth of certificates with optimal parents, signed
pub fn make_optimal_signed_certificates(
    range: RangeInclusive<Round>,
    initial_parents: &BTreeSet<CertificateDigest>,
    committee: &Committee,
    keys: &[(AuthorityIdentifier, KeyPair)],
) -> (VecDeque<Certificate>, BTreeSet<CertificateDigest>) {
    make_signed_certificates(range, initial_parents, committee, keys, 0.0)
}

// Bernoulli-samples from a set of ancestors passed as a argument,
fn this_cert_parents(
    ancestors: &BTreeSet<CertificateDigest>,
    failure_prob: f64,
) -> BTreeSet<CertificateDigest> {
    std::iter::from_fn(|| {
        let f: f64 = rand::thread_rng().gen();
        Some(f > failure_prob)
    })
    .take(ancestors.len())
    .zip(ancestors)
    .flat_map(|(parenthood, parent)| parenthood.then_some(*parent))
    .collect::<BTreeSet<_>>()
}

// Utility for making several rounds worth of certificates through iterated
// parenthood sampling. The making of individual certificates once parents are
// figured out is delegated to the `make_one_certificate` argument
fn rounds_of_certificates(
    range: RangeInclusive<Round>,
    initial_parents: &BTreeSet<CertificateDigest>,
    ids: &[AuthorityIdentifier],
    failure_probability: f64,
    make_one_certificate: impl Fn(
        AuthorityIdentifier,
        Round,
        BTreeSet<CertificateDigest>,
    ) -> (CertificateDigest, Certificate),
) -> (VecDeque<Certificate>, BTreeSet<CertificateDigest>) {
    let mut certificates = VecDeque::new();
    let mut parents = initial_parents.iter().cloned().collect::<BTreeSet<_>>();
    let mut next_parents = BTreeSet::new();

    for round in range {
        next_parents.clear();
        for id in ids {
            let this_cert_parents = this_cert_parents(&parents, failure_probability);

            let (digest, certificate) = make_one_certificate(*id, round, this_cert_parents);
            certificates.push_back(certificate);
            next_parents.insert(digest);
        }
        parents = next_parents.clone();
    }
    (certificates, next_parents)
}

// make rounds worth of unsigned certificates with the sampled number of parents
pub fn make_certificates(
    committee: &Committee,
    range: RangeInclusive<Round>,
    initial_parents: &BTreeSet<CertificateDigest>,
    ids: &[AuthorityIdentifier],
    failure_probability: f64,
) -> (VecDeque<Certificate>, BTreeSet<CertificateDigest>) {
    let generator = |pk, round, parents| mock_certificate(committee, pk, round, parents);

    rounds_of_certificates(range, initial_parents, ids, failure_probability, generator)
}

// Creates certificates for the provided rounds but also having slow nodes.
// `range`: the rounds for which we intend to create the certificates for
// `initial_parents`: the parents to use when start creating the certificates
// `keys`: the authorities for which it will create certificates for
// `slow_nodes`: the authorities which are considered slow. Being a slow
// authority means that we will  still create certificates for them on each
// round, but no other authority from higher round will refer
// to those certificates. The number (by stake) of slow_nodes can not be > f ,
// as otherwise no valid graph will be produced.
pub fn make_certificates_with_slow_nodes(
    committee: &Committee,
    range: RangeInclusive<Round>,
    initial_parents: Vec<Certificate>,
    names: &[AuthorityIdentifier],
    slow_nodes: &[(AuthorityIdentifier, f64)],
) -> (VecDeque<Certificate>, Vec<Certificate>) {
    let mut rand = StdRng::seed_from_u64(1);

    // ensure provided slow nodes do not account > f
    let slow_nodes_stake: Stake = slow_nodes
        .iter()
        .map(|(key, _)| committee.authority(key).unwrap().stake())
        .sum();

    assert!(slow_nodes_stake < committee.validity_threshold());

    let mut certificates = VecDeque::new();
    let mut parents = initial_parents;
    let mut next_parents = Vec::new();

    for round in range {
        next_parents.clear();
        for name in names {
            let this_cert_parents = this_cert_parents_with_slow_nodes(
                name,
                parents.clone(),
                slow_nodes,
                &mut rand,
                committee,
            );

            let (_, certificate) = mock_certificate(committee, *name, round, this_cert_parents);
            certificates.push_back(certificate.clone());
            next_parents.push(certificate);
        }
        parents = next_parents.clone();
    }
    (certificates, next_parents)
}

#[derive(Debug, Clone, Copy)]
pub enum TestLeaderSupport {
    // There will be support for the leader, but less than f+1
    Weak,
    // There will be strong support for the leader, meaning >= f+1
    Strong,
    // Leader will be completely omitted by the voters
    NoSupport,
}

pub struct TestLeaderConfiguration {
    // The round of the leader
    pub round: Round,
    // The leader id. That allow us to explicitly dictate which we consider the leader to be
    pub authority: AuthorityIdentifier,
    // If true then the leader for that round will not be created at all
    pub should_omit: bool,
    // The support that this leader should receive from the voters of next round
    pub support: Option<TestLeaderSupport>,
}

/// Creates fully connected DAG for the dictated rounds but with specific
/// conditions for the leaders. By providing the `leader_configuration` we can
/// dictate the setup for specific leaders of specific rounds. For a leader the
/// following can be configured:
/// * whether a leader will exist or not for a round
/// * whether a leader will receive enough support from the next round
pub fn make_certificates_with_leader_configuration(
    committee: &Committee,
    range: RangeInclusive<Round>,
    initial_parents: &BTreeSet<CertificateDigest>,
    names: &[AuthorityIdentifier],
    leader_configurations: HashMap<Round, TestLeaderConfiguration>,
) -> (VecDeque<Certificate>, BTreeSet<CertificateDigest>) {
    for round in leader_configurations.keys() {
        assert_eq!(round % 2, 0, "Leaders are elected only on even rounds");
    }

    let mut certificates: VecDeque<Certificate> = VecDeque::new();
    let mut parents = initial_parents.iter().cloned().collect::<BTreeSet<_>>();
    let mut next_parents = BTreeSet::new();

    for round in range {
        next_parents.clear();

        for name in names {
            // should we produce the leader of that round?
            if let Some(leader_config) = leader_configurations.get(&round) {
                if leader_config.should_omit && leader_config.authority == *name {
                    // just skip and don't create the certificate for this authority
                    continue;
                }
            }

            // we now check for the leader of previous round. If should not be omitted we
            // need to check on the support we are supposed to provide
            let cert_parents = if round > 0 {
                if let Some(leader_config) = leader_configurations.get(&(round - 1)) {
                    match leader_config.support {
                        Some(TestLeaderSupport::Weak) => {
                            // find the leader from the previous round
                            let leader_certificate = certificates
                                .iter()
                                .find(|c| {
                                    c.round() == round - 1 && c.origin() == leader_config.authority
                                })
                                .unwrap();

                            // check whether anyone from the current round already included it
                            // if yes, then we should remove it and not vote again.
                            if certificates.iter().any(|c| {
                                c.round() == round
                                    && c.header().parents().contains(&leader_certificate.digest())
                            }) {
                                let mut p = parents.clone();
                                p.remove(&leader_certificate.digest());
                                p
                            } else {
                                // otherwise return all the parents
                                parents.clone()
                            }
                        }
                        Some(TestLeaderSupport::Strong) => {
                            // just return the whole parent set so we can vote for it
                            parents.clone()
                        }
                        Some(TestLeaderSupport::NoSupport) => {
                            // remove the leader from the set of parents
                            let c = certificates
                                .iter()
                                .find(|c| {
                                    c.round() == round - 1 && c.origin() == leader_config.authority
                                })
                                .unwrap();
                            let mut p = parents.clone();
                            p.remove(&c.digest());
                            p
                        }
                        None => parents.clone(),
                    }
                } else {
                    parents.clone()
                }
            } else {
                parents.clone()
            };

            // Create the certificates
            let (_, certificate) = mock_certificate(committee, *name, round, cert_parents);
            certificates.push_back(certificate.clone());
            next_parents.insert(certificate.digest());
        }
        parents = next_parents.clone();
    }
    (certificates, next_parents)
}

// Returns the parents that should be used as part of a newly created
// certificate. The `slow_nodes` parameter is used to dictate which parents to
// exclude and not use. The slow node will not be used under some probability
// which is provided as part of the tuple. If probability to use it is 0.0, then
// the parent node will NEVER be used. If probability to use it is 1.0, then the
// parent node will ALWAYS be used. We always make sure to include our "own"
// certificate, thus the `name` property is needed.
pub fn this_cert_parents_with_slow_nodes(
    authority_id: &AuthorityIdentifier,
    ancestors: Vec<Certificate>,
    slow_nodes: &[(AuthorityIdentifier, f64)],
    rand: &mut StdRng,
    committee: &Committee,
) -> BTreeSet<CertificateDigest> {
    let mut parents = BTreeSet::new();
    let mut not_included = Vec::new();
    let mut total_stake = 0;

    for parent in ancestors {
        let authority = committee.authority(&parent.origin()).unwrap();

        // Identify if the parent is within the slow nodes - and is not the same author
        // as the one we want to create the certificate for.
        if let Some((_, inclusion_probability)) = slow_nodes
            .iter()
            .find(|(id, _)| *id != *authority_id && *id == parent.header().author())
        {
            let b = Bernoulli::new(*inclusion_probability).unwrap();
            let should_include = b.sample(rand);

            if should_include {
                parents.insert(parent.digest());
                total_stake += authority.stake();
            } else {
                not_included.push(parent);
            }
        } else {
            // just add it directly as it is not within the slow nodes or we are the
            // same author.
            parents.insert(parent.digest());
            total_stake += authority.stake();
        }
    }

    // ensure we'll have enough parents (2f + 1)
    while total_stake < committee.quorum_threshold() {
        let parent = not_included.pop().unwrap();
        let authority = committee.authority(&parent.origin()).unwrap();

        total_stake += authority.stake();

        parents.insert(parent.digest());
    }

    assert!(
        committee.reached_quorum(total_stake),
        "Not enough parents by stake provided. Expected at least {} but instead got {}",
        committee.quorum_threshold(),
        total_stake
    );

    parents
}

// make rounds worth of unsigned certificates with the sampled number of parents
pub fn make_certificates_with_epoch(
    committee: &Committee,
    range: RangeInclusive<Round>,
    epoch: Epoch,
    initial_parents: &BTreeSet<CertificateDigest>,
    keys: &[AuthorityIdentifier],
) -> (VecDeque<Certificate>, BTreeSet<CertificateDigest>) {
    let mut certificates = VecDeque::new();
    let mut parents = initial_parents.iter().cloned().collect::<BTreeSet<_>>();
    let mut next_parents = BTreeSet::new();

    for round in range {
        next_parents.clear();
        for name in keys {
            let (digest, certificate) =
                mock_certificate_with_epoch(committee, *name, round, epoch, parents.clone());
            certificates.push_back(certificate);
            next_parents.insert(digest);
        }
        parents = next_parents.clone();
    }
    (certificates, next_parents)
}

// make rounds worth of signed certificates with the sampled number of parents
pub fn make_signed_certificates(
    range: RangeInclusive<Round>,
    initial_parents: &BTreeSet<CertificateDigest>,
    committee: &Committee,
    keys: &[(AuthorityIdentifier, KeyPair)],
    failure_probability: f64,
) -> (VecDeque<Certificate>, BTreeSet<CertificateDigest>) {
    let ids = keys
        .iter()
        .map(|(authority, _)| *authority)
        .collect::<Vec<_>>();
    let generator =
        |pk, round, parents| mock_signed_certificate(keys, pk, round, parents, committee);

    rounds_of_certificates(
        range,
        initial_parents,
        &ids[..],
        failure_probability,
        generator,
    )
}

pub fn mock_certificate_with_rand<R: RngCore + ?Sized>(
    committee: &Committee,
    origin: AuthorityIdentifier,
    round: Round,
    parents: BTreeSet<CertificateDigest>,
    rand: &mut R,
) -> (CertificateDigest, Certificate) {
    let header_builder = HeaderV1Builder::default();
    let header = header_builder
        .author(origin)
        .round(round)
        .epoch(0)
        .parents(parents)
        .payload(fixture_payload_with_rand(1, rand))
        .build()
        .unwrap();
    let certificate = Certificate::new_unsigned(committee, Header::V1(header), Vec::new()).unwrap();
    (certificate.digest(), certificate)
}

// Creates a badly signed certificate from its given round, origin and parents,
// Note: the certificate is signed by a random key rather than its author
pub fn mock_certificate(
    committee: &Committee,
    origin: AuthorityIdentifier,
    round: Round,
    parents: BTreeSet<CertificateDigest>,
) -> (CertificateDigest, Certificate) {
    mock_certificate_with_epoch(committee, origin, round, 0, parents)
}

// Creates a badly signed certificate from its given round, epoch, origin, and
// parents, Note: the certificate is signed by a random key rather than its
// author
pub fn mock_certificate_with_epoch(
    committee: &Committee,
    origin: AuthorityIdentifier,
    round: Round,
    epoch: Epoch,
    parents: BTreeSet<CertificateDigest>,
) -> (CertificateDigest, Certificate) {
    let header_builder = HeaderV1Builder::default();
    let header = header_builder
        .author(origin)
        .round(round)
        .epoch(epoch)
        .parents(parents)
        .payload(fixture_payload(1))
        .build()
        .unwrap();
    let certificate = Certificate::new_unsigned(committee, Header::V1(header), Vec::new()).unwrap();
    (certificate.digest(), certificate)
}

// Creates one signed certificate from a set of signers - the signers must
// include the origin
pub fn mock_signed_certificate(
    signers: &[(AuthorityIdentifier, KeyPair)],
    origin: AuthorityIdentifier,
    round: Round,
    parents: BTreeSet<CertificateDigest>,
    committee: &Committee,
) -> (CertificateDigest, Certificate) {
    let header_builder = HeaderV1Builder::default()
        .author(origin)
        .payload(fixture_payload(1))
        .round(round)
        .epoch(0)
        .parents(parents);

    let header = header_builder.build().unwrap();

    let cert =
        Certificate::new_unsigned(committee, Header::V1(header.clone()), Vec::new()).unwrap();

    let mut votes = Vec::new();
    for (name, signer) in signers {
        let sig = Signature::new_secure(&to_intent_message(cert.header().digest()), signer);
        votes.push((*name, sig))
    }
    let cert = Certificate::new_unverified(committee, Header::V1(header), votes).unwrap();
    (cert.digest(), cert)
}

pub struct Builder<R = OsRng> {
    rng: R,
    committee_size: NonZeroUsize,
    number_of_workers: NonZeroUsize,
    randomize_ports: bool,
    epoch: Epoch,
    protocol_version: ProtocolVersion,
    stake: VecDeque<Stake>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Self {
            epoch: Epoch::default(),
            protocol_version: ProtocolVersion::max(),
            rng: OsRng,
            committee_size: NonZeroUsize::new(4).unwrap(),
            number_of_workers: NonZeroUsize::new(4).unwrap(),
            randomize_ports: false,
            stake: VecDeque::new(),
        }
    }
}

impl<R> Builder<R> {
    pub fn committee_size(mut self, committee_size: NonZeroUsize) -> Self {
        self.committee_size = committee_size;
        self
    }

    pub fn number_of_workers(mut self, number_of_workers: NonZeroUsize) -> Self {
        self.number_of_workers = number_of_workers;
        self
    }

    pub fn randomize_ports(mut self, randomize_ports: bool) -> Self {
        self.randomize_ports = randomize_ports;
        self
    }

    pub fn epoch(mut self, epoch: Epoch) -> Self {
        self.epoch = epoch;
        self
    }

    pub fn protocol_version(mut self, protocol_version: ProtocolVersion) -> Self {
        self.protocol_version = protocol_version;
        self
    }

    pub fn stake_distribution(mut self, stake: VecDeque<Stake>) -> Self {
        self.stake = stake;
        self
    }

    pub fn rng<N: rand::RngCore + rand::CryptoRng>(self, rng: N) -> Builder<N> {
        Builder {
            rng,
            epoch: self.epoch,
            protocol_version: self.protocol_version,
            committee_size: self.committee_size,
            number_of_workers: self.number_of_workers,
            randomize_ports: self.randomize_ports,
            stake: self.stake,
        }
    }
}

impl<R: rand::RngCore + rand::CryptoRng> Builder<R> {
    pub fn build(mut self) -> CommitteeFixture {
        if !self.stake.is_empty() {
            assert_eq!(
                self.stake.len(),
                self.committee_size.get(),
                "Stake vector has been provided but is different length the committee - it should be the same"
            );
        }

        let mut authorities: Vec<AuthorityFixture> = (0..self.committee_size.get())
            .map(|_| {
                AuthorityFixture::generate(
                    StdRng::from_rng(&mut self.rng).unwrap(),
                    self.number_of_workers,
                    |host| {
                        if self.randomize_ports {
                            get_available_port(host)
                        } else {
                            0
                        }
                    },
                )
            })
            .collect();

        // now order the AuthorityFixtures by the authority PublicKey so when we iterate
        // either via the committee.authorities() or via the
        // fixture.authorities() we'll get the same order.
        authorities.sort_by_key(|a1| a1.public_key());

        // create the committee in order to assign the ids to the authorities
        let mut committee_builder = CommitteeBuilder::new(self.epoch);
        for a in authorities.iter() {
            committee_builder = committee_builder.add_authority(
                a.public_key().clone(),
                self.stake.pop_front().unwrap_or(1),
                a.address.clone(),
                a.network_public_key(),
                a.address.to_string(),
            );
        }
        let committee = committee_builder.build();

        // Update the Fixtures with the id assigned from the committee
        for authority in authorities.iter_mut() {
            let a = committee
                .authority_by_key(authority.keypair.public())
                .unwrap();
            authority.authority = OnceCell::with_value(a.clone());
            authority.stake = a.stake();
        }

        // Now update the stake to follow the order of the authorities so we produce
        // expected results
        let authorities: Vec<AuthorityFixture> = authorities
            .into_iter()
            .map(|mut authority| {
                authority.stake = self.stake.pop_front().unwrap_or(1);
                authority
            })
            .collect();

        CommitteeFixture {
            authorities,
            committee,
            epoch: self.epoch,
        }
    }
}

pub struct CommitteeFixture {
    authorities: Vec<AuthorityFixture>,
    committee: Committee,
    epoch: Epoch,
}

impl CommitteeFixture {
    pub fn authority(&self, index: usize) -> &AuthorityFixture {
        &self.authorities[index]
    }

    pub fn authorities(&self) -> impl Iterator<Item = &AuthorityFixture> {
        self.authorities.iter()
    }

    pub fn builder() -> Builder {
        Builder::new()
    }

    pub fn committee(&self) -> Committee {
        self.committee.clone()
    }

    pub fn worker_cache(&self) -> WorkerCache {
        WorkerCache {
            epoch: self.epoch,
            workers: self
                .authorities
                .iter()
                .map(|a| (a.public_key(), a.worker_index()))
                .collect(),
        }
    }

    // pub fn header(&self, author: PublicKey) -> Header {
    // Currently sign with the last authority
    pub fn header(&self) -> Header {
        self.authorities.last().unwrap().header(&self.committee())
    }

    pub fn headers(&self) -> Vec<Header> {
        let committee = self.committee();

        self.authorities
            .iter()
            .map(|a| a.header_with_round(&committee, 1))
            .collect()
    }

    pub fn headers_next_round(&self) -> Vec<Header> {
        let committee = self.committee();
        self.authorities
            .iter()
            .map(|a| a.header_with_round(&committee, 2))
            .collect()
    }

    pub fn headers_round(
        &self,
        prior_round: Round,
        parents: &BTreeSet<CertificateDigest>,
    ) -> (Round, Vec<Header>) {
        let round = prior_round + 1;
        let next_headers = self
            .authorities
            .iter()
            .map(|a| {
                let builder = types::HeaderV1Builder::default();
                let header = builder
                    .author(a.id())
                    .round(round)
                    .epoch(0)
                    .parents(parents.clone())
                    .with_payload_batch(fixture_batch_with_transactions(10), 0, 0)
                    .build()
                    .unwrap();
                Header::V1(header)
            })
            .collect();

        (round, next_headers)
    }

    pub fn votes(&self, header: &Header) -> Vec<Vote> {
        self.authorities()
            .flat_map(|a| {
                // we should not re-sign using the key of the authority
                // that produced the header
                if a.id() == header.author() {
                    None
                } else {
                    Some(a.vote(header))
                }
            })
            .collect()
    }

    pub fn certificate(&self, header: &Header) -> Certificate {
        let committee = self.committee();
        let votes: Vec<_> = self
            .votes(header)
            .into_iter()
            .map(|x| (x.author(), x.signature().clone()))
            .collect();
        Certificate::new_unverified(&committee, header.clone(), votes).unwrap()
    }
}

pub struct AuthorityFixture {
    authority: OnceCell<Authority>,
    keypair: KeyPair,
    network_keypair: NetworkKeyPair,
    stake: Stake,
    address: Multiaddr,
    workers: BTreeMap<WorkerId, WorkerFixture>,
}

impl AuthorityFixture {
    pub fn id(&self) -> AuthorityIdentifier {
        self.authority.get().unwrap().id()
    }

    pub fn authority(&self) -> &Authority {
        self.authority.get().unwrap()
    }

    pub fn keypair(&self) -> &KeyPair {
        &self.keypair
    }

    pub fn network_keypair(&self) -> NetworkKeyPair {
        self.network_keypair.copy()
    }

    pub fn new_network(&self, router: anemo::Router) -> anemo::Network {
        anemo::Network::bind(self.address.to_anemo_address().unwrap())
            .server_name("narwhal")
            .private_key(self.network_keypair().private().0.to_bytes())
            .start(router)
            .unwrap()
    }

    pub fn address(&self) -> &Multiaddr {
        &self.address
    }

    pub fn worker(&self, id: WorkerId) -> &WorkerFixture {
        self.workers.get(&id).unwrap()
    }

    pub fn worker_keypairs(&self) -> Vec<NetworkKeyPair> {
        self.workers
            .values()
            .map(|worker| worker.keypair.copy())
            .collect()
    }

    pub fn public_key(&self) -> PublicKey {
        self.keypair.public().clone()
    }

    pub fn network_public_key(&self) -> NetworkPublicKey {
        self.network_keypair.public().clone()
    }

    pub fn worker_index(&self) -> WorkerIndex {
        WorkerIndex(
            self.workers
                .iter()
                .map(|(id, w)| (*id, w.info.clone()))
                .collect(),
        )
    }

    pub fn header(&self, committee: &Committee) -> Header {
        let header = self
            .header_builder(committee)
            .payload(Default::default())
            .build()
            .unwrap();
        Header::V1(header)
    }

    pub fn header_with_round(&self, committee: &Committee, round: Round) -> Header {
        let header = self
            .header_builder(committee)
            .payload(Default::default())
            .round(round)
            .build()
            .unwrap();
        Header::V1(header)
    }

    pub fn header_builder(&self, committee: &Committee) -> types::HeaderV1Builder {
        types::HeaderV1Builder::default()
            .author(self.id())
            .round(1)
            .epoch(committee.epoch())
            .parents(
                Certificate::genesis(committee)
                    .iter()
                    .map(|x| x.digest())
                    .collect(),
            )
    }

    pub fn vote(&self, header: &Header) -> Vote {
        Vote::new_with_signer(header, &self.id(), &self.keypair)
    }

    fn generate<R, P>(mut rng: R, number_of_workers: NonZeroUsize, mut get_port: P) -> Self
    where
        R: AllowedRng,
        P: FnMut(&str) -> u16,
    {
        let keypair = KeyPair::generate(&mut rng);
        let network_keypair = NetworkKeyPair::generate(&mut rng);
        let host = "127.0.0.1";
        let address: Multiaddr = format!("/ip4/{}/udp/{}", host, get_port(host))
            .parse()
            .unwrap();

        let workers = (0..number_of_workers.get())
            .map(|idx| {
                let worker = WorkerFixture::generate(&mut rng, idx as u32, &mut get_port);

                (idx as u32, worker)
            })
            .collect();

        Self {
            authority: OnceCell::new(),
            keypair,
            network_keypair,
            stake: 1,
            address,
            workers,
        }
    }
}

pub struct WorkerFixture {
    keypair: NetworkKeyPair,
    #[allow(dead_code)]
    id: WorkerId,
    info: WorkerInfo,
}

impl WorkerFixture {
    pub fn keypair(&self) -> NetworkKeyPair {
        self.keypair.copy()
    }

    pub fn info(&self) -> &WorkerInfo {
        &self.info
    }

    pub fn new_network(&self, router: anemo::Router) -> anemo::Network {
        anemo::Network::bind(self.info().worker_address.to_anemo_address().unwrap())
            .server_name("narwhal")
            .private_key(self.keypair().private().0.to_bytes())
            .start(router)
            .unwrap()
    }

    fn generate<R, P>(rng: R, id: WorkerId, mut get_port: P) -> Self
    where
        R: rand::RngCore + rand::CryptoRng,
        P: FnMut(&str) -> u16,
    {
        let keypair = NetworkKeyPair::generate(&mut StdRng::from_rng(rng).unwrap());
        let worker_name = keypair.public().clone();
        let host = "127.0.0.1";
        let worker_address = format!("/ip4/{}/udp/{}", host, get_port(host))
            .parse()
            .unwrap();
        let transactions = format!("/ip4/{}/tcp/{}/http", host, get_port(host))
            .parse()
            .unwrap();

        Self {
            keypair,
            id,
            info: WorkerInfo {
                name: worker_name,
                worker_address,
                transactions,
            },
        }
    }
}

pub fn test_network(keypair: NetworkKeyPair, address: &Multiaddr) -> anemo::Network {
    let address = address.to_anemo_address().unwrap();
    let network_key = keypair.private().0.to_bytes();
    anemo::Network::bind(address)
        .server_name("narwhal")
        .private_key(network_key)
        .start(anemo::Router::new())
        .unwrap()
}

pub fn random_network() -> anemo::Network {
    let network_key = NetworkKeyPair::generate(&mut StdRng::from_rng(OsRng).unwrap());
    let address = "/ip4/127.0.0.1/udp/0".parse().unwrap();
    test_network(network_key, &address)
}
