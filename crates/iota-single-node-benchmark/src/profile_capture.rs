// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::Write,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer};

/// Gates row emission so the benchmark's setup transactions (account
/// funding, package publishing) don't pollute the dataset; `run_benchmark`
/// enables capture only around the measured execution phase.
static CAPTURE_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_capture_enabled(enabled: bool) {
    CAPTURE_ENABLED.store(enabled, Ordering::Relaxed);
}

/// A tracing layer that writes one JSON line per executed transaction,
/// containing the transaction digest, the measured wall-clock nanoseconds,
/// and the full resource profile.
///
/// It captures the `resource_profile`-target event emitted by
/// `AuthorityState::execute_transaction`, which carries the profile
/// pre-serialized as JSON in its `profile_json` field. The first line of the
/// output file is a metadata record describing the run; every following line
/// is `{"tx_digest": ..., "measured_ns": ..., "profile": {...}}`.
pub struct ProfileCapture {
    out: Mutex<File>,
}

impl ProfileCapture {
    pub fn new(path: &Path, run_meta: serde_json::Value) -> std::io::Result<Self> {
        let mut file = File::create(path)?;
        writeln!(file, "{}", serde_json::json!({ "meta": run_meta }))?;
        Ok(Self {
            out: Mutex::new(file),
        })
    }
}

#[derive(Default)]
struct EventFields {
    tx_digest: Option<String>,
    measured_ns: Option<u64>,
    profile_json: Option<String>,
}

impl Visit for EventFields {
    fn record_u64(&mut self, field: &Field, value: u64) {
        if field.name() == "measured_ns" {
            self.measured_ns = Some(value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "tx_digest" => self.tx_digest = Some(format!("{value:?}")),
            "profile_json" => self.profile_json = Some(format!("{value:?}")),
            _ => {}
        }
    }
}

impl<S: tracing::Subscriber> Layer<S> for ProfileCapture {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        if event.metadata().target() != "resource_profile"
            || !CAPTURE_ENABLED.load(Ordering::Relaxed)
        {
            return;
        }
        let mut fields = EventFields::default();
        event.record(&mut fields);
        // The execution engine emits a second `resource_profile` event without
        // `measured_ns`/`profile_json`; requiring all three fields skips it.
        let (Some(tx_digest), Some(measured_ns), Some(profile)) =
            (fields.tx_digest, fields.measured_ns, fields.profile_json)
        else {
            return;
        };
        let mut out = self.out.lock().unwrap();
        // `profile` is already JSON, so it is spliced in verbatim.
        let _ = writeln!(
            out,
            "{{\"tx_digest\":{},\"measured_ns\":{measured_ns},\"profile\":{profile}}}",
            serde_json::json!(tx_digest),
        );
    }
}
