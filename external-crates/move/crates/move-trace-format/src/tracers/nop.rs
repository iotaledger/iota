// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{
    format::TraceEvent,
    interface::{Tracer, Writer},
};

pub struct NopTracer;
impl Tracer for NopTracer {
    fn notify(&mut self, _event: &TraceEvent, _writer: Writer<'_>) -> bool {
        // keep all events
        true
    }

    fn wants_effects(&self) -> bool {
        true
    }
}
