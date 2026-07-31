// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Debug capture: [`DebugConfig`] toggles the Move VM gas profiler and
//! instruction tracing independently; a run returns the matching
//! [`DebugArtifacts`].

use std::path::PathBuf;

use move_trace_format::format::{TraceEvent, TraceVersion};

/// Configuration for a single debug-enabled run.
///
/// The [`Default`] disables all capture.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct DebugConfig {
    /// Enable the Move VM gas profiler and choose where the Speedscope JSON
    /// ends up.
    pub profile: Option<ProfileSink>,
    /// Enable instruction-level execution tracing. Only captured on the
    /// `MoveAuthenticator` path; see [`with_tracing`](Self::with_tracing).
    pub trace: bool,
}

impl DebugConfig {
    /// Enable the gas profiler, writing to `sink`.
    ///
    /// Requires the crate's `tracing` feature; without it the run captures no
    /// profile and [`DebugArtifacts::profile`] stays `None`.
    #[must_use]
    pub fn with_profiling(mut self, sink: ProfileSink) -> Self {
        self.profile = Some(sink);
        self
    }

    /// Enable instruction-level execution tracing.
    ///
    /// Requires the crate's `tracing` feature; without it
    /// [`DebugArtifacts::trace`] stays `None`.
    ///
    /// Tracing is only captured for signed transactions that authorize via a
    /// `MoveAuthenticator`. The unsigned
    /// [`LocalVm::execute`](crate::LocalVm::execute) path and
    /// standard-signature transactions run through the engine's dev-inspect
    /// entry point, which accepts no trace builder; for those,
    /// [`DebugArtifacts::trace`] stays `None` even when tracing was
    /// requested.
    #[must_use]
    pub fn with_tracing(mut self) -> Self {
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
    /// Write the merged profile JSON to the given path on disk. A path that
    /// can't be written fails the run with a
    /// [`VmSdkError`](crate::VmSdkError).
    Path(PathBuf),
    /// Write the profile to a temporary location and read its bytes back into
    /// [`ProfileOutput::Json`] after execution. (Native only — on wasm32 the
    /// filesystem is unavailable and this behaves as a no-op.)
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

/// An instruction-level execution trace: the events the Move VM emitted, in
/// order, plus the same trace in the format the Move trace debugger reads.
#[derive(Clone)]
pub struct ExecutionTrace {
    /// Version of the trace format the events were captured with.
    pub version: TraceVersion,
    /// Events in the order the VM emitted them.
    pub events: Vec<TraceEvent>,
    bytes: Vec<u8>,
}

impl ExecutionTrace {
    pub(crate) fn new(version: TraceVersion, events: Vec<TraceEvent>, bytes: Vec<u8>) -> Self {
        Self {
            version,
            events,
            bytes,
        }
    }

    /// The trace as the VM encoded it: a version header line followed by one
    /// JSON-encoded event per line, zstd-compressed. Write it to a file with
    /// the `json.zst` extension to open the run in the Move trace debugger.
    ///
    /// (On wasm32 the Move trace format is not compressed, so these are plain
    /// line-delimited JSON bytes.)
    pub fn trace_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Take the encoded trace, as described on
    /// [`trace_bytes`](Self::trace_bytes).
    pub fn into_trace_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

// Summarised rather than derived: a trace of a real transaction runs to
// hundreds of thousands of events, so printing them all buries whatever else
// the caller was inspecting. Reach for `events` to see them.
impl std::fmt::Debug for ExecutionTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionTrace")
            .field("version", &self.version)
            .field("event_count", &self.events.len())
            .field("trace_bytes_len", &self.bytes.len())
            .finish()
    }
}

/// Artifacts captured from a run, present when any [`DebugConfig`] toggle was
/// enabled.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct DebugArtifacts {
    /// Gas profile output, if [`DebugConfig::profile`] was set.
    pub profile: Option<ProfileOutput>,
    /// Instruction-level execution trace. `None` unless the run went through
    /// the `MoveAuthenticator` path (see [`DebugConfig::with_tracing`]).
    pub trace: Option<ExecutionTrace>,
}
