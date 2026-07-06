// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use insta::assert_yaml_snapshot;
use iota_network_stack::Multiaddr;
use rand::{SeedableRng as _, rngs::StdRng};
use starfish_config::{
    Authority, AuthorityKeyPair, Committee, NetworkKeyPair, ProtocolKeyPair, Stake,
};

fn test_authorities(stakes: &[Stake]) -> Vec<Authority> {
    let mut authorities = vec![];
    let mut rng = StdRng::from_seed([9; 32]);
    for (i, stake) in stakes.iter().enumerate() {
        let authority_keypair = AuthorityKeyPair::generate(&mut rng);
        let protocol_keypair = ProtocolKeyPair::generate(&mut rng);
        let network_keypair = NetworkKeyPair::generate(&mut rng);
        authorities.push(Authority {
            stake: *stake,
            address: Multiaddr::empty(),
            hostname: format!("test_host_{i}"),
            authority_key: authority_keypair.public(),
            protocol_key: protocol_keypair.public(),
            network_key: network_keypair.public(),
        });
    }
    authorities
}

// Committee is not sent over network or stored on disk itself, but some of its
// fields are. So this test can still be useful to detect accidental format
// changes.
#[test]
fn committee_snapshot_matches() {
    let epoch = 100;

    let mut authorities: Vec<_> = vec![];
    let mut rng = StdRng::from_seed([9; 32]);
    let num_of_authorities = 10;
    for i in 1..=num_of_authorities {
        let authority_keypair = AuthorityKeyPair::generate(&mut rng);
        let protocol_keypair = ProtocolKeyPair::generate(&mut rng);
        let network_keypair = NetworkKeyPair::generate(&mut rng);
        authorities.push(Authority {
            stake: i as Stake,
            address: Multiaddr::empty(),
            hostname: "test_host".to_string(),
            authority_key: authority_keypair.public(),
            protocol_key: protocol_keypair.public(),
            network_key: network_keypair.public(),
        });
    }

    let committee = Committee::new(epoch, authorities);

    assert_yaml_snapshot!("committee", committee)
}

#[test]
#[should_panic(expected = "cannot have zero stake")]
fn committee_rejects_zero_stake_authority() {
    Committee::new(0, test_authorities(&[1, 0, 1]));
}

#[test]
#[should_panic(expected = "Total stake must not overflow")]
fn committee_rejects_total_stake_overflow() {
    Committee::new(0, test_authorities(&[u64::MAX, 1]));
}

#[test]
fn committee_computes_thresholds_for_large_total_stake() {
    let committee = Committee::new(0, test_authorities(&[u64::MAX / 2, u64::MAX / 2]));
    let total = committee.total_stake();
    assert_eq!(total, u64::MAX - 1);
    // A wrapped threshold computation would land near total / 3.
    assert!(committee.quorum_threshold() > total / 2);
    assert!(committee.quorum_threshold() <= total);
}
