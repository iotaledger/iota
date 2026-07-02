// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-run execution environment.
//!
//! A fresh [`ExecutionEnv`] is built for each [`LocalVm`] run, holding the
//! debug-configured Move engine for that one call. This module also owns the
//! executor construction and the gas-profile capture.

use std::sync::Arc;

use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_types::metrics::{BytecodeVerifierMetrics, LimitsMetrics};
use move_trace_format::format::MoveTraceBuilder;

use crate::{
    debug::{DebugArtifacts, DebugConfig, ProfileOutput, ProfileSink},
    error::{VmError, VmSdkError},
    executor::local_vm::LocalVm,
};

/// The Move engine and debug wiring for a single `execute*` call.
pub(super) struct ExecutionEnv {
    pub(super) protocol_config: ProtocolConfig,
    pub(super) reference_gas_price: u64,
    pub(super) epoch_id: u64,
    pub(super) epoch_timestamp_ms: u64,
    pub(super) executor: Arc<dyn Executor + Send + Sync>,
    pub(super) limits_metrics: Arc<LimitsMetrics>,
    pub(super) bytecode_verifier_metrics: Arc<BytecodeVerifierMetrics>,
    debug: DebugConfig,
    profile_capture: Option<ProfileCapture>,
}

/// Where a run's gas profile is captured: a temp dir the profiler writes its
/// per-invocation JSON into, plus the caller's requested output path for
/// [`ProfileSink::Path`] (`None` for [`ProfileSink::Capture`]).
struct ProfileCapture {
    dir: std::path::PathBuf,
    target: Option<std::path::PathBuf>,
}

impl ExecutionEnv {
    pub(super) fn new(vm: &LocalVm, debug: &DebugConfig) -> Result<Self, VmSdkError> {
        if debug.any_enabled() {
            warn_if_tracing_unavailable();
        }
        // A profiled run builds its own executor because `iota_execution::executor`
        // bakes the profiler path in at construction; otherwise the VM's shared
        // executor is reused.
        let (executor, profile_capture) = match &debug.profile {
            Some(sink) => {
                let (executor, capture) = build_executor_with_profile(&vm.protocol_config, sink)?;
                (executor, Some(capture))
            }
            None => (vm.cached_executor()?.clone(), None),
        };

        Ok(Self {
            protocol_config: vm.protocol_config.clone(),
            reference_gas_price: vm.reference_gas_price,
            epoch_id: vm.epoch_id,
            epoch_timestamp_ms: vm.epoch_timestamp_ms,
            executor,
            limits_metrics: vm.limits_metrics.clone(),
            bytecode_verifier_metrics: vm.bytecode_verifier_metrics.clone(),
            debug: debug.clone(),
            profile_capture,
        })
    }

    pub(super) fn trace_enabled(&self) -> bool {
        self.debug.trace
    }

    /// Materialise captured artifacts: the gas profile and the finished trace.
    /// Returns `Ok(None)` when no debug capture was requested.
    ///
    /// # Errors
    ///
    /// Returns [`VmSdkError::Vm`] when a requested
    /// [`ProfileSink::Path`] cannot be written.
    pub(super) fn collect_artifacts(
        &self,
        trace_builder: Option<MoveTraceBuilder>,
    ) -> Result<Option<DebugArtifacts>, VmSdkError> {
        if !self.debug.any_enabled() {
            return Ok(None);
        }
        let profile = collect_profile(self.profile_capture.as_ref())?;

        Ok(Some(DebugArtifacts {
            profile,
            trace: trace_builder.map(|b| b.into_trace()),
        }))
    }
}

impl Drop for ExecutionEnv {
    fn drop(&mut self) {
        if let Some(capture) = self.profile_capture.take() {
            if let Err(e) = std::fs::remove_dir_all(&capture.dir) {
                eprintln!(
                    "iota-vm-sdk: failed to remove gas-profile temp dir {}: {e}",
                    capture.dir.display()
                );
            }
        }
    }
}

/// Warn once when a debug capture was requested but the crate was built
/// without the `tracing` feature, so the otherwise-silent no-op is visible.
#[cfg(not(feature = "tracing"))]
fn warn_if_tracing_unavailable() {
    use std::sync::Once;
    static WARNED: Once = Once::new();
    WARNED.call_once(|| {
        eprintln!(
            "iota-vm-sdk: a gas profile or trace was requested, but the crate was built \
             without the `tracing` feature; nothing will be captured. Rebuild with \
             `--features tracing` to enable it."
        );
    });
}

/// With the `tracing` feature on, capture works and there is nothing to warn
/// about.
#[cfg(feature = "tracing")]
fn warn_if_tracing_unavailable() {}

/// Build a Move executor with no profiler.
pub(super) fn build_executor(
    protocol_config: &ProtocolConfig,
) -> Result<Arc<dyn Executor + Send + Sync>, VmSdkError> {
    // `silent = true`: the Move `debug::print` natives are compiled out of this
    // build (they need `move-stdlib-natives`'s `testing` feature), so a
    // non-silent executor would only route the same gas charge through a no-op.
    iota_execution::executor(protocol_config, true, None).map_err(|e| VmError::new(e).into())
}

