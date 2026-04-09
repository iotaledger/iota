// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Versioned BCS envelopes for types without native BCS discriminants.

use serde::{Deserialize, Serialize};

/// Versioned BCS envelope for [`iota_sdk_types::Object`].
#[derive(Serialize, Deserialize)]
#[non_exhaustive]
pub enum VersionedObject {
    V1(iota_sdk_types::Object),
}

impl VersionedObject {
    pub fn try_v1(&self) -> Option<&iota_sdk_types::Object> {
        match self {
            Self::V1(inner) => Some(inner),
        }
    }

    pub fn try_into_v1(self) -> Result<iota_sdk_types::Object, Self> {
        match self {
            Self::V1(inner) => Ok(inner),
        }
    }
}

/// Versioned BCS envelope for [`iota_sdk_types::Event`].
#[derive(Serialize, Deserialize)]
#[non_exhaustive]
pub enum VersionedEvent {
    V1(iota_sdk_types::Event),
}

impl VersionedEvent {
    pub fn try_v1(&self) -> Option<&iota_sdk_types::Event> {
        match self {
            Self::V1(inner) => Some(inner),
        }
    }

    pub fn try_into_v1(self) -> Result<iota_sdk_types::Event, Self> {
        match self {
            Self::V1(inner) => Ok(inner),
        }
    }
}

/// Versioned BCS envelope for [`iota_sdk_types::CheckpointSummary`].
#[derive(Serialize, Deserialize)]
#[non_exhaustive]
pub enum VersionedCheckpointSummary {
    V1(iota_sdk_types::CheckpointSummary),
}

impl VersionedCheckpointSummary {
    pub fn try_v1(&self) -> Option<&iota_sdk_types::CheckpointSummary> {
        match self {
            Self::V1(inner) => Some(inner),
        }
    }

    pub fn try_into_v1(self) -> Result<iota_sdk_types::CheckpointSummary, Box<Self>> {
        match self {
            Self::V1(inner) => Ok(inner),
        }
    }
}

/// Versioned BCS envelope for
/// [`iota_sdk_types::ValidatorAggregatedSignature`].
#[derive(Serialize, Deserialize)]
#[non_exhaustive]
pub enum VersionedValidatorAggregatedSignature {
    V1(iota_sdk_types::ValidatorAggregatedSignature),
}

impl VersionedValidatorAggregatedSignature {
    pub fn try_v1(&self) -> Option<&iota_sdk_types::ValidatorAggregatedSignature> {
        match self {
            Self::V1(inner) => Some(inner),
        }
    }

    pub fn try_into_v1(self) -> Result<iota_sdk_types::ValidatorAggregatedSignature, Self> {
        match self {
            Self::V1(inner) => Ok(inner),
        }
    }
}
