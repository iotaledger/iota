// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Per-run execution environment.
//!
//! A fresh [`ExecutionEnv`] is built for each [`LocalVm`] run, holding the
//! debug-configured Move engine for that one `execute*` / `execute_signed` call
//! (a single `LocalVm` may run many). This module also owns the executor
//! construction and the gas-profile capture.

use std::sync::Arc;

use iota_execution::Executor;
use iota_protocol_config::ProtocolConfig;
use iota_types::metrics::{BytecodeVerifierMetrics, LimitsMetrics};
use move_trace_format::format::MoveTraceBuilder;

use super::local_vm::LocalVm;
use crate::{
    debug::{DebugArtifacts, DebugConfig, ProfileOutput, ProfileSink},
    error::{VmError, VmSdkError},
};

/// Per-run engine + debug wiring, built fresh for each `execute*` call because
/// `iota_execution::executor` bakes the profiler path in at construction.
pub(super) struct ExecutionEnv {
    pub(super) protocol_config: ProtocolConfig,
    pub(super) reference_gas_price: u64,
    pub(super) epoch_id: u64,
    pub(super) epoch_timestamp_ms: u64,
    pub(super) executor: Arc<dyn Executor + Send + Sync>,
    pub(super) limits_metrics: Arc<LimitsMetrics>,
    pub(super) bytecode_verifier_metrics: Arc<BytecodeVerifierMetrics>,
    debug: DebugConfig,
    capture_profile_dir: Option<std::path::PathBuf>,
}

impl ExecutionEnv {
    pub(super) fn new(vm: &LocalVm, debug: &DebugConfig) -> Result<Self, VmSdkError> {
        let (executor, capture_profile_dir) =
            build_executor_with_profile(&vm.protocol_config, debug)?;

        Ok(Self {
            protocol_config: vm.protocol_config.clone(),
            reference_gas_price: vm.reference_gas_price,
            epoch_id: vm.epoch_id,
            epoch_timestamp_ms: vm.epoch_timestamp_ms,
            executor,
            limits_metrics: vm.limits_metrics.clone(),
            bytecode_verifier_metrics: vm.bytecode_verifier_metrics.clone(),
            debug: debug.clone(),
            capture_profile_dir,
        })
    }

    pub(super) fn trace_enabled(&self) -> bool {
        self.debug.trace
    }

    /// Materialise captured artifacts: the gas profile and the finished trace.
    /// Returns `None` when no debug capture was requested.
    pub(super) fn collect_artifacts(
        &self,
        trace_builder: Option<MoveTraceBuilder>,
    ) -> Option<DebugArtifacts> {
        if !self.debug.any_enabled() {
            return None;
        }
        let profile = collect_profile(&self.debug, self.capture_profile_dir.as_deref());

        Some(DebugArtifacts {
            profile,
            trace: trace_builder.map(|b| b.into_trace()),
        })
    }
}

impl Drop for ExecutionEnv {
    fn drop(&mut self) {
        if let Some(dir) = self.capture_profile_dir.take() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

/// Build a Move executor with no profiler.
pub(super) fn build_executor(
    protocol_config: &ProtocolConfig,
    _debug: &DebugConfig,
) -> Result<Arc<dyn Executor + Send + Sync>, VmSdkError> {
    // `silent = true`: the Move `debug::print` natives are compiled out of this
    // build (they need `move-stdlib-natives`'s `testing` feature), so a
    // non-silent executor would only route the same gas charge through a no-op.
    iota_execution::executor(protocol_config, true, None).map_err(|e| VmError::new(e).into())
}

fn build_executor_with_profile(
    protocol_config: &ProtocolConfig,
    debug: &DebugConfig,
) -> Result<(Arc<dyn Executor + Send + Sync>, Option<std::path::PathBuf>), VmSdkError> {
    let (profile_path, capture_dir) = match &debug.profile {
        Some(ProfileSink::Path(p)) => (Some(p.clone()), None),
        Some(ProfileSink::Capture) => {
            let dir = profile_capture_dir();
            std::fs::create_dir_all(&dir)
                .map_err(|e| VmError::new(format!("create profile dir: {e}")))?;
            (Some(dir.join("profile.json")), Some(dir))
        }
        _ => (None, None),
    };

    // See `build_executor` for why the executor is always silent.
    let executor =
        iota_execution::executor(protocol_config, true, profile_path).map_err(VmError::new)?;
    Ok((executor, capture_dir))
}

fn collect_profile(
    debug: &DebugConfig,
    capture_dir: Option<&std::path::Path>,
) -> Option<ProfileOutput> {
    match (&debug.profile, capture_dir) {
        (Some(ProfileSink::Path(p)), _) => Some(ProfileOutput::Path(p.clone())),
        (Some(ProfileSink::Capture), Some(dir)) => merge_profile_dir(dir),
        _ => None,
    }
}

fn profile_capture_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("iota-vm-sdk-gas-profile-{pid}-{n}"))
}

/// Merge every Speedscope JSON file the profiler wrote into `dir` into one
/// document. The profiler writes one file per VM invocation (e.g. an
/// authenticator call + the PTB body), each with its own frames table; the
/// merge concatenates `profiles` and rebuilds a de-duplicated `shared.frames`.
fn merge_profile_dir(dir: &std::path::Path) -> Option<ProfileOutput> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut docs: Vec<serde_json::Value> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(doc) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                docs.push(doc);
            }
        }
    }
    if docs.is_empty() {
        return None;
    }
    if docs.len() == 1 {
        return serde_json::to_vec(&docs[0]).ok().map(ProfileOutput::Json);
    }
    let mut merged_frames: Vec<serde_json::Value> = Vec::new();
    let mut merged_profiles: Vec<serde_json::Value> = Vec::new();
    for mut doc in docs {
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
                        if let Some(frame) =
                            ev.get("frame").and_then(|v| v.as_u64()).map(|i| i as usize)
                        {
                            if let Some(new) = index_map.get(frame) {
                                ev["frame"] = serde_json::json!(new);
                            }
                        }
                    }
                }
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
    serde_json::to_vec(&merged).ok().map(ProfileOutput::Json)
}
