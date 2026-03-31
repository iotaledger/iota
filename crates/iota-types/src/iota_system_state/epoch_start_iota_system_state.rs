// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anemo::{
    PeerId,
    types::{PeerAffinity, PeerInfo},
};
use consensus_config::{Authority, Committee as ConsensusCommittee};
use enum_dispatch::enum_dispatch;
use iota_protocol_config::ProtocolVersion;
use serde::{Deserialize, Serialize};
use starfish_config::{Authority as StarfishAuthority, Committee as StarfishCommittee};
use tracing::{error, warn};

use crate::{
    base_types::{AuthorityName, EpochId, IotaAddress},
    committee::{Committee, CommitteeWithNetworkMetadata, NetworkMetadata, StakeUnit},
    crypto::{AuthorityPublicKey, NetworkPublicKey},
    iota_system_state::iota_system_state_inner_v1::ValidatorV1,
    multiaddr::Multiaddr,
};

#[enum_dispatch]
pub trait EpochStartSystemStateTrait {
    fn epoch(&self) -> EpochId;
    fn protocol_version(&self) -> ProtocolVersion;
    fn reference_gas_price(&self) -> u64;
    fn safe_mode(&self) -> bool;
    fn epoch_start_timestamp_ms(&self) -> u64;
    fn epoch_duration_ms(&self) -> u64;
    fn get_validator_addresses(&self) -> Vec<IotaAddress>;
    fn get_iota_committee(&self) -> Committee;
    fn get_iota_committee_with_network_metadata(&self) -> CommitteeWithNetworkMetadata;
    fn get_consensus_committee(&self) -> ConsensusCommittee;
    fn get_starfish_committee(&self) -> StarfishCommittee;
    fn get_validator_as_p2p_peers(&self, excluding_self: AuthorityName) -> Vec<PeerInfo>;
    fn get_authority_names_to_peer_ids(&self) -> HashMap<AuthorityName, PeerId>;
    fn get_authority_names_to_hostnames(&self) -> HashMap<AuthorityName, String>;
    fn get_active_validators(&self) -> Vec<AuthorityPublicKey>;
}

/// This type captures the minimum amount of information from IotaSystemState
/// needed by a validator to run the protocol. This allows us to decouple from
/// the actual IotaSystemState type, and hence do not need to evolve it when we
/// upgrade the IotaSystemState type. Evolving EpochStartSystemState is also a
/// lot easier in that we could add optional fields and fill them with None for
/// older versions. When we absolutely must delete fields, we could also add new
/// db tables to store the new version. This is OK because we only store one
/// copy of this as part of EpochStartConfiguration for the most recent epoch in
/// the db.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
#[enum_dispatch(EpochStartSystemStateTrait)]
pub enum EpochStartSystemState {
    V1(EpochStartSystemStateV1),
    V2(EpochStartSystemStateV2),
}

impl EpochStartSystemState {
    pub fn new_v1(
        epoch: EpochId,
        protocol_version: u64,
        reference_gas_price: u64,
        safe_mode: bool,
        epoch_start_timestamp_ms: u64,
        epoch_duration_ms: u64,
        committee_validators: Vec<EpochStartValidatorInfoV1>,
    ) -> Self {
        Self::V1(EpochStartSystemStateV1 {
            epoch,
            protocol_version,
            reference_gas_price,
            safe_mode,
            epoch_start_timestamp_ms,
            epoch_duration_ms,
            committee_validators,
        })
    }

    pub fn new_v2(
        epoch: EpochId,
        protocol_version: u64,
        reference_gas_price: u64,
        safe_mode: bool,
        epoch_start_timestamp_ms: u64,
        epoch_duration_ms: u64,
        committee_validators: Vec<EpochStartValidatorInfoV1>,
        active_validators: Vec<EpochStartValidatorInfoV1>,
    ) -> Self {
        Self::V2(EpochStartSystemStateV2 {
            v1: EpochStartSystemStateV1 {
                epoch,
                protocol_version,
                reference_gas_price,
                safe_mode,
                epoch_start_timestamp_ms,
                epoch_duration_ms,
                committee_validators,
            },
            active_validators,
        })
    }

