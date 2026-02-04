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
    pub fn epoch_number(&self) -> Result<u64, TryFromProtoError> {
        self.epoch
            .ok_or_else(|| TryFromProtoError::missing(Self::EPOCH_FIELD.name))
    }

    /// Deserialize the validator committee.
    pub fn committee(&self) -> Result<iota_sdk_types::ValidatorCommittee, TryFromProtoError> {
        match &self.committee {
            Some(committee) => Ok(committee.try_into()?),
            None => Err(TryFromProtoError::missing(Self::COMMITTEE_FIELD.name)),
        }
    }

    /// Get the first checkpoint sequence number in this epoch.
    pub fn first_checkpoint_sequence_number(&self) -> Result<u64, TryFromProtoError> {
        self.first_checkpoint
            .ok_or_else(|| TryFromProtoError::missing(Self::FIRST_CHECKPOINT_FIELD.name))
    }

    /// Get the last checkpoint sequence number in this epoch.
    ///
    /// Returns `Ok(None)` for the current in-progress epoch (field not yet
    /// set).
    pub fn last_checkpoint_sequence_number(&self) -> Result<Option<u64>, TryFromProtoError> {
        Ok(self.last_checkpoint)
    }

    /// Get the epoch start time in milliseconds.
    pub fn start_ms(&self) -> Result<u64, TryFromProtoError> {
        let ts = self
            .start
            .ok_or_else(|| TryFromProtoError::missing(Self::START_FIELD.name))?;
        crate::proto::proto_to_timestamp_ms(ts).map_err(|e| e.nested(Self::START_FIELD.name))
    }

    /// Get the epoch end time in milliseconds.
    ///
    /// Returns `Ok(None)` for the current in-progress epoch (field not yet
    /// set).
    pub fn end_ms(&self) -> Result<Option<u64>, TryFromProtoError> {
        self.end
            .map(|ts| {
                crate::proto::proto_to_timestamp_ms(ts).map_err(|e| e.nested(Self::END_FIELD.name))
            })
            .transpose()
    }

    /// Get the reference gas price in NANOS.
    pub fn gas_price(&self) -> Result<u64, TryFromProtoError> {
        self.reference_gas_price
            .ok_or_else(|| TryFromProtoError::missing(Self::REFERENCE_GAS_PRICE_FIELD.name))
    }

    /// Get the raw BCS-encoded system state bytes.
    ///
    /// This is a snapshot of IOTA's SystemState
    /// (`0x3::iota_system::SystemState`) at the beginning of the epoch (for
    /// past epochs) or the current state (for the current epoch).
    pub fn system_state_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs_system_state
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_SYSTEM_STATE_FIELD.name))
    }

    /// Get the protocol version number.
    pub fn protocol_version(&self) -> Result<u64, TryFromProtoError> {
        self.protocol_config
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::PROTOCOL_CONFIG_FIELD.name))?
            .protocol_version
            .ok_or_else(|| {
                TryFromProtoError::missing(ProtocolConfig::PROTOCOL_VERSION_FIELD.name)
                    .nested(Self::PROTOCOL_CONFIG_FIELD.name)
            })
    }

    /// Get the feature flags map.
    pub fn feature_flags(
        &self,
    ) -> Result<&std::collections::BTreeMap<String, bool>, TryFromProtoError> {
        let config = self
            .protocol_config
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::PROTOCOL_CONFIG_FIELD.name))?;
        let flags = config.feature_flags.as_ref().ok_or_else(|| {
            TryFromProtoError::missing(ProtocolConfig::FEATURE_FLAGS_FIELD.name)
                .nested(Self::PROTOCOL_CONFIG_FIELD.name)
        })?;
        Ok(&flags.flags)
    }

    /// Get the protocol attributes map.
    pub fn protocol_attributes(
        &self,
    ) -> Result<&std::collections::BTreeMap<String, String>, TryFromProtoError> {
        let config = self
            .protocol_config
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::PROTOCOL_CONFIG_FIELD.name))?;
        let attrs = config.attributes.as_ref().ok_or_else(|| {
            TryFromProtoError::missing(ProtocolConfig::ATTRIBUTES_FIELD.name)
                .nested(Self::PROTOCOL_CONFIG_FIELD.name)
        })?;
        Ok(&attrs.attributes)
    }

    // TODO: Implement when IotaSystemState type is available in iota-sdk-types.
    // Use `system_state_bcs()` for raw bytes access in the meantime.
    // See https://github.com/iotaledger/iota/issues/10077
    //
    // pub fn system_state(&self) -> Result<iota_sdk_types::IotaSystemState,
    // TryFromProtoError> {     ...
    // }

    // TODO: Implement when ProtocolConfig conversion is available.
    // Use `protocol_version()`, `feature_flags()`, and `protocol_attributes()`
    // for individual field access in the meantime.
    // See https://github.com/iotaledger/iota/issues/10077
    //
    // pub fn protocol_config(&self) -> Result<iota_protocol_config::ProtocolConfig,
    // TryFromProtoError> {     ...
    // }
}

// ProtocolConfig
//

impl ProtocolConfig {
    /// Get the protocol version number.
    pub fn version(&self) -> Result<u64, TryFromProtoError> {
        self.protocol_version
            .ok_or_else(|| TryFromProtoError::missing(Self::PROTOCOL_VERSION_FIELD.name))
    }

    /// Get the feature flags map.
    pub fn flags(&self) -> Result<&std::collections::BTreeMap<String, bool>, TryFromProtoError> {
        self.feature_flags
            .as_ref()
            .map(|f| &f.flags)
            .ok_or_else(|| TryFromProtoError::missing(Self::FEATURE_FLAGS_FIELD.name))
    }

    /// Get the protocol attributes map.
    pub fn attrs(&self) -> Result<&std::collections::BTreeMap<String, String>, TryFromProtoError> {
        self.attributes
            .as_ref()
            .map(|a| &a.attributes)
            .ok_or_else(|| TryFromProtoError::missing(Self::ATTRIBUTES_FIELD.name))
    }

    // TODO: Implement when ProtocolConfig conversion is available.
    // Use `version()`, `flags()`, and `attrs()` for individual field access in the
    // meantime.
    // See https://github.com/iotaledger/iota/issues/10077
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
    pub fn epoch_number(&self) -> Result<u64, TryFromProtoError> {
        self.epoch
            .ok_or_else(|| TryFromProtoError::missing(Self::EPOCH_FIELD.name))
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
    pub fn bls_public_key(&self) -> Result<iota_sdk_types::Bls12381PublicKey, TryFromProtoError> {
        let pk = self
            .public_key
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::PUBLIC_KEY_FIELD.name))?;
        iota_sdk_types::Bls12381PublicKey::from_bytes(pk.as_ref())
            .map_err(|e| TryFromProtoError::invalid(Self::PUBLIC_KEY_FIELD, e))
    }

    /// Get the raw public key bytes.
    pub fn public_key_bytes(&self) -> Result<&[u8], TryFromProtoError> {
        self.public_key
            .as_ref()
            .map(|pk| pk.as_ref())
            .ok_or_else(|| TryFromProtoError::missing(Self::PUBLIC_KEY_FIELD.name))
    }

    /// Get the voting weight (stake).
    pub fn voting_weight(&self) -> Result<u64, TryFromProtoError> {
        self.weight
            .ok_or_else(|| TryFromProtoError::missing(Self::WEIGHT_FIELD.name))
    }
}
