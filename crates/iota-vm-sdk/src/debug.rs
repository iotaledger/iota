// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Debug capture: [`DebugConfig`] toggles the Move VM gas profiler and
//! instruction tracing independently; a run returns the matching
//! [`DebugArtifacts`].

use std::path::PathBuf;

use move_trace_format::format::{MoveTraceBuilder, MoveTraceReader, TraceEvent, TraceVersion};

use crate::error::TraceError;

/// Configuration for a single debug-enabled run.
///
/// The [`Default`] disables all capture.
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct DebugConfig {
    /// Enable the Move VM gas profiler and choose where the Speedscope JSON
    /// ends up.
    pub profile: Option<ProfileSink>,
    /// Enable instruction-level execution tracing. Not captured for every run;
    /// see [`with_tracing`](Self::with_tracing).
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
    /// Not captured for a plain PTB in
    /// [`ExecutionMode::DevInspect`](crate::ExecutionMode): a traced run uses
    /// the engine entry point that runs under full (non-dev-inspect) VM
    /// semantics, so dev-inspect's relaxed checks and tracing cannot be had
    /// together and the relaxed checks win —
    /// [`DebugArtifacts::trace`] stays `None`. A transaction authorizing via a
    /// `MoveAuthenticator` is traced in every mode. See
    /// [`ExecutionMode::supports_tracing`](crate::ExecutionMode::supports_tracing).
    ///
    /// A traced run reports no
    /// [`command_results`](crate::ExecutionResult::command_results) — the
    /// entry points that accept a trace builder do not collect them.
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

/// An instruction-level execution trace, in the format the Move trace debugger
/// reads. Decode its events with [`events`](Self::events), or hand the encoded
/// bytes to a file or another tool with [`bytes`](Self::bytes).
pub struct ExecutionTrace {
    version: TraceVersion,
    event_count: usize,
    bytes: Vec<u8>,
}

impl ExecutionTrace {
    /// Build from a finished [`MoveTraceBuilder`]: reads the version and event
    /// count while the builder is still alive (`into_trace` moves the trace
    /// out, and encoding consumes it), then encodes.
    pub(crate) fn from_builder(builder: MoveTraceBuilder) -> Self {
        let version = builder.trace.version;
        let event_count = builder.current_trace_offset();
        Self {
            version,
            event_count,
            bytes: builder.into_trace().into_compressed_json_bytes(),
        }
    }

    /// Version of the trace format the run was captured with.
    pub fn version(&self) -> TraceVersion {
        self.version
    }

    /// How many events the VM emitted. Counted during the run, so reading it
    /// decodes nothing.
    pub fn event_count(&self) -> usize {
        self.event_count
    }

    /// The trace as the VM encoded it: a version header line followed by one
    /// JSON-encoded event per line, zstd-compressed. Write it to a file with
    /// the `json.zst` extension to open the run in the Move trace debugger.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Take the encoded trace, as described on [`bytes`](Self::bytes).
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl ExecutionTrace {
    /// The events the VM emitted, in order, decoded from the encoded trace.
    ///
    /// Each call decodes the trace again, so collect the events if you need
    /// them more than once. Decoding a large trace costs many times what the
    /// encoded bytes do (a run of ~15,000 events decodes from 66 KB of
    /// compressed bytes to about 4 MB), which is why they are not decoded up
    /// front.
    ///
    /// # Errors
    ///
    /// Returns [`TraceError`] if the encoded trace cannot be opened; the
    /// returned iterator yields one per event that cannot be decoded.
    pub fn events(&self) -> Result<TraceEvents<'_>, TraceError> {
        let reader = MoveTraceReader::new(std::io::Cursor::new(self.bytes.as_slice()))
            .map_err(|source| TraceError { source })?;
        Ok(TraceEvents(reader))
    }
}

/// The events of an [`ExecutionTrace`], decoded as the iterator advances. See
/// [`ExecutionTrace::events`].
pub struct TraceEvents<'a>(MoveTraceReader<'static, std::io::Cursor<&'a [u8]>>);

impl Iterator for TraceEvents<'_> {
    type Item = Result<TraceEvent, TraceError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next_event() {
            Ok(Some(event)) => Some(Ok(event)),
            Ok(None) => None,
            Err(source) => Some(Err(TraceError { source })),
        }
    }
}

// Summarised rather than derived: a derived impl would dump `bytes` as a flat
// list of numbers, which is useless and can run to hundreds of kilobytes for a
// real transaction. Reach for `events` to see the decoded events.
impl std::fmt::Debug for ExecutionTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionTrace")
            .field("version", &self.version)
            .field("event_count", &self.event_count)
            .field("bytes_len", &self.bytes.len())
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
    /// Instruction-level execution trace, if [`DebugConfig::trace`] was set and
    /// the run could be traced (see [`DebugConfig::with_tracing`]).
    pub trace: Option<ExecutionTrace>,
}
