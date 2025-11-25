// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

/// Response from the build cache server when checking if binaries exist.
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildCacheResponse {
    pub commit: String,
    pub cpu_target: String,
    pub available: bool,
    pub binaries: Vec<String>,
}

/// Request to build binaries for a specific commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRequest {
    pub commit: String,
    /// CPU target architecture (e.g., "native", "x86-64-v3", "skylake")
    pub cpu_target: String,
    pub binaries: Vec<String>,
}

/// Status of a build job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuildStatus {
    Queued,
    Building,
    Success,
    Failed(String),
}

/// A build job in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildJob {
    pub commit: String,
    pub cpu_target: String,
    pub binaries: Vec<String>,
    pub status: BuildStatus,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}
