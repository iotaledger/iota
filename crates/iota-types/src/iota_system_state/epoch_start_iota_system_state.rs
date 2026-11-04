// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use anemo::{
    PeerId,
    types::{PeerAffinity, PeerInfo},
};
use enum_dispatch::enum_dispatch;
use iota_multiaddr::Multiaddr;
use iota_protocol_config::ProtocolVersion;
use iota_sdk_types::Address;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
    base_types::{AuthorityName, EpochId},
    committee::{Committee, CommitteeWithNetworkMetadata, NetworkMetadata, StakeUnit},
    crypto::{AggregateAuthorityPublicKey, NetworkPublicKey},
    iota_system_state::iota_system_state_inner_v1::ValidatorV1,
};

#[enum_dispatch]
pub trait EpochStartSystemStateTrait {
    fn epoch(&self) -> EpochId;
    fn protocol_version(&self) -> ProtocolVersion;
    fn reference_gas_price(&self) -> u64;
    fn safe_mode(&self) -> bool;
    fn epoch_start_timestamp_ms(&self) -> u64;
    fn epoch_duration_ms(&self) -> u64;
    fn get_validator_addresses(&self) -> Vec<Address>;
    fn get_iota_committee(&self) -> Committee;
    fn get_iota_committee_with_network_metadata(&self) -> CommitteeWithNetworkMetadata;
    /// The validators forming the committee for this epoch, in the order of
    /// the committee of the Move validator set,
    /// [`ValidatorSetV2::committee_members`]. Callers that need them ordered
    /// by authority key, such as when building a consensus committee, must
    /// sort them.
    ///
    /// [`ValidatorSetV2::committee_members`]: crate::iota_system_state::iota_system_state_inner_v2::ValidatorSetV2::committee_members
    fn committee_validators(&self) -> &[EpochStartValidatorInfoV1];
    fn get_validator_as_p2p_peers(&self, excluding_self: AuthorityName) -> Vec<PeerInfo>;
    fn get_authority_names_to_peer_ids(&self) -> HashMap<AuthorityName, PeerId>;
    fn get_authority_names_to_hostnames(&self) -> HashMap<AuthorityName, String>;
    fn get_active_validators(&self) -> Vec<AggregateAuthorityPublicKey>;
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

    /// The committee member index of each committee validator, by authority
    /// name: its position in the committee of the Move validator set,
    /// [`ValidatorSetV2::committee_members`], which is the order of
    /// [`EpochStartSystemStateTrait::committee_validators()`]. Move's
    /// `validator_set::advance_epoch()` reads the `scores` vector of the
    /// change-epoch transaction by this index. The authority index, which
    /// may differ, is [`Committee::authority_index()`].
    ///
    /// [`ValidatorSetV2::committee_members`]: crate::iota_system_state::iota_system_state_inner_v2::ValidatorSetV2::committee_members
    pub fn committee_member_indices(&self) -> HashMap<AuthorityName, usize> {
        self.committee_validators()
            .iter()
            .enumerate()
            .map(|(committee_member_index, validator)| {
                (validator.authority_name(), committee_member_index)
            })
            .collect()
    }

