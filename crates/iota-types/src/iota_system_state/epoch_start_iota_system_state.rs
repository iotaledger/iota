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
use tracing::{error, warn};

use crate::{
    base_types::{AuthorityName, EpochId, IotaAddress},
    committee::{Committee, CommitteeWithNetworkMetadata, NetworkMetadata, StakeUnit},
    crypto::{AuthorityPublicKey, NetworkPublicKey},
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
    fn get_validator_as_p2p_peers(&self, excluding_self: AuthorityName) -> Vec<PeerInfo>;
    fn get_authority_names_to_peer_ids(&self) -> HashMap<AuthorityName, PeerId>;
    fn get_authority_names_to_hostnames(&self) -> HashMap<AuthorityName, String>;
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
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
#[enum_dispatch(EpochStartSystemStateTrait)]
pub enum EpochStartSystemState {
    V1(EpochStartSystemStateV1),
}

impl EpochStartSystemState {
    pub fn new_v1(
        epoch: EpochId,
        protocol_version: u64,
        reference_gas_price: u64,
        safe_mode: bool,
        epoch_start_timestamp_ms: u64,
        epoch_duration_ms: u64,
        active_validators: Vec<EpochStartValidatorInfoV1>,
    ) -> Self {
        Self::V1(EpochStartSystemStateV1 {
            epoch,
            protocol_version,
            reference_gas_price,
            safe_mode,
            epoch_start_timestamp_ms,
            epoch_duration_ms,
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
                active_validators: state.active_validators.clone(),
            }),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct EpochStartSystemStateV1 {
    epoch: EpochId,
    protocol_version: u64,
    reference_gas_price: u64,
    safe_mode: bool,
    epoch_start_timestamp_ms: u64,
    epoch_duration_ms: u64,
    active_validators: Vec<EpochStartValidatorInfoV1>,
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
            active_validators: vec![],
        }
    }
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
        self.active_validators
            .iter()
            .map(|validator| validator.iota_address)
            .collect()
    }

    fn get_iota_committee_with_network_metadata(&self) -> CommitteeWithNetworkMetadata {
        let validators = self
            .active_validators
            .iter()
            .map(|validator| {
                (
                    validator.authority_name(),
                    (
                        validator.voting_power,
                        NetworkMetadata {
                            network_address: validator.iota_net_address.clone(),
                            primary_address: validator.primary_address.clone(),
                        },
                    ),
                )
            })
            .collect();

        CommitteeWithNetworkMetadata::new(self.epoch, validators)
    }

    fn get_iota_committee(&self) -> Committee {
        let voting_rights = self
            .active_validators
            .iter()
            .map(|validator| (validator.authority_name(), validator.voting_power))
            .collect();
        Committee::new(self.epoch, voting_rights)
    }

    fn get_consensus_committee(&self) -> ConsensusCommittee {
        let mut authorities = vec![];
        for validator in self.active_validators.iter() {
            authorities.push(Authority {
                stake: validator.voting_power as consensus_config::Stake,
                address: validator.primary_address.clone(),
                hostname: validator.hostname.clone(),
                authority_key: consensus_config::AuthorityPublicKey::new(
                    validator.authority_pubkey.clone(),
                ),
                protocol_key: consensus_config::ProtocolPublicKey::new(
                    validator.protocol_pubkey.clone(),
                ),
                network_key: consensus_config::NetworkPublicKey::new(
                    validator.network_pubkey.clone(),
                ),
            });
        }

        // Sort the authorities by their authority (public) key in ascending order, same
        // as the order in the IOTA committee returned from get_iota_committee().
        authorities.sort_by(|a1, a2| a1.authority_key.cmp(&a2.authority_key));

        for ((i, mysticeti_authority), iota_authority_name) in authorities
            .iter()
            .enumerate()
            .zip(self.get_iota_committee().names())
        {
            if iota_authority_name.0 != mysticeti_authority.authority_key.to_bytes() {
                error!(
                    "Mismatched authority order between IOTA and Mysticeti! Index {}, Mysticeti authority {:?}\nIota authority name {}",
                    i, mysticeti_authority, iota_authority_name
                );
            }
        }

        ConsensusCommittee::new(self.epoch as consensus_config::Epoch, authorities)
    }

    fn get_validator_as_p2p_peers(&self, excluding_self: AuthorityName) -> Vec<PeerInfo> {
        self.active_validators
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
        self.active_validators
            .iter()
            .map(|validator| {
                let name = validator.authority_name();
                let peer_id = PeerId(validator.network_pubkey.0.to_bytes());

                (name, peer_id)
            })
            .collect()
    }

    fn get_authority_names_to_hostnames(&self) -> HashMap<AuthorityName, String> {
        self.active_validators
            .iter()
            .map(|validator| {
                let name = validator.authority_name();
                let hostname = validator.hostname.clone();

                (name, hostname)
            })
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

#[cfg(test)]
mod test {
    use fastcrypto::traits::KeyPair;
    use iota_network_stack::Multiaddr;
    use iota_protocol_config::ProtocolVersion;
    use rand::thread_rng;

    use crate::{
        base_types::IotaAddress,
        committee::CommitteeTrait,
        crypto::{AuthorityKeyPair, NetworkKeyPair, get_key_pair},
        iota_system_state::epoch_start_iota_system_state::{
            EpochStartSystemStateTrait, EpochStartSystemStateV1, EpochStartValidatorInfoV1,
        },
    };

    #[test]
    fn test_iota_and_mysticeti_committee_are_same() {
        // GIVEN
        let mut active_validators = vec![];

        for i in 0..10 {
            let (iota_address, authority_key): (IotaAddress, AuthorityKeyPair) = get_key_pair();
            let protocol_network_key = NetworkKeyPair::generate(&mut thread_rng());

            active_validators.push(EpochStartValidatorInfoV1 {
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
            active_validators,
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
}
