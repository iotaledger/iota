// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! In-memory collection of the Move VM's trace events.
//!
//! A [`MoveTraceBuilder`] serializes every event it receives into a
//! zstd-compressed byte stream, which is what the Move trace debugger reads but
//! is opaque to a caller that wants to inspect the run. The builder also hands
//! each event to its [`Tracer`], so an [`EventCollector`] installed there
//! gathers the typed events and both forms come out of a single run.

use std::{cell::RefCell, rc::Rc};

use move_trace_format::{
    format::{MoveTraceBuilder, TraceEvent},
    interface::{Tracer, Writer},
};

use crate::debug::ExecutionTrace;

/// Shared handle onto the events collected so far. The builder owns one handle
/// through its [`EventCollector`]; the run keeps the other to take the events
/// back out.
pub(super) type CollectedEvents = Rc<RefCell<Vec<TraceEvent>>>;

/// Appends every event the VM emits to a [`CollectedEvents`] buffer.
struct EventCollector {
    events: CollectedEvents,
}

impl Tracer for EventCollector {
    fn notify(&mut self, event: &TraceEvent, _writer: Writer<'_>) {
        self.events.borrow_mut().push(event.clone());
    }
}

/// Build a trace builder that also collects its events into `events`.
pub(super) fn collecting_trace_builder(events: &CollectedEvents) -> MoveTraceBuilder {
    MoveTraceBuilder::new_with_tracer(Box::new(EventCollector {
        events: Rc::clone(events),
    }))
}

/// Finish `builder` and pair the encoded trace with the events collected from
/// it.
pub(super) fn finish_trace(builder: MoveTraceBuilder, events: &CollectedEvents) -> ExecutionTrace {
    let trace = builder.into_trace();
    let version = trace.version;
    // Joins the encoder thread, so every event has been through `notify` by the
    // time the events are taken below.
    let bytes = trace.into_compressed_json_bytes();
    ExecutionTrace::new(version, std::mem::take(&mut *events.borrow_mut()), bytes)
}
