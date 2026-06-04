// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Debug, gas-profiling, and tracing configuration plus the artifacts a run
//! captures.
//!
//! [`DebugConfig`] is the user-facing input surface — independently toggle
//! `debug::print` capture, the Move VM gas profiler, and instruction tracing.
//! A run returns the matching [`DebugArtifacts`].

use std::path::PathBuf;

use move_trace_format::format::MoveTrace;

/// User-facing configuration for a single debug-enabled run.
///
/// The [`Default`] disables all debug capture and matches a plain
/// `ExecuteOptions` with no debug config.
#[derive(Debug, Default, Clone)]
pub struct DebugConfig {
    /// Capture Move `debug::print` output into [`DebugArtifacts::prints`].
    /// When `false`, prints go to stdout and [`DebugArtifacts::prints`] is
    /// empty.
    pub capture_debug_prints: bool,
    /// Enable the Move VM gas profiler and choose where the Speedscope JSON
    /// ends up.
    pub profile: Option<ProfileSink>,
    /// Enable instruction-level execution tracing. The resulting
    /// [`MoveTrace`] is returned in [`DebugArtifacts::trace`].
    pub trace: bool,
}

impl DebugConfig {
    /// `true` if any debug capture is enabled.
    pub(crate) fn any_enabled(&self) -> bool {
        self.capture_debug_prints || self.profile.is_some() || self.trace
    }
}

/// Where to write the Speedscope-format gas profile.
#[derive(Debug, Clone)]
pub enum ProfileSink {
    /// Write the profile JSON to the given path on disk (forwarded directly to
    /// the Move VM profiler).
    Path(PathBuf),
    /// Write the profile to a temporary location and read its bytes back into
    /// [`ProfileOutput::Json`] after execution.
    Capture,
}

/// The resulting gas profile: either a filesystem path the VM wrote to or the
/// raw JSON bytes read back after execution.
#[derive(Debug)]
pub enum ProfileOutput {
    Path(PathBuf),
    Json(Vec<u8>),
}

/// One captured Move `debug::print` line.
#[derive(Debug, Clone)]
pub struct DebugPrint {
    /// The text the Move code printed.
    pub message: String,
}

/// Artifacts captured from a run. Each field is populated if and only if the
/// matching [`DebugConfig`] toggle was enabled.
#[derive(Default)]
#[non_exhaustive]
pub struct DebugArtifacts {
    /// Captured `debug::print` lines. Empty unless
    /// [`DebugConfig::capture_debug_prints`] was set.
    pub prints: Vec<DebugPrint>,
    /// Gas profile output, if [`DebugConfig::profile`] was set.
    pub profile: Option<ProfileOutput>,
    /// Instruction-level execution trace, if [`DebugConfig::trace`] was set.
    pub trace: Option<MoveTrace>,
}
