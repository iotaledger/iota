// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::iota_system_state::epoch_start_iota_system_state::EpochStartSystemStateTrait;
use starfish_config::{Authority, Committee as ConsensusCommittee};
use tracing::error;

/// Returns the consensus committee for an epoch, built from its start state,
/// with authorities sorted by authority key so the order matches the IOTA
/// committee. Consensus types live in `starfish-config`, so this conversion
/// lives here rather than in `iota-types`.
pub fn get_consensus_committee<T: EpochStartSystemStateTrait>(state: &T) -> ConsensusCommittee {
    let committee_validators = state.committee_validators();
    let mut authorities: Vec<Authority> = Vec::with_capacity(committee_validators.len());

    for validator in committee_validators.iter() {
        authorities.push(Authority {
            stake: validator.voting_power as starfish_config::Stake,
            address: validator.primary_address.clone(),
            hostname: validator.hostname.clone(),
            authority_key: <starfish_config::AuthorityPublicKey>::new(
                validator.authority_pubkey.clone(),
            ),
            protocol_key: <starfish_config::ProtocolPublicKey>::new(
                validator.protocol_pubkey.clone(),
            ),
            network_key: <starfish_config::NetworkPublicKey>::new(validator.network_pubkey.clone()),
        });
    }

    // Sort the authorities by their authority (public) key in ascending order, same
    // as the order in the IOTA committee returned from get_iota_committee().
    authorities.sort_by(|a1, a2| a1.authority_key.cmp(&a2.authority_key));

    for ((i, authority), iota_authority_name) in authorities
        .iter()
        .enumerate()
        .zip(state.get_iota_committee().names())
    {
        if iota_authority_name.0 != authority.authority_key.to_bytes() {
            error!(
                "Mismatched authority order between IOTA and Starfish! \
                Index {i}, Starfish authority {authority:?}\nIota authority name {iota_authority_name}"
            );
        }
    }

    ConsensusCommittee::new(state.epoch() as starfish_config::Epoch, authorities)
}

#[cfg(test)]
mod test {
    use fastcrypto::traits::{KeyPair, ToFromBytes};
    use iota_multiaddr::Multiaddr;
    use iota_protocol_config::ProtocolVersion;
    use iota_sdk_types::Address;
    use iota_types::{
        committee::CommitteeTrait,
        crypto::{AuthorityKeyPair, NetworkKeyPair, get_key_pair},
        iota_system_state::epoch_start_iota_system_state::{
            EpochStartSystemState, EpochStartSystemStateTrait, EpochStartValidatorInfoV1,
        },
    };
    use rand::thread_rng;

    use super::get_consensus_committee;

    #[test]
    fn test_iota_and_consensus_committee_are_same() {
        // GIVEN
        let mut committee_validators = vec![];

        for i in 0..10 {
            let (iota_address, authority_key): (Address, AuthorityKeyPair) = get_key_pair();
            let protocol_network_key = NetworkKeyPair::generate(&mut thread_rng());

            committee_validators.push(EpochStartValidatorInfoV1 {
                iota_address,
                authority_pubkey: authority_key.public().clone(),
                network_pubkey: protocol_network_key.public().clone(),
                protocol_pubkey: protocol_network_key.public().clone(),
                iota_net_address: Multiaddr::empty(),
                p2p_address: Multiaddr::empty(),
                primary_address: Multiaddr::empty(),
                voting_power: 1_000,
                hostname: format!("host-{i}").to_string(),
            })
        }

        let state = EpochStartSystemState::new_v1(
            10,
            ProtocolVersion::MAX.as_u64(),
            0,
            false,
            0,
            0,
            committee_validators,
        );

        // WHEN
        let iota_committee = state.get_iota_committee();
        let consensus_committee = get_consensus_committee(&state);

        // THEN
        // assert the validators details
        assert_eq!(iota_committee.num_members(), 10);
        assert_eq!(iota_committee.num_members(), consensus_committee.size());
        assert_eq!(
            iota_committee.validity_threshold(),
            consensus_committee.validity_threshold()
        );
        assert_eq!(
            iota_committee.quorum_threshold(),
            consensus_committee.quorum_threshold()
        );
        assert_eq!(state.epoch(), consensus_committee.epoch());

        for (authority_index, consensus_authority) in consensus_committee.authorities() {
            let iota_authority_name = iota_committee
                .authority_by_index(authority_index.value() as u32)
                .unwrap();

            assert_eq!(
                consensus_authority.authority_key.to_bytes(),
                iota_authority_name.0,
                "IOTA Foundation & IOTA committee member of same index correspond to different public key"
            );
            assert_eq!(
                consensus_authority.stake,
                iota_committee.weight(iota_authority_name),
                "IOTA Foundation & IOTA committee member stake differs"
            );
        }
    }

