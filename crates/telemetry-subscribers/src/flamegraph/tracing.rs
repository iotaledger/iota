// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! `tracing` subscriber collecting flamegraphs.

use std::sync::Arc;

#[cfg(debug_assertions)]
use super::flame::NodeId;
use super::{
    flame::{Flames, FrameLabel, GraphId, Metadata, Tid},
    grafana,
    metric::{Clock, FlameMetric},
    svg,
};

#[derive(Debug)]
struct FlameSpanInner {
    // The only useful data provided by tracing API that can be stored here.
    metadata: &'static tracing::Metadata<'static>,
    // Debug info to ensure correct enter/exit pairing.
    #[cfg(debug_assertions)]
    entry: Option<(Tid, NodeId)>,
}

impl Clone for FlameSpanInner {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata,
            #[cfg(debug_assertions)]
            entry: None,
        }
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
struct FlameSpan(std::boxed::Box<FlameSpanInner>);

impl FlameSpan {
    fn new(attrs: &tracing::span::Attributes<'_>) -> Self {
        Self(Box::new(FlameSpanInner {
            metadata: attrs.metadata(),
            #[cfg(debug_assertions)]
            entry: None,
        }))
    }

    fn clone_span(id: &tracing::span::Id) -> tracing::span::Id {
        let span: FlameSpanRef<'_> = id.into();
        // Only metadata is cloned, entry is not.
        Self(std::boxed::Box::new(span.clone())).into()
    }
}

impl From<FlameSpan> for tracing::span::Id {
    fn from(span: FlameSpan) -> tracing::span::Id {
        let raw = std::boxed::Box::into_raw(span.0);
        tracing::span::Id::from_u64(raw as u64)
    }
}

impl From<tracing::span::Id> for FlameSpan {
    fn from(id: tracing::span::Id) -> Self {
        let raw = id.into_u64() as *mut _;
        // SAFETY: `tracing` guarantees that the corresponding object is valid (eg. was
        // not dropped yet).
        let ptr = unsafe { std::boxed::Box::from_raw(raw) };
        Self(ptr)
    }
}

#[derive(Debug)]
#[repr(transparent)]
struct FlameSpanRef<'a>(&'a FlameSpanInner);

impl<'a> From<&'a tracing::span::Id> for FlameSpanRef<'a> {
    fn from(id: &'a tracing::span::Id) -> Self {
        let raw = id.into_u64() as *const FlameSpanInner;
        // SAFETY: `tracing` guarantees that the corresponding object is valid (eg. was
        // not dropped yet).
        Self(unsafe { &*raw as &FlameSpanInner })
    }
}

impl<'a> std::ops::Deref for FlameSpanRef<'a> {
    type Target = FlameSpanInner;

    fn deref(&self) -> &FlameSpanInner {
        self.0
    }
}

#[derive(Debug)]
#[repr(transparent)]
struct FlameSpanMut<'a>(&'a mut FlameSpanInner);

impl<'a> From<&'a tracing::span::Id> for FlameSpanMut<'a> {
    fn from(id: &'a tracing::span::Id) -> Self {
        let raw = id.into_u64() as *mut FlameSpanInner;
        // SAFETY: `tracing` guarantees that the corresponding object is valid (eg. was
        // not dropped yet) and that there are no other references to it while
        // we have a mutable reference.
        Self(unsafe { &mut *raw as &mut FlameSpanInner })
    }
}

impl<'a> std::ops::Deref for FlameSpanMut<'a> {
    type Target = FlameSpanInner;

    fn deref(&self) -> &FlameSpanInner {
        self.0
    }
}

impl<'a> std::ops::DerefMut for FlameSpanMut<'a> {
    fn deref_mut(&mut self) -> &mut FlameSpanInner {
        self.0
    }
}

#[derive(Clone, Default)]
pub struct FlameSub {
    flames: Arc<Flames<FlameMetric>>,
}

impl FlameSub {
    pub fn new() -> Self {
        Self {
            flames: Arc::new(Flames::new()),
        }
    }

    fn enabled_metadata(&self, metadata: &tracing::Metadata<'_>) -> bool {
        // This method is called without any reference to the actual call graph in which
        // the span will be used. Thus we have to enable all spans, we are not
        // interested in events.
        metadata.is_span()
    }

