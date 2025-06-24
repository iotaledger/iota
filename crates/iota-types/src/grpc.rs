// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC-specific versioned types for forward compatibility.
//!
//! These types provide versioning for gRPC streaming while positioning
//! for future core type evolution. When core checkpoint types themselves
//! need versioning, these wrappers will evolve naturally.

use serde::{Deserialize, Serialize};

/// Forward-compatible versioned checkpoint data for gRPC streaming.
/// Designed to support future core type evolution.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CheckpointData {
    V1(crate::full_checkpoint_content::CheckpointData),
}

/// Forward-compatible versioned checkpoint summary for gRPC streaming.
/// Designed to support future core type evolution.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum CertifiedCheckpointSummary {
    V1(crate::messages_checkpoint::CertifiedCheckpointSummary),
}

impl From<crate::full_checkpoint_content::CheckpointData> for CheckpointData {
    fn from(data: crate::full_checkpoint_content::CheckpointData) -> Self {
        Self::V1(data)
    }
}

impl From<crate::messages_checkpoint::CertifiedCheckpointSummary> for CertifiedCheckpointSummary {
    fn from(summary: crate::messages_checkpoint::CertifiedCheckpointSummary) -> Self {
        Self::V1(summary)
    }
}

impl CheckpointData {
    /// Extract the V1 checkpoint data, returning None for unknown versions
    pub fn into_v1(self) -> Option<crate::full_checkpoint_content::CheckpointData> {
        match self {
            Self::V1(data) => Some(data),
        }
    }

    /// Get a reference to the V1 checkpoint data, returning None for unknown
    /// versions
    pub fn as_v1(&self) -> Option<&crate::full_checkpoint_content::CheckpointData> {
        match self {
            Self::V1(data) => Some(data),
        }
    }

    /// Get the sequence number regardless of version
    pub fn sequence_number(&self) -> u64 {
        match self {
            Self::V1(data) => data.checkpoint_summary.sequence_number,
        }
    }
}

impl CertifiedCheckpointSummary {
    /// Extract the V1 checkpoint summary, returning None for unknown versions
    pub fn into_v1(self) -> Option<crate::messages_checkpoint::CertifiedCheckpointSummary> {
        match self {
            Self::V1(summary) => Some(summary),
        }
    }

    /// Get a reference to the V1 checkpoint summary, returning None for unknown
    /// versions
    pub fn as_v1(&self) -> Option<&crate::messages_checkpoint::CertifiedCheckpointSummary> {
        match self {
            Self::V1(summary) => Some(summary),
        }
    }

    /// Get the sequence number regardless of version
    pub fn sequence_number(&self) -> u64 {
        match self {
            Self::V1(summary) => summary.data().sequence_number,
        }
    }
}