fn build_executor_with_profile(
    protocol_config: &ProtocolConfig,
    sink: &ProfileSink,
) -> Result<(Arc<dyn Executor + Send + Sync>, ProfileCapture), VmSdkError> {
    // Both sinks point the profiler at a temp dir we own: it writes one
    // timestamped file per VM invocation there, which `collect_profile` merges.
    // `ProfileSink::Path` additionally records the caller's target path.
    let dir = profile_capture_dir();
    std::fs::create_dir_all(&dir).map_err(|e| VmError::new(format!("create profile dir: {e}")))?;
    let target = match sink {
        ProfileSink::Path(p) => Some(p.clone()),
        ProfileSink::Capture => None,
    };

    // See `build_executor` for why the executor is always silent.
    let executor = iota_execution::executor(protocol_config, true, Some(dir.join("profile.json")))
        .map_err(VmError::new)?;
    Ok((executor, ProfileCapture { dir, target }))
}

fn collect_profile(capture: Option<&ProfileCapture>) -> Result<Option<ProfileOutput>, VmSdkError> {
    let Some(capture) = capture else {
        return Ok(None);
    };
    // Merge the profiler's per-invocation files; `None` when nothing was written
    // (e.g. an unmetered run).
    let Some(merged) = merge_profile_dir(&capture.dir) else {
        return Ok(None);
    };
    match &capture.target {
        // `ProfileSink::Path`: write the merged profile to the caller's path.
        Some(target) => {
            std::fs::write(target, &merged).map_err(|e| {
                VmError::new(format!("write gas profile to {}: {e}", target.display()))
            })?;
            Ok(Some(ProfileOutput::Path(target.clone())))
        }
        // `ProfileSink::Capture`: hand back the merged bytes.
        None => Ok(Some(ProfileOutput::Json(merged))),
    }
}

/// Name prefix for the per-run gas-profile capture directory created in the
/// system temp dir.
const PROFILE_CAPTURE_DIR_PREFIX: &str = "iota-vm-sdk-gas-profile-";

fn profile_capture_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("{PROFILE_CAPTURE_DIR_PREFIX}{pid}-{n}"))
}

/// Merge every Speedscope JSON file the profiler wrote into `dir` into one
/// document. The profiler writes one file per VM invocation (e.g. an
/// authenticator call plus the PTB body), each with its own frames table, so
/// the merge concatenates `profiles` and rebuilds a de-duplicated
/// `shared.frames`.
fn merge_profile_dir(dir: &std::path::Path) -> Option<Vec<u8>> {
    let entries = std::fs::read_dir(dir).ok()?;
    // Sort by file name: the profiler's nanosecond-stamped names put the
    // authenticator profile(s) before the PTB body, i.e. invocation order.
    let mut files: Vec<(std::ffi::OsString, serde_json::Value)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                files.push((path.file_name().unwrap_or_default().to_os_string(), doc));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let docs: Vec<serde_json::Value> = files.into_iter().map(|(_, doc)| doc).collect();
    if docs.is_empty() {
        return None;
    }
    if docs.len() == 1 {
        return serde_json::to_vec(&docs[0]).ok();
    }
    let mut merged_frames: Vec<serde_json::Value> = Vec::new();
    let mut merged_profiles: Vec<serde_json::Value> = Vec::new();
    // Label invocations (last is the PTB body, rest authenticators), keeping
    // the original name (the transaction digest) in parentheses.
    let last = docs.len() - 1;
    for (idx, mut doc) in docs.into_iter().enumerate() {
        let Some(frames) = doc
            .pointer("/shared/frames")
            .and_then(|v| v.as_array())
            .cloned()
        else {
            continue;
        };
        let mut index_map: Vec<usize> = Vec::with_capacity(frames.len());
        for frame in frames {
            let existing = merged_frames.iter().position(|f| f == &frame);
            index_map.push(existing.unwrap_or_else(|| {
                merged_frames.push(frame);
                merged_frames.len() - 1
            }));
        }
        if let Some(profiles) = doc.get_mut("profiles").and_then(|v| v.as_array_mut()) {
            for profile in profiles.iter_mut() {
                if let Some(events) = profile.get_mut("events").and_then(|v| v.as_array_mut()) {
                    for ev in events.iter_mut() {
                        if let Some(frame) = ev
                            .get("frame")
                            .and_then(|v| v.as_u64())
                            .and_then(|i| usize::try_from(i).ok())
                        {
                            if let Some(new) = index_map.get(frame) {
                                ev["frame"] = serde_json::json!(new);
                            }
                        }
                    }
                }
                let label = if idx == last {
                    "transaction body".to_string()
                } else if last == 1 {
                    "authenticator".to_string()
                } else {
                    format!("authenticator {}", idx + 1)
                };
                let digest = profile
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let named = format!("{label} ({digest})");
                profile["name"] = serde_json::json!(named);
                merged_profiles.push(profile.clone());
            }
        }
    }
    let merged = serde_json::json!({
        "exporter": "iota-vm-sdk (merged)",
        "$schema": "https://www.speedscope.app/file-format-schema.json",
        "shared": { "frames": merged_frames },
        "profiles": merged_profiles,
    });
    serde_json::to_vec(&merged).ok()
}
