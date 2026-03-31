// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

include!("../../../generated/iota.grpc.v1.epoch.rs");
include!("../../../generated/iota.grpc.v1.epoch.field_info.rs");
include!("../../../generated/iota.grpc.v1.epoch.accessors.rs");

use tap::Pipe;

use crate::{proto::TryFromProtoError, v1::bcs::BcsData};

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
    /// Get the epoch ID.
    ///
    /// Requires `epoch` in the read_mask.
    pub fn epoch_id(&self) -> Result<iota_sdk_types::EpochId, TryFromProtoError> {
        self.epoch
            .ok_or_else(|| TryFromProtoError::missing(Self::EPOCH_FIELD.name))
    }

    /// Deserialize the validator committee.
    ///
    /// Requires `committee` in the read_mask.
    pub fn committee(&self) -> Result<iota_sdk_types::ValidatorCommittee, TryFromProtoError> {
        match &self.committee {
            Some(committee) => Ok(committee.try_into()?),
            None => Err(TryFromProtoError::missing(Self::COMMITTEE_FIELD.name)),
        }
    }

    /// Get the raw BCS-encoded system state bytes.
    ///
    /// This is a snapshot of IOTA's SystemState
    /// (`0x3::iota_system::SystemState`) at the beginning of the epoch (for
    /// past epochs) or the current state (for the current epoch).
    ///
    /// Requires `bcs_system_state` in the read_mask.
    // TODO: Implement when IotaSystemState type is available in iota-sdk-types.
    // Use `system_state_bcs()` for raw bytes access in the meantime.
    // See https://github.com/iotaledger/iota/issues/10077
    //
    // pub fn system_state(&self) -> Result<iota_sdk_types::IotaSystemState,
    // TryFromProtoError> {     ...
    // }
    pub fn system_state_bcs(&self) -> Result<&[u8], TryFromProtoError> {
        self.bcs_system_state
            .as_ref()
            .map(BcsData::as_bytes)
            .ok_or_else(|| TryFromProtoError::missing(Self::BCS_SYSTEM_STATE_FIELD.name))
    }

    /// Get the first checkpoint sequence number in this epoch.
    ///
    /// Requires `first_checkpoint` in the read_mask.
    pub fn first_checkpoint_sequence_number(
        &self,
    ) -> Result<iota_sdk_types::CheckpointSequenceNumber, TryFromProtoError> {
        self.first_checkpoint
            .ok_or_else(|| TryFromProtoError::missing(Self::FIRST_CHECKPOINT_FIELD.name))
    }

    /// Get the last checkpoint sequence number in this epoch.
    ///
    /// Returns `Ok(None)` for the current in-progress epoch (field not yet
    /// set).
    pub fn last_checkpoint_sequence_number(
        &self,
    ) -> Result<Option<iota_sdk_types::CheckpointSequenceNumber>, TryFromProtoError> {
        Ok(self.last_checkpoint)
    }

    /// Get the epoch start time in milliseconds.
    ///
    /// Requires `start` in the read_mask.
    pub fn start_ms(&self) -> Result<iota_sdk_types::CheckpointTimestamp, TryFromProtoError> {
        let ts = self
            .start
            .ok_or_else(|| TryFromProtoError::missing(Self::START_FIELD.name))?;
        crate::proto::proto_to_timestamp_ms(ts).map_err(|e| e.nested(Self::START_FIELD.name))
    }

    /// Get the epoch end time in milliseconds.
    ///
    /// Returns `Ok(None)` for the current in-progress epoch (field not yet
    /// set).
    pub fn end_ms(&self) -> Result<Option<iota_sdk_types::CheckpointTimestamp>, TryFromProtoError> {
        self.end
            .map(|ts| {
                crate::proto::proto_to_timestamp_ms(ts).map_err(|e| e.nested(Self::END_FIELD.name))
            })
            .transpose()
    }

    /// Get the reference gas price in NANOS.
    ///
    /// Requires `reference_gas_price` in the read_mask.
    pub fn gas_price(&self) -> Result<u64, TryFromProtoError> {
        self.reference_gas_price
            .ok_or_else(|| TryFromProtoError::missing(Self::REFERENCE_GAS_PRICE_FIELD.name))
    }

    /// Get the protocol configuration for this epoch.
    ///
    /// Requires `protocol_config` in the read_mask.
    pub fn protocol_config(&self) -> Result<&ProtocolConfig, TryFromProtoError> {
        self.protocol_config
            .as_ref()
            .ok_or_else(|| TryFromProtoError::missing(Self::PROTOCOL_CONFIG_FIELD.name))
    }
}

// ProtocolConfig
//

impl ProtocolConfig {
    /// Get the protocol version number.
    ///
    /// Requires `protocol_version` in the read_mask.
    pub fn version(&self) -> Result<iota_sdk_types::ProtocolVersion, TryFromProtoError> {
        self.protocol_version
            .ok_or_else(|| TryFromProtoError::missing(Self::PROTOCOL_VERSION_FIELD.name))
    }

    /// Get the feature flags map.
    ///
    /// Requires `feature_flags` in the read_mask to return all flags.
    /// Use `feature_flags.{name}` to request only specific named flags.
    pub fn feature_flags(
        &self,
    ) -> Result<&std::collections::BTreeMap<String, bool>, TryFromProtoError> {
        self.feature_flags
            .as_ref()
            .map(|f| &f.flags)
            .ok_or_else(|| TryFromProtoError::missing(Self::FEATURE_FLAGS_FIELD.name))
    }

    /// Get the protocol attributes map.
    ///
    /// Requires `attributes` in the read_mask to return all attributes.
    /// Use `attributes.{name}` to request only specific named attributes.
    pub fn attributes(
        &self,
    ) -> Result<&std::collections::BTreeMap<String, String>, TryFromProtoError> {
        self.attributes
            .as_ref()
            .map(|a| &a.attributes)
            .ok_or_else(|| TryFromProtoError::missing(Self::ATTRIBUTES_FIELD.name))
    }
}
