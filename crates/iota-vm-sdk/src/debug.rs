// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Debug, gas-profiling, and tracing configuration plus the artifacts a run
//! captures.
//!
//! [`DebugConfig`] toggles the Move VM gas profiler and instruction tracing
//! independently; a run returns the matching [`DebugArtifacts`].

use std::path::PathBuf;

use move_trace_format::format::MoveTrace;

/// Configuration for a single debug-enabled run.
///
/// The [`Default`] disables all capture. Construct via [`Default`] and the
/// `with_*` builders.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct DebugConfig {
    /// Enable the Move VM gas profiler and choose where the Speedscope JSON
    /// ends up.
    pub profile: Option<ProfileSink>,
    /// Enable instruction-level execution tracing. Only captured on the
    /// `MoveAuthenticator` path; see [`with_trace`](Self::with_trace).
    pub trace: bool,
}

impl DebugConfig {
    /// Enable the gas profiler, writing to `sink`.
    #[must_use]
    pub fn with_profile(mut self, sink: ProfileSink) -> Self {
        self.profile = Some(sink);
        self
    }

    /// Enable instruction-level execution tracing.
    ///
    /// Tracing is only captured for signed transactions that authorize via a
    /// `MoveAuthenticator`. The unsigned
    /// [`LocalVm::execute`](crate::LocalVm::execute) path and
    /// standard-signature transactions run through the engine's dev-inspect
    /// entry point, which accepts no trace builder; for those,
    /// [`DebugArtifacts::trace`] stays `None` even when tracing was
    /// requested.
    #[must_use]
    pub fn with_trace(mut self) -> Self {
        self.trace = true;
        self
    }

    /// `true` if any debug capture is enabled.
    pub(crate) fn any_enabled(&self) -> bool {
        self.profile.is_some() || self.trace
    }
}

/// Where to write the Speedscope-format gas profile.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ProfileSink {
    /// Write the merged profile JSON to the given path on disk. If the path
    /// can't be written, the run yields no profile rather than an error.
    Path(PathBuf),
    /// Write the profile to a temporary location and read its bytes back into
    /// [`ProfileOutput::Json`] after execution.
    Capture,
}

/// The resulting gas profile: either a filesystem path the VM wrote to or the
/// raw JSON bytes read back after execution.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProfileOutput {
    /// Speedscope JSON written to this path (from [`ProfileSink::Path`]).
    Path(PathBuf),
    /// Merged Speedscope JSON bytes read back (from [`ProfileSink::Capture`]).
    Json(Vec<u8>),
}

/// Artifacts captured from a run, present when any [`DebugConfig`] toggle was
/// enabled.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct DebugArtifacts {
    /// Gas profile output, if [`DebugConfig::profile`] was set.
    pub profile: Option<ProfileOutput>,
    /// Instruction-level execution trace. `None` unless the run went through
    /// the `MoveAuthenticator` path (see [`DebugConfig::with_trace`]).
    pub trace: Option<MoveTrace>,
}
