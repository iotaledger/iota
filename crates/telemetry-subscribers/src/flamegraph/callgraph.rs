// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! CallGraph structure that is able to collect call tree data of functions
//! properly annotated. It is able to convert the data to a nested-set form
//! suitable for Grafana Flame Graph panel.

use std::fmt;

pub use super::arena::NodeId;
use super::{
    arena::{Arena, TreeNode},
    metric::{MergeMetrics, SpanMetrics},
};

/// Frame identifier that can be constructed from tracing::Metadata.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FrameLabel {
    /// Function name.
    pub(crate) name: &'static str,
}

impl fmt::Display for FrameLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl From<&'static str> for FrameLabel {
    fn from(name: &'static str) -> Self {
        Self { name }
    }
}

impl PartialEq<str> for FrameLabel {
    fn eq(&self, other: &str) -> bool {
        self.name.eq(other)
    }
}

/// Call graph frame.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Frame<S> {
    /// Frame identifier.
    pub(super) label: FrameLabel,
    /// Frame execution telemetry.
    pub(super) metrics: S,
}

impl<S> Frame<S> {
    fn new(label: FrameLabel, metrics: S) -> Self {
        Self { label, metrics }
    }
}

/// A single call graph.
///
/// Internally, it is represented by a tree, root node represents the entry
/// function. The entry function can be called multiple times (including
/// recursive re-entries), thus gathering stats about multiple calls.
#[derive(Clone)]
pub struct CallGraph<S> {
    /// Tree with nodes recording Frame values.
    pub(super) graph: Arena<TreeNode<Frame<S>>>,
    /// The position of the currently entered function in the call graph.
    /// `None` value points to "outside" of call graph, meaning it was not yet
    /// entered. Cursor is updated when a frame is entered/exited.
    pub(super) cursor: Option<NodeId>,
}

impl<S> Default for CallGraph<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> CallGraph<S> {
    /// Create an empty call graph.
    pub fn new() -> Self {
        Self {
            graph: Arena::default(),
            cursor: None,
        }
    }

    pub fn cursor(&self) -> Option<NodeId> {
        self.cursor
    }

    pub(super) fn root(&self) -> Option<&Frame<S>> {
        (!self.graph.is_empty()).then(|| &self.graph[NodeId::default()].value)
    }
}

impl<S: SpanMetrics> CallGraph<S> {
    #[inline]
    fn find_frame(&self, cursor: NodeId, label: FrameLabel) -> Option<NodeId> {
        self.graph
            .find_child(cursor, |frame_node| frame_node.value.label == label)
    }

    /// Lookup an existing frame with that label or create a new one.
    fn enter_frame_node(&mut self, label: FrameLabel) -> NodeId {
        if let Some(cursor) = self.cursor {
            // already have some frames, can try re-opening some existing child frame?
            if S::REENTER {
                // try finding child frame node with the provided label
                if let Some(node_id) = self.find_frame(cursor, label) {
                    // we already entered this frame before, just return its node id
                    return node_id;
                }
                // we haven't entered this frame yet
            }

            // just create a new frame node
            self.graph
                .push_leaf(cursor, Frame::new(label, S::default()))
        } else {
            // no frames yet, this is the entry function corresponding to the root frame
            // node
            if self.graph.is_empty() {
                // add a new root node to the graph
                self.graph.push_root(Frame::new(label, S::default()))
            } else {
                // root already exists (entered and exited the corresponding frame); this is
                // allowed! just return root node id
                let root_id = NodeId::default();

                // make sure we are entering the same entry function
                debug_assert_eq!(label, self.graph[root_id].value.label);
                root_id
            }
        }
    }

    /// Open and enter frame.
    pub fn enter_frame(&mut self, label: FrameLabel, arg: <S as SpanMetrics>::Arg) -> NodeId {
        let cursor = self.enter_frame_node(label);
        self.graph[cursor].value.metrics.enter(arg);
        self.cursor = Some(cursor);
        cursor
    }

    fn exit_frame_node(&mut self, cursor: NodeId, arg: <S as SpanMetrics>::Arg) -> bool {
        let node = &mut self.graph[cursor];
        node.value.metrics.exit(arg);

        // update cursor to point to the parent frame node
        if NodeId::default() == cursor {
            // self.cursor points to root
            // check for invariant: root node parent is root itself
            debug_assert_eq!(cursor, node.parent);
            self.cursor = None;
            true
        } else {
            // parent is well defined
            self.cursor = Some(node.parent);
            false
        }
    }

    /// Exit and close frame and return true if it was a root frame indicating
    /// that the call graph has finished.
    pub fn exit_frame(&mut self, arg: <S as SpanMetrics>::Arg) -> bool {
        let cursor = self.cursor.expect("Cursor must be non-empty");
        self.exit_frame_node(cursor, arg)
    }

    /// Exit and close frame and return true if it was a root frame indicating
    /// that the call graph has finished.
    #[cfg(debug_assertions)]
    pub fn exit_frame_checked(&mut self, cursor: NodeId, arg: <S as SpanMetrics>::Arg) -> bool {
        assert_eq!(cursor, self.cursor.expect("Cursor must be non-empty"));
        self.exit_frame_node(cursor, arg)
    }
}

impl<S: MergeMetrics> CallGraph<S> {
    pub fn merge(&mut self, other: CallGraph<S>) {
        assert!(self.cursor.is_none());

        // we allow merging still running call graphs
        self.graph.merge(
            other.graph,
            |frame, other_frame| frame.label == other_frame.label,
            |frame, other_frame| frame.metrics.merge(other_frame.metrics),
        );
    }
}
