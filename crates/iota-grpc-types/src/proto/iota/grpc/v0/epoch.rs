// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v0.epoch.rs");
include!("../../../generated/iota.grpc.v0.epoch.field_info.rs");

use tap::Pipe;

use crate::{proto::TryFromProtoError, v0::bcs::BcsData};

// ValidatorCommitteeMember
//

impl From<iota_sdk_types::ValidatorCommitteeMember> for ValidatorCommitteeMember {
    fn from(value: iota_sdk_types::ValidatorCommitteeMember) -> Self {
        Self {
            public_key: Some(value.public_key.as_bytes().to_vec().into()),
            weight: Some(value.stake),
        }
    }
}

impl TryFrom<&ValidatorCommitteeMember> for iota_sdk_types::ValidatorCommitteeMember {
    type Error = TryFromProtoError;

    fn try_from(
        ValidatorCommitteeMember { public_key, weight }: &ValidatorCommitteeMember,
    ) -> Result<Self, Self::Error> {
        let public_key = public_key
            .as_ref()
            .ok_or_else(|| {
                TryFromProtoError::missing(ValidatorCommitteeMember::PUBLIC_KEY_FIELD.name)
            })?
            .as_ref()
            .pipe(iota_sdk_types::Bls12381PublicKey::from_bytes)
            .map_err(|e| {
                TryFromProtoError::invalid(ValidatorCommitteeMember::PUBLIC_KEY_FIELD, e)
            })?;

        let stake = weight.ok_or_else(|| {
            TryFromProtoError::missing(ValidatorCommitteeMember::WEIGHT_FIELD.name)
        })?;
        Ok(Self { public_key, stake })
    }
}

// ValidatorCommittee
//

impl From<iota_sdk_types::ValidatorCommittee> for ValidatorCommittee {
    fn from(value: iota_sdk_types::ValidatorCommittee) -> Self {
        Self {
            epoch: Some(value.epoch),
            members: Some(ValidatorCommitteeMembers {
                members: value.members.into_iter().map(Into::into).collect(),
            }),
        }
    }
}

impl TryFrom<&ValidatorCommittee> for iota_sdk_types::ValidatorCommittee {
    type Error = TryFromProtoError;

    fn try_from(value: &ValidatorCommittee) -> Result<Self, Self::Error> {
        let epoch = value
            .epoch
            .ok_or_else(|| TryFromProtoError::missing(ValidatorCommittee::EPOCH_FIELD.name))?;
        let members = value
            .members
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(ValidatorCommittee::MEMBERS_FIELD.name))?;
        Ok(Self {
            epoch,
            members: members
                .members
                .iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
        })
    }
}

impl Epoch {
    /// Get the epoch number.
    pub fn epoch_number(&self) -> Option<u64> {
        self.epoch
    }

    /// Deserialize the validator committee.
    pub fn committee(&self) -> Result<iota_sdk_types::ValidatorCommittee, TryFromProtoError> {
        match &self.committee {
            Some(committee) => Ok(committee.try_into()?),
            None => Err(TryFromProtoError::missing(Self::COMMITTEE_FIELD.name)),
        }
    }

    /// Get the first checkpoint sequence number in this epoch.
    pub fn first_checkpoint_sequence_number(&self) -> Option<u64> {
        self.first_checkpoint
    }

    /// Get the last checkpoint sequence number in this epoch.
    pub fn last_checkpoint_sequence_number(&self) -> Option<u64> {
        self.last_checkpoint
    }

    /// Get the epoch start time in milliseconds.
    pub fn start_ms(&self) -> Result<Option<u64>, TryFromProtoError> {
        self.start
            .map(|ts| {
                crate::proto::proto_to_timestamp_ms(ts)
                    .map_err(|e| e.nested(Self::START_FIELD.name))
            })
            .transpose()
    }

    /// Get the epoch end time in milliseconds.
    pub fn end_ms(&self) -> Result<Option<u64>, TryFromProtoError> {
        self.end
            .map(|ts| {
                crate::proto::proto_to_timestamp_ms(ts).map_err(|e| e.nested(Self::END_FIELD.name))
            })
            .transpose()
    }

    /// Get the reference gas price in NANOS.
    pub fn gas_price(&self) -> Option<u64> {
        self.reference_gas_price
    }

    /// Get the raw BCS-encoded system state bytes.
    ///
    /// This is a snapshot of IOTA's SystemState
    /// (`0x3::iota_system::SystemState`) at the beginning of the epoch (for
    /// past epochs) or the current state (for the current epoch).
    pub fn system_state_bcs(&self) -> Option<&[u8]> {
        self.bcs_system_state.as_ref().map(BcsData::as_bytes)
    }

    /// Get the protocol version number.
    pub fn protocol_version(&self) -> Option<u64> {
        self.protocol_config
            .as_ref()
            .and_then(|c| c.protocol_version)
    }