    fn enter_metadata(&self, metadata: &tracing::Metadata<'static>) {
        let tid = Tid::current();
        let _ = self.flames.enter(
            tid,
            FrameLabel {
                name: metadata.name(),
            },
            metadata.target(),
            Clock::now(),
        );
    }

    fn enter_span(&self, #[allow(unused_mut)] mut span: FlameSpanMut<'_>) {
        let tid = Tid::current();
        #[cfg(debug_assertions)]
        {
            debug_assert!(span.entry.is_none());
        }
        let _cursor = self.flames.enter(
            tid,
            FrameLabel {
                name: span.metadata.name(),
            },
            span.metadata.target(),
            Clock::now(),
        );
        #[cfg(debug_assertions)]
        {
            span.entry = Some((tid, _cursor));
        }
    }

    fn exit(&self) {
        let tid = Tid::current();
        self.flames.exit(tid, Clock::now());
    }

    #[cfg(debug_assertions)]
    fn exit_checked(&self, mut span: FlameSpanMut<'_>) {
        let tid = Tid::current();
        if let Some((entry_tid, cursor)) = span.entry.take() {
            debug_assert_eq!(entry_tid, tid);
            self.flames.exit_checked(
                tid,
                span.metadata.name(),
                span.metadata.target(),
                cursor,
                Clock::now(),
            );
        } else {
            panic!("span has not been entered prior to exiting");
        }
    }

    pub fn list_nested_sets(&self) -> Vec<(GraphId, f64)> {
        use grafana::Dashboard as _;
        self.flames.list_nested_sets()
    }

    pub fn get_nested_sets(
        &self,
        label: &'static str,
        running: bool,
        completed: bool,
    ) -> Vec<grafana::NestedSetFrame> {
        use grafana::Dashboard as _;
        self.flames.get_nested_sets(label, running, completed)
    }

    pub fn get_nested_set(
        &self,
        graph_id: &str,
        running: bool,
        completed: bool,
    ) -> Vec<grafana::NestedSetFrame> {
        use grafana::Dashboard as _;
        self.flames.get_nested_set(graph_id, running, completed)
    }

    pub fn get_svg(
        &self,
        graph_id: &str,
        running: bool,
        completed: bool,
        config: &svg::Config,
    ) -> Option<svg::Svg> {
        self.flames
            .render_svg(&Metadata::from(graph_id), running, completed, config)
    }

    pub fn get_combined_svg(
        &self,
        caption: &str,
        running: bool,
        completed: bool,
        config: &svg::Config,
    ) -> Option<svg::Svg> {
        self.flames
            .render_combined_svg(caption, running, completed, config)
    }
}

impl tracing::Subscriber for FlameSub {
    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}
    fn event(&self, _event: &tracing::Event<'_>) {}

    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        self.enabled_metadata(metadata)
    }

    fn new_span(&self, attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        FlameSpan::new(attrs).into()
    }

    fn clone_span(&self, id: &tracing::span::Id) -> tracing::span::Id {
        FlameSpan::clone_span(id)
    }

    fn enter(&self, id: &tracing::span::Id) {
        let span = FlameSpanMut::from(id);
        self.enter_span(span);
    }

    fn exit(&self, _id: &tracing::span::Id) {
        #[cfg(debug_assertions)]
        {
            let span = FlameSpanMut::from(_id);
            self.exit_checked(span);
        }
        #[cfg(not(debug_assertions))]
        self.exit();
    }

    fn try_close(&self, id: tracing::span::Id) -> bool {
        let _span = FlameSpan::from(id);
        true
    }
}

impl<S> tracing_subscriber::Layer<S> for FlameSub
where
    S: tracing::Subscriber + for<'lookup> tracing_subscriber::registry::LookupSpan<'lookup>,
{
    fn enabled(
        &self,
        _metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        // When used as a Layer, we must return true for events to allow them to pass
        // through to other layers.
        true
    }

    fn on_new_span(
        &self,
        _attrs: &tracing::span::Attributes<'_>,
        _id: &tracing::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }

    fn on_id_change(
        &self,
        _old: &tracing::span::Id,
        _new: &tracing::span::Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
    }

    fn on_enter(&self, id: &tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = ctx.metadata(id).unwrap();
        self.enter_metadata(metadata);
    }

    fn on_exit(&self, _id: &tracing::span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.exit();
    }

    fn on_close(&self, _id: tracing::span::Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {}
}
