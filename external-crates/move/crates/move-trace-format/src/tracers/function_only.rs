// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::file_format_common::Opcodes;

use crate::{
    format::TraceEvent,
    interface::{EventFilter, Tracer, Writer},
};

/// A tracer that only keeps `OpenFrame`, and `CloseFrame` events.
pub struct FunctionOnlyTracer;
impl Tracer for FunctionOnlyTracer {
    fn notify(&mut self, event: &TraceEvent, _writer: Writer<'_>) -> bool {
        matches!(
            event,
            TraceEvent::OpenFrame { .. } | TraceEvent::CloseFrame { .. }
        )
    }

    fn instruction_filter(&self, _instruction: &Opcodes, _pc: u16) -> Option<EventFilter> {
        None
    }
}
