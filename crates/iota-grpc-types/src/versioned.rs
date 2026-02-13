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
    pub fn into_inner(self) -> iota_sdk_types::Object {
        match self {
            Self::V1(inner) => inner,
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
    pub fn into_inner(self) -> iota_sdk_types::Event {
        match self {
            Self::V1(inner) => inner,
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
    pub fn into_inner(self) -> iota_sdk_types::CheckpointSummary {
        match self {
            Self::V1(inner) => inner,
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
    pub fn into_inner(self) -> iota_sdk_types::ValidatorAggregatedSignature {
        match self {
            Self::V1(inner) => inner,
        }
    }
}
