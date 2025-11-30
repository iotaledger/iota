// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

/// Response from the build cache server when checking if binaries exist.
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildCacheResponse {
    pub commit: String,
    pub cpu_target: String,
    pub available: bool,
    pub toolchain: Option<String>,
    pub features: Vec<String>,
    pub binaries: Vec<String>,
}

/// Request to build binaries for a specific commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildRequest {
    pub commit: String,
    /// CPU target architecture (e.g., "native", "x86-64-v3", "skylake")
    pub cpu_target: String,
    /// Optional rust toolchain override (e.g., "stable", "nightly", "1.75.0")
    /// If "stable" is passed, it's treated as default and ignored in cache key
    #[serde(default)]
    pub toolchain: Option<String>,
    /// Optional feature flags to enable during build (will be sorted for cache
    /// key)
    #[serde(default)]
    pub features: Vec<String>,
    /// List of binary names to build
    pub binaries: Vec<String>,
}

/// Response from the build endpoint with resolved commit hash.
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildResponse {
    /// The resolved commit hash (even if a branch/tag was provided in the
    /// request)
    pub resolved_commit: String,
    pub cpu_target: String,
    pub toolchain: Option<String>,
    pub features: Vec<String>,
    pub binaries: Vec<String>,
    pub message: String,
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
    pub toolchain: Option<String>,
    pub features: Vec<String>,
    pub binaries: Vec<String>,
    pub status: BuildStatus,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}