    pub fn new_for_testing_with_epoch(epoch: EpochId) -> Self {
        Self::V1(EpochStartSystemStateV1::new_for_testing_with_epoch(epoch))
    }

    pub fn new_at_next_epoch_for_testing(&self) -> Self {
        // Only need to support the latest version for testing.
        match self {
            Self::V1(state) => Self::V1(EpochStartSystemStateV1 {
                epoch: state.epoch + 1,
                protocol_version: state.protocol_version,
                reference_gas_price: state.reference_gas_price,
                safe_mode: state.safe_mode,
                epoch_start_timestamp_ms: state.epoch_start_timestamp_ms,
                epoch_duration_ms: state.epoch_duration_ms,
                committee_validators: state.committee_validators.clone(),
            }),
            Self::V2(state) => Self::V2(EpochStartSystemStateV2 {
                v1: EpochStartSystemStateV1 {
                    epoch: state.v1.epoch + 1,
                    protocol_version: state.v1.protocol_version,
                    reference_gas_price: state.v1.reference_gas_price,
                    safe_mode: state.v1.safe_mode,
                    epoch_start_timestamp_ms: state.v1.epoch_start_timestamp_ms,
                    epoch_duration_ms: state.v1.epoch_duration_ms,
                    committee_validators: state.v1.committee_validators.clone(),
                },
                active_validators: state.active_validators.clone(),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct EpochStartSystemStateV1 {
    epoch: EpochId,
    protocol_version: u64,
    reference_gas_price: u64,
    safe_mode: bool,
    epoch_start_timestamp_ms: u64,
    epoch_duration_ms: u64,
    committee_validators: Vec<EpochStartValidatorInfoV1>,
}

impl EpochStartSystemStateV1 {
    pub fn new_for_testing() -> Self {
        Self::new_for_testing_with_epoch(0)
    }

    pub fn new_for_testing_with_epoch(epoch: EpochId) -> Self {
        Self {
            epoch,
            protocol_version: ProtocolVersion::MAX.as_u64(),
            reference_gas_price: crate::transaction::DEFAULT_VALIDATOR_GAS_PRICE,
            safe_mode: false,
            epoch_start_timestamp_ms: 0,
            epoch_duration_ms: 1000,
            committee_validators: vec![],
        }
    }
}
// Helper macro to implement get_committee functions for different committee
// types.
macro_rules! impl_get_committee {
    (
        // function signature
        fn $fn_name:ident -> $committee_ty:path,
        // Committee authority type
        authority = $authority_ty:ident,
        // Config module where the committee authority public key types are defined
        cfg = $cfg:ident,
        // Label for logging purposes
        label = $label:expr
    ) => {
        fn $fn_name(&self) -> $committee_ty {
            let mut authorities: Vec<$authority_ty> =
                Vec::with_capacity(self.committee_validators.len());

            for validator in self.committee_validators.iter() {
                authorities.push($authority_ty {
                    stake: validator.voting_power as $cfg::Stake,
                    address: validator.primary_address.clone(),
                    hostname: validator.hostname.clone(),
                    authority_key: <$cfg::AuthorityPublicKey>::new(
                        validator.authority_pubkey.clone(),
                    ),
                    protocol_key: <$cfg::ProtocolPublicKey>::new(validator.protocol_pubkey.clone()),
                    network_key: <$cfg::NetworkPublicKey>::new(validator.network_pubkey.clone()),
                });
            }

            // Sort the authorities by their authority (public) key in ascending order, same
            // as the order in the IOTA committee returned from get_iota_committee().
            authorities.sort_by(|a1, a2| a1.authority_key.cmp(&a2.authority_key));

            for ((i, authority), iota_authority_name) in authorities
                .iter()
                .enumerate()
                .zip(self.get_iota_committee().names())
            {
                if iota_authority_name.0 != authority.authority_key.to_bytes() {
                    error!(
                        "Mismatched authority order between IOTA and {}! \
                        Index {}, {} authority {:?}\nIota authority name {}",
                        $label, i, $label, authority, iota_authority_name
                    );
                }
            }

            <$committee_ty>::new(self.epoch as $cfg::Epoch, authorities)
        }
    };
}

impl EpochStartSystemStateTrait for EpochStartSystemStateV1 {
    fn epoch(&self) -> EpochId {
        self.epoch
    }

    fn protocol_version(&self) -> ProtocolVersion {
        ProtocolVersion::new(self.protocol_version)
    }

    fn reference_gas_price(&self) -> u64 {
        self.reference_gas_price
    }

    fn safe_mode(&self) -> bool {
        self.safe_mode
    }

    fn epoch_start_timestamp_ms(&self) -> u64 {
        self.epoch_start_timestamp_ms
    }

    fn epoch_duration_ms(&self) -> u64 {
        self.epoch_duration_ms
    }

    fn get_validator_addresses(&self) -> Vec<IotaAddress> {
        self.committee_validators
            .iter()
            .map(|validator| validator.iota_address)
            .collect()
    }

    fn get_iota_committee_with_network_metadata(&self) -> CommitteeWithNetworkMetadata {
        let validators = self
            .committee_validators
            .iter()
            .map(|validator| {
                (
                    validator.authority_name(),
                    (
                        validator.voting_power,
                        NetworkMetadata {
                            network_address: validator.iota_net_address.clone(),
                            primary_address: validator.primary_address.clone(),
                            network_public_key: Some(validator.network_pubkey.clone()),
                        },
                    ),
                )
            })
            .collect();

        CommitteeWithNetworkMetadata::new(self.epoch, validators)
    }

    fn get_iota_committee(&self) -> Committee {
        let voting_rights = self
            .committee_validators
            .iter()
            .map(|validator| (validator.authority_name(), validator.voting_power))
            .collect();
        Committee::new(self.epoch, voting_rights)
    }

    impl_get_committee!(
        fn get_consensus_committee -> ConsensusCommittee,
        authority = Authority,
        cfg = consensus_config,
        label = "Mysticeti"
    );

    impl_get_committee!(
        fn get_starfish_committee -> StarfishCommittee,
        authority = StarfishAuthority,
        cfg = starfish_config,
        label = "Starfish"
    );

    fn get_validator_as_p2p_peers(&self, excluding_self: AuthorityName) -> Vec<PeerInfo> {
        self.committee_validators
            .iter()
            .filter(|validator| validator.authority_name() != excluding_self)
            .map(|validator| {
                let address = validator
                    .p2p_address
                    .to_anemo_address()
                    .into_iter()
                    .collect::<Vec<_>>();
                let peer_id = PeerId(validator.network_pubkey.0.to_bytes());
                if address.is_empty() {
                    warn!(
                        ?peer_id,
                        "Peer has invalid p2p address: {}", &validator.p2p_address
                    );
                }
                PeerInfo {
                    peer_id,
                    affinity: PeerAffinity::High,
                    address,
                }
            })
            .collect()
    }

    fn get_authority_names_to_peer_ids(&self) -> HashMap<AuthorityName, PeerId> {
        self.committee_validators
            .iter()
            .map(|validator| {
                let name = validator.authority_name();
                let peer_id = PeerId(validator.network_pubkey.0.to_bytes());

                (name, peer_id)
            })
            .collect()
    }

    fn get_authority_names_to_hostnames(&self) -> HashMap<AuthorityName, String> {
        self.committee_validators
            .iter()
            .map(|validator| {
                let name = validator.authority_name();
                let hostname = validator.hostname.clone();

                (name, hostname)
            })
            .collect()
    }

    fn get_active_validators(&self) -> Vec<AuthorityPublicKey> {
        // for V1 the committee and active validators are the same as there is no
        // additional committee selection
        self.committee_validators
            .iter()
            .map(|validator| validator.authority_pubkey.clone())
            .collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct EpochStartSystemStateV2 {
    v1: EpochStartSystemStateV1,
    active_validators: Vec<EpochStartValidatorInfoV1>,
}

impl EpochStartSystemStateV2 {
    pub fn new_for_testing() -> Self {
        Self::new_for_testing_with_epoch(0)
    }

    pub fn new_for_testing_with_epoch(epoch: EpochId) -> Self {
        Self {
            v1: EpochStartSystemStateV1::new_for_testing_with_epoch(epoch),
            active_validators: vec![],
        }
    }
}

impl EpochStartSystemStateTrait for EpochStartSystemStateV2 {
    fn epoch(&self) -> EpochId {
        self.v1.epoch()
    }

    fn protocol_version(&self) -> ProtocolVersion {
        self.v1.protocol_version()
    }

    fn reference_gas_price(&self) -> u64 {
        self.v1.reference_gas_price()
    }

    fn safe_mode(&self) -> bool {
        self.v1.safe_mode()
    }

    fn epoch_start_timestamp_ms(&self) -> u64 {
        self.v1.epoch_start_timestamp_ms()
    }

    fn epoch_duration_ms(&self) -> u64 {
        self.v1.epoch_duration_ms()
    }

    fn get_validator_addresses(&self) -> Vec<IotaAddress> {
        self.v1.get_validator_addresses()
    }

    fn get_iota_committee_with_network_metadata(&self) -> CommitteeWithNetworkMetadata {
        self.v1.get_iota_committee_with_network_metadata()
    }

    fn get_iota_committee(&self) -> Committee {
        self.v1.get_iota_committee()
    }

    fn get_consensus_committee(&self) -> ConsensusCommittee {
        self.v1.get_consensus_committee()
    }

    fn get_starfish_committee(&self) -> StarfishCommittee {
        self.v1.get_starfish_committee()
    }

    fn get_validator_as_p2p_peers(&self, excluding_self: AuthorityName) -> Vec<PeerInfo> {
        self.v1.get_validator_as_p2p_peers(excluding_self)
    }

    fn get_authority_names_to_peer_ids(&self) -> HashMap<AuthorityName, PeerId> {
        self.v1.get_authority_names_to_peer_ids()
    }

    fn get_authority_names_to_hostnames(&self) -> HashMap<AuthorityName, String> {
        self.v1.get_authority_names_to_hostnames()
    }

    fn get_active_validators(&self) -> Vec<AuthorityPublicKey> {
        self.active_validators
            .iter()
            .map(|validator| validator.authority_pubkey.clone())
            .collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct EpochStartValidatorInfoV1 {
    pub iota_address: IotaAddress,
    pub authority_pubkey: AuthorityPublicKey,
    pub network_pubkey: NetworkPublicKey,
    pub protocol_pubkey: NetworkPublicKey,
    pub iota_net_address: Multiaddr,
    pub p2p_address: Multiaddr,
    pub primary_address: Multiaddr,
    pub voting_power: StakeUnit,
    pub hostname: String,
}

impl EpochStartValidatorInfoV1 {
    pub fn authority_name(&self) -> AuthorityName {
        (&self.authority_pubkey).into()
    }
}

/// Converts a validator into EpochStartValidatorInfoV1
pub fn convert_validator_to_epoch_start_info(validator: &ValidatorV1) -> EpochStartValidatorInfoV1 {
    let metadata = validator.verified_metadata();
    EpochStartValidatorInfoV1 {
        iota_address: metadata.iota_address,
        authority_pubkey: metadata.authority_pubkey.clone(),
        network_pubkey: metadata.network_pubkey.clone(),
        protocol_pubkey: metadata.protocol_pubkey.clone(),
        iota_net_address: metadata.net_address.clone(),
        p2p_address: metadata.p2p_address.clone(),
        primary_address: metadata.primary_address.clone(),
        voting_power: validator.voting_power,
        hostname: metadata.name.clone(),
    }
}

#[cfg(test)]
mod test {
    use bcs;
    use fastcrypto::traits::{KeyPair, ToFromBytes};
    use iota_network_stack::Multiaddr;
    use iota_protocol_config::ProtocolVersion;
    use rand::thread_rng;

    use crate::{
        base_types::IotaAddress,
        committee::CommitteeTrait,
        crypto::{AuthorityKeyPair, NetworkKeyPair, get_key_pair},
        iota_system_state::epoch_start_iota_system_state::{
            EpochStartSystemState, EpochStartSystemStateTrait, EpochStartSystemStateV1,
            EpochStartValidatorInfoV1,
        },
    };

    #[test]
    fn test_iota_and_mysticeti_committee_are_same() {
        // GIVEN
        let mut committee_validators = vec![];

        for i in 0..10 {
            let (iota_address, authority_key): (IotaAddress, AuthorityKeyPair) = get_key_pair();
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

        let state = EpochStartSystemStateV1 {
            epoch: 10,
            protocol_version: ProtocolVersion::MAX.as_u64(),
            reference_gas_price: 0,
            safe_mode: false,
            epoch_start_timestamp_ms: 0,
            epoch_duration_ms: 0,
            committee_validators,
        };

        // WHEN
        let iota_committee = state.get_iota_committee();
        let consensus_committee = state.get_consensus_committee();

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
        assert_eq!(state.epoch, consensus_committee.epoch());

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
    fn test_v2_iota_and_mysticeti_committee_are_same() {
        // GIVEN
        let mut committee_validators = vec![];
        let mut non_committee_validators = vec![];

        for i in 0..10 {
            let (iota_address, authority_key): (IotaAddress, AuthorityKeyPair) = get_key_pair();
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

            let (iota_address, authority_key): (IotaAddress, AuthorityKeyPair) = get_key_pair();
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
        let consensus_committee = state.get_consensus_committee();
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

    #[test]
    fn test_epoch_start_system_state_versioning() {
        // Create test validators
        let (iota_address1, authority_key1): (IotaAddress, AuthorityKeyPair) = get_key_pair();
        let protocol_network_key1 = NetworkKeyPair::generate(&mut thread_rng());
        let net_address1 = "/ip4/127.0.0.1/tcp/1337".parse().unwrap();
        let p2p_address1 = "/ip4/127.0.0.1/tcp/1338".parse().unwrap();
        let primary_address1 = "/ip4/127.0.0.1/tcp/1339".parse().unwrap();

        let committee_validator = EpochStartValidatorInfoV1 {
            iota_address: iota_address1,
            authority_pubkey: authority_key1.public().clone(),
            network_pubkey: protocol_network_key1.public().clone(),
            protocol_pubkey: protocol_network_key1.public().clone(),
            iota_net_address: net_address1,
            p2p_address: p2p_address1,
            primary_address: primary_address1,
            voting_power: 1_000,
            hostname: "committee-1.example.com".to_string(),
        };

        let (iota_address2, authority_key2): (IotaAddress, AuthorityKeyPair) = get_key_pair();
        let protocol_network_key2 = NetworkKeyPair::generate(&mut thread_rng());
        let net_address2: Multiaddr = "/ip4/127.0.0.1/tcp/2337".parse().unwrap();
        let p2p_address2: Multiaddr = "/ip4/127.0.0.1/tcp/2338".parse().unwrap();
        let primary_address2: Multiaddr = "/ip4/127.0.0.1/tcp/2339".parse().unwrap();

        let non_committee_validator = EpochStartValidatorInfoV1 {
            iota_address: iota_address2,
            authority_pubkey: authority_key2.public().clone(),
            network_pubkey: protocol_network_key2.public().clone(),
            protocol_pubkey: protocol_network_key2.public().clone(),
            iota_net_address: net_address2,
            p2p_address: p2p_address2,
            primary_address: primary_address2,
            voting_power: 500,
            hostname: "non-committee-1.example.com".to_string(),
        };

        // Create test states with non-default values
        let v1_state = EpochStartSystemState::new_v1(
            10,
            ProtocolVersion::MAX.as_u64(),
            100_000,
            true,
            1_000_000,
            2_000_000,
            vec![committee_validator.clone()],
        );

        let v2_state = EpochStartSystemState::new_v2(
            20,
            ProtocolVersion::MAX.as_u64(),
            200_000,
            true,
            3_000_000,
            4_000_000,
            vec![committee_validator.clone()],
            vec![committee_validator.clone(), non_committee_validator.clone()],
        );

        // Test V1 serialization/deserialization
        let v1_serialized = bcs::to_bytes(&v1_state).unwrap();
        let v1_deserialized: EpochStartSystemState = bcs::from_bytes(&v1_serialized).unwrap();

        // Verify all V1 fields
        assert_eq!(v1_deserialized.epoch(), 10);
        assert_eq!(v1_deserialized.protocol_version(), ProtocolVersion::MAX);
        assert_eq!(v1_deserialized.reference_gas_price(), 100_000);
        assert!(v1_deserialized.safe_mode());
        assert_eq!(v1_deserialized.epoch_start_timestamp_ms(), 1_000_000);
        assert_eq!(v1_deserialized.epoch_duration_ms(), 2_000_000);

        let v1_committee = v1_deserialized.get_iota_committee_with_network_metadata();
        assert_eq!(v1_committee.epoch(), 10);
        assert_eq!(v1_committee.validators().len(), 1);

        let v1_validators = v1_deserialized.get_validator_addresses();
        assert_eq!(v1_validators.len(), 1);
        assert_eq!(v1_validators[0], iota_address1);

        let v1_active_validators = v1_deserialized.get_active_validators();
        assert_eq!(v1_active_validators.len(), 1); // in V1 committee_validators and active_validators are the same as there is no additional committee selection

        // Test V2 serialization/deserialization
        let v2_serialized = bcs::to_bytes(&v2_state).unwrap();
        let v2_deserialized: EpochStartSystemState = bcs::from_bytes(&v2_serialized).unwrap();

        // Verify all V2 fields
        assert_eq!(v2_deserialized.epoch(), 20);
        assert_eq!(v2_deserialized.protocol_version(), ProtocolVersion::MAX);
        assert_eq!(v2_deserialized.reference_gas_price(), 200_000);
        assert!(v2_deserialized.safe_mode());
        assert_eq!(v2_deserialized.epoch_start_timestamp_ms(), 3_000_000);
        assert_eq!(v2_deserialized.epoch_duration_ms(), 4_000_000);

        let v2_committee = v2_deserialized.get_iota_committee_with_network_metadata();
        assert_eq!(v2_committee.epoch(), 20);
        assert_eq!(v2_committee.validators().len(), 1);

        let v2_validators = v2_deserialized.get_validator_addresses();
        assert_eq!(v2_validators.len(), 1);
        assert_eq!(v2_validators[0], iota_address1);

        let v2_active_validators = v2_deserialized.get_active_validators();
        assert_eq!(v2_active_validators.len(), 2); // Should contain one committee member and one validator.

        let non_committee_validator_pubkey = v2_active_validators
            .iter()
            .find(|pubkey| pubkey.as_bytes() == non_committee_validator.authority_pubkey.as_bytes())
            .unwrap();

        assert_eq!(
            non_committee_validator_pubkey.as_bytes(),
            non_committee_validator.authority_pubkey.as_bytes()
        );

        let committee_validator_pubkey = v2_active_validators
            .iter()
            .find(|pubkey| pubkey.as_bytes() == committee_validator.authority_pubkey.as_bytes())
            .unwrap();

        assert_eq!(
            committee_validator_pubkey.as_bytes(),
            committee_validator.authority_pubkey.as_bytes()
        );
    }
}