    /// Converts authority indices to committee member indices: entry `i` of
    /// the result is the committee member index of the member of `committee`
    /// with authority index `i`. `committee` names the member at each
    /// authority index, as [`Committee::authority_index()`] assigns them,
    /// and must be this epoch's committee. The result is a permutation of
    /// `0..committee.num_members()`.
    ///
    /// # Panics
    ///
    /// If the committee validators of this state do not cover the members of
    /// `committee` exactly once.
    pub fn authority_indices_to_committee_member_indices(
        &self,
        committee: &Committee,
    ) -> Vec<usize> {
        let committee_member_indices = self.committee_member_indices();
        assert_eq!(
            committee_member_indices.len(),
            committee.num_members(),
            "there should be one committee member index per committee member"
        );

        // `names()` lists the members in authority index order.
        committee
            .names()
            .map(|name| {
                *committee_member_indices
                    .get(name)
                    .expect("committee member should have a committee member index")
            })
            .collect()
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

    fn get_validator_addresses(&self) -> Vec<Address> {
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

    fn committee_validators(&self) -> &[EpochStartValidatorInfoV1] {
        &self.committee_validators
    }

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

    fn get_active_validators(&self) -> Vec<AggregateAuthorityPublicKey> {
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

    fn get_validator_addresses(&self) -> Vec<Address> {
        self.v1.get_validator_addresses()
    }

    fn get_iota_committee_with_network_metadata(&self) -> CommitteeWithNetworkMetadata {
        self.v1.get_iota_committee_with_network_metadata()
    }

    fn get_iota_committee(&self) -> Committee {
        self.v1.get_iota_committee()
    }

    fn committee_validators(&self) -> &[EpochStartValidatorInfoV1] {
        self.v1.committee_validators()
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

    fn get_active_validators(&self) -> Vec<AggregateAuthorityPublicKey> {
        self.active_validators
            .iter()
            .map(|validator| validator.authority_pubkey.clone())
            .collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct EpochStartValidatorInfoV1 {
    pub iota_address: Address,
    pub authority_pubkey: AggregateAuthorityPublicKey,
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
    use iota_multiaddr::Multiaddr;
    use iota_protocol_config::ProtocolVersion;
    use iota_sdk_types::Address;
    use rand08::thread_rng;

    use crate::{
        crypto::{AuthorityKeyPair, NetworkKeyPair, get_key_pair},
        iota_system_state::epoch_start_iota_system_state::{
            EpochStartSystemState, EpochStartSystemStateTrait, EpochStartValidatorInfoV1,
        },
    };

    #[test]
    fn test_epoch_start_system_state_versioning() {
        // Create test validators
        let (iota_address1, authority_key1): (Address, AuthorityKeyPair) = get_key_pair();
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

        let (iota_address2, authority_key2): (Address, AuthorityKeyPair) = get_key_pair();
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

    /// Committee validators for a test, `count` of them with equal voting
    /// power, sorted by authority name, which is their authority index order.
    fn committee_validators_in_authority_index_order(
        count: usize,
    ) -> Vec<EpochStartValidatorInfoV1> {
        let mut validators: Vec<EpochStartValidatorInfoV1> = (0..count)
            .map(|i| {
                let (iota_address, authority_key): (Address, AuthorityKeyPair) = get_key_pair();
                let network_key = NetworkKeyPair::generate(&mut thread_rng());
                EpochStartValidatorInfoV1 {
                    iota_address,
                    authority_pubkey: authority_key.public().clone(),
                    network_pubkey: network_key.public().clone(),
                    protocol_pubkey: network_key.public().clone(),
                    iota_net_address: Multiaddr::empty(),
                    p2p_address: Multiaddr::empty(),
                    primary_address: Multiaddr::empty(),
                    voting_power: 10_000 / count as u64,
                    hostname: format!("validator-{i}"),
                }
            })
            .collect();
        validators.sort_by_key(|validator| validator.authority_name());

        validators
    }

    /// An epoch start state whose committee is `committee_validators`, stored
    /// in that order.
    fn epoch_start_state_with_committee(
        committee_validators: Vec<EpochStartValidatorInfoV1>,
    ) -> EpochStartSystemState {
        EpochStartSystemState::new_v1(
            1,
            ProtocolVersion::MAX.as_u64(),
            0,
            false,
            0,
            0,
            committee_validators,
        )
    }

    /// `authority_indices_to_committee_member_indices()` maps each authority
    /// index to the committee member index of the same validator, for the
    /// three orders `committee_members` can have: the authority index order of
    /// a genesis committee, the reverse order a seat cap with equal stakes
    /// produces, and a validator appended after a join.
    #[test]
    fn committee_member_indices_follow_the_committee_members_order() {
        let in_authority_index_order = committee_validators_in_authority_index_order(4);
        let committee =
            epoch_start_state_with_committee(in_authority_index_order.clone()).get_iota_committee();

        // A genesis committee with a seat for everyone stores its validators
        // in authority index order.
        assert_eq!(
            epoch_start_state_with_committee(in_authority_index_order.clone())
                .authority_indices_to_committee_member_indices(&committee),
            vec![0, 1, 2, 3],
            "genesis committee"
        );

        // Equal stakes picked by `take_top_n` store them in the reverse order.
        let mut reversed = in_authority_index_order.clone();
        reversed.reverse();
        assert_eq!(
            epoch_start_state_with_committee(reversed)
                .authority_indices_to_committee_member_indices(&committee),
            vec![3, 2, 1, 0],
            "capped committee with equal stakes"
        );

        // A validator that joined after genesis is stored last whatever its
        // authority index.
        let mut joined_last = in_authority_index_order;
        let newcomer = joined_last.remove(0);
        joined_last.push(newcomer);
        assert_eq!(
            epoch_start_state_with_committee(joined_last)
                .authority_indices_to_committee_member_indices(&committee),
            vec![3, 0, 1, 2],
            "validator joined after genesis"
        );
    }

    /// `authority_indices_to_committee_member_indices()` panics when a member
    /// of `committee` has no committee member index in the state: here the
    /// state lacks the first validator.
    #[test]
    #[should_panic(expected = "there should be one committee member index per committee member")]
    fn committee_member_indices_must_cover_the_committee() {
        let validators = committee_validators_in_authority_index_order(4);
        let committee = epoch_start_state_with_committee(validators.clone()).get_iota_committee();
        epoch_start_state_with_committee(validators[1..].to_vec())
            .authority_indices_to_committee_member_indices(&committee);
    }
}