    /// Get the feature flags map.
    pub fn feature_flags(&self) -> Option<&std::collections::BTreeMap<String, bool>> {
        self.protocol_config
            .as_ref()
            .and_then(|c| c.feature_flags.as_ref())
            .map(|f| &f.flags)
    }

    /// Get the protocol attributes map.
    pub fn protocol_attributes(&self) -> Option<&std::collections::BTreeMap<String, String>> {
        self.protocol_config
            .as_ref()
            .and_then(|c| c.attributes.as_ref())
            .map(|a| &a.attributes)
    }

    // TODO: Implement when IotaSystemState type is available in iota-sdk-types.
    // Use `system_state_bcs()` for raw bytes access in the meantime.
    //
    // pub fn system_state(&self) -> Result<iota_sdk_types::IotaSystemState,
    // TryFromProtoError> {     ...
    // }

    // TODO: Implement when ProtocolConfig conversion is available.
    // Use `protocol_version()`, `feature_flags()`, and `protocol_attributes()`
    // for individual field access in the meantime.
    //
    // pub fn protocol_config(&self) -> Result<iota_protocol_config::ProtocolConfig,
    // TryFromProtoError> {     ...
    // }
}

// ProtocolConfig
//

impl ProtocolConfig {
    /// Get the protocol version number.
    pub fn version(&self) -> Option<u64> {
        self.protocol_version
    }

    /// Get the feature flags map.
    pub fn flags(&self) -> Option<&std::collections::BTreeMap<String, bool>> {
        self.feature_flags.as_ref().map(|f| &f.flags)
    }

    /// Get the protocol attributes map.
    pub fn attrs(&self) -> Option<&std::collections::BTreeMap<String, String>> {
        self.attributes.as_ref().map(|a| &a.attributes)
    }

    // TODO: Implement when ProtocolConfig conversion is available.
    // Use `version()`, `flags()`, and `attrs()` for individual field access in the
    // meantime.
    //
    // pub fn to_protocol_config(&self) ->
    // Result<iota_protocol_config::ProtocolConfig, TryFromProtoError> {     ...
    // }
}

// ProtocolFeatureFlags
//

impl ProtocolFeatureFlags {
    /// Get the feature flags map.
    pub fn feature_flags(&self) -> &std::collections::BTreeMap<String, bool> {
        &self.flags
    }
}

// ProtocolAttributes
//

impl ProtocolAttributes {
    /// Get the attributes map.
    pub fn protocol_attributes(&self) -> &std::collections::BTreeMap<String, String> {
        &self.attributes
    }
}

// ValidatorCommitteeMembers
//

impl ValidatorCommitteeMembers {
    /// Deserialize all committee members.
    pub fn committee_members(
        &self,
    ) -> Result<Vec<iota_sdk_types::ValidatorCommitteeMember>, TryFromProtoError> {
        self.members
            .iter()
            .enumerate()
            .map(|(i, m)| {
                m.committee_member()
                    .map_err(|e| e.nested_at(Self::MEMBERS_FIELD.name, i))
            })
            .collect()
    }
}

// ValidatorCommittee
//

impl ValidatorCommittee {
    /// Deserialize the validator committee.
    pub fn validator_committee(
        &self,
    ) -> Result<iota_sdk_types::ValidatorCommittee, TryFromProtoError> {
        self.try_into()
    }

    /// Get the epoch number.
    pub fn epoch_number(&self) -> Option<u64> {
        self.epoch
    }

    /// Deserialize all committee members.
    pub fn committee_members(
        &self,
    ) -> Result<Vec<iota_sdk_types::ValidatorCommitteeMember>, TryFromProtoError> {
        match &self.members {
            Some(members) => members
                .committee_members()
                .map_err(|e| e.nested(Self::MEMBERS_FIELD.name)),
            None => Err(TryFromProtoError::missing(Self::MEMBERS_FIELD.name)),
        }
    }
}

// ValidatorCommitteeMember
//

impl ValidatorCommitteeMember {
    /// Deserialize the committee member.
    pub fn committee_member(
        &self,
    ) -> Result<iota_sdk_types::ValidatorCommitteeMember, TryFromProtoError> {
        self.try_into()
    }

    /// Get the BLS public key.
    pub fn bls_public_key(
        &self,
    ) -> Result<Option<iota_sdk_types::Bls12381PublicKey>, TryFromProtoError> {
        self.public_key
            .as_ref()
            .map(|pk| {
                iota_sdk_types::Bls12381PublicKey::from_bytes(pk.as_ref())
                    .map_err(|e| TryFromProtoError::invalid(Self::PUBLIC_KEY_FIELD, e))
            })
            .transpose()
    }

    /// Get the raw public key bytes.
    pub fn public_key_bytes(&self) -> Option<&[u8]> {
        self.public_key.as_ref().map(|pk| pk.as_ref())
    }

    /// Get the voting weight (stake).
    pub fn voting_weight(&self) -> Option<u64> {
        self.weight
    }
}