    #[test]
    fn test_v2_iota_and_consensus_committee_are_same() {
        // GIVEN
        let mut committee_validators = vec![];
        let mut non_committee_validators = vec![];

        for i in 0..10 {
            let (iota_address, authority_key): (Address, AuthorityKeyPair) = get_key_pair();
            let protocol_network_key = NetworkKeyPair::generate(&mut thread_rng());

            committee_validators.push(EpochStartValidatorInfoV1 {
                iota_address,
                authority_pubkey: authority_key.public().clone(),
                network_pubkey: protocol_network_key.public().clone(),
                protocol_pubkey: protocol_network_key.public().clone(),
                iota_net_address: Multiaddr::empty(),
                p2p_address: Multiaddr::empty(),
                primary_address: Multiaddr::empty(),
                voting_power: 1_000,
                hostname: format!("committee-{i}").to_string(),
            });

            let (iota_address, authority_key): (Address, AuthorityKeyPair) = get_key_pair();
            let protocol_network_key = NetworkKeyPair::generate(&mut thread_rng());

            non_committee_validators.push(EpochStartValidatorInfoV1 {
                iota_address,
                authority_pubkey: authority_key.public().clone(),
                network_pubkey: protocol_network_key.public().clone(),
                protocol_pubkey: protocol_network_key.public().clone(),
                iota_net_address: Multiaddr::empty(),
                p2p_address: Multiaddr::empty(),
                primary_address: Multiaddr::empty(),
                voting_power: 500,
                hostname: format!("non-committee-{i}").to_string(),
            });
        }

        // Create active_validators list containing all validators in the desired order
        let mut active_validators = committee_validators.clone();
        active_validators.extend(non_committee_validators.clone());

        let state = EpochStartSystemState::new_v2(
            10,
            ProtocolVersion::MAX.as_u64(),
            0,
            false,
            0,
            0,
            committee_validators.clone(),
            active_validators,
        );

        // WHEN
        let iota_committee = state.get_iota_committee();
        let consensus_committee = get_consensus_committee(&state);
        let active_validators = state.get_active_validators();

        // THEN
        // Assert committee validators details
        assert_eq!(iota_committee.num_members(), 10);
        assert_eq!(iota_committee.num_members(), consensus_committee.size());
        assert_eq!(
            iota_committee.validity_threshold(),
            consensus_committee.validity_threshold()
        );
        assert_eq!(
            iota_committee.quorum_threshold(),
            consensus_committee.quorum_threshold()
        );

        // Verify committee validators are correctly mapped
        for (authority_index, consensus_authority) in consensus_committee.authorities() {
            let iota_authority_name = iota_committee
                .authority_by_index(authority_index.value() as u32)
                .unwrap();

            assert_eq!(
                consensus_authority.authority_key.to_bytes(),
                iota_authority_name.0,
                "IOTA Foundation & IOTA committee member of same index correspond to different public key"
            );
            assert_eq!(
                consensus_authority.stake,
                iota_committee.weight(iota_authority_name),
                "IOTA Foundation & IOTA committee member stake differs"
            );
        }

        // Verify active validators (should include all: committee + non-committee)
        assert_eq!(active_validators.len(), 20); // 10 committee + 10 non-committee

        // Verify order is preserved - active_validators should contain all validators
        // in the expected order First committee validators, then non-committee
        // validators
        let expected_order: Vec<_> = committee_validators
            .iter()
            .chain(non_committee_validators.iter())
            .collect();

        for (i, expected_validator) in expected_order.iter().enumerate() {
            let found_pubkey = &active_validators[i];
            assert_eq!(
                found_pubkey.as_bytes(),
                expected_validator.authority_pubkey.as_bytes(),
                "Order not preserved: expected validator at index {i}",
            );
        }

        // Verify committee validators are in active_validators
        for validator in committee_validators.iter() {
            let found = active_validators
                .iter()
                .find(|pubkey| pubkey.as_bytes() == validator.authority_pubkey.as_bytes())
                .unwrap();
            assert_eq!(found.as_bytes(), validator.authority_pubkey.as_bytes());
        }

        // Verify non-committee validators are in active_validators
        for validator in non_committee_validators.iter() {
            let found = active_validators
                .iter()
                .find(|pubkey| pubkey.as_bytes() == validator.authority_pubkey.as_bytes())
                .unwrap();
            assert_eq!(found.as_bytes(), validator.authority_pubkey.as_bytes());
        }
    }
}
