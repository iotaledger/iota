// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Thread/task safe call graphs collector.

use std::{
    collections::{HashMap, hash_map},
    fmt,
};

use parking_lot::{Mutex, RwLock};
#[cfg(msim)]
use tokio::task::runtime_task::try_id;
#[cfg(not(msim))]
use tokio::task::try_id;

pub use super::{arena::NodeId, callgraph::FrameLabel};
use super::{
    callgraph::CallGraph,
    metric::{MergeMetrics, SpanMetrics},
};

#[cfg(not(msim))]
type TaskId = tokio::task::Id;
#[cfg(msim)]
type TaskId = tokio::task::runtime_task::Id;

/// Some async fns run inside tokio runtime as tasks and are assigned task IDs.
/// Other async fns run inside tokio runtime but are not assigned task IDs; we
/// want to avoid this case. Sync fns run on a single thread and are assigned
/// thread IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Tid {
    ThreadId(std::thread::ThreadId),
    TaskId(TaskId),
}

impl Tid {
    /// Get the Tid based on current tokio task or thread.
    pub fn current() -> Tid {
        try_id()
            .map(Tid::TaskId)
            .unwrap_or_else(|| Tid::ThreadId(std::thread::current().id()))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Metadata<'a> {
    pub caption: &'a str,
    pub target: &'a str,
}

impl serde::Serialize for Metadata<'_> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if !self.target.is_empty() {
            s.serialize_str(&format!("{}::{}", self.target, self.caption))
        } else {
            s.serialize_str(self.caption)
        }
    }
}

impl fmt::Display for Metadata<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.target.is_empty() {
            write!(f, "{}::{}", self.target, self.caption)
        } else {
            write!(f, "{}", self.caption)
        }
    }
}

impl PartialEq<str> for Metadata<'_> {
    fn eq(&self, other: &str) -> bool {
        if !self.target.is_empty() {
            self.caption.eq(other)
        } else {
            other.eq(&format!("{}::{}", self.target, self.caption))
        }
    }
}

impl<'a> From<&'a str> for Metadata<'a> {
    fn from(s: &'a str) -> Self {
        s.rsplit_once("::")
            .map(|(target, caption)| Self { caption, target })
            .unwrap_or(Self {
                caption: s,
                target: "",
            })
    }
}

pub type GraphId = Metadata<'static>;

pub(super) struct Graph<S> {
    pub(super) graph_id: GraphId,
    pub(super) mutex: Mutex<CallGraph<S>>,
}

/// Associative collection of flamegraphs per thread/task.
///
/// # Important Note
///
/// 1. Internally different call graphs are stored in a hash map indexed by the
///    current Tid. It means that the call graph must only be accessed either
///    from the same tokio task, or from the same thread. Trying to access a
///    call graph (entering/exiting frames) from different tasks/threads will
///    result in undefined behavior.
///
/// 2. Hash map indexed by Tid serves as a mutex guard itself preventing
///    different threads from accessing the same call graph making inner Mutex
///    unnecessary. Currently, the implementation uses Mutex to avoid unsafe
///    code.
pub struct Flames<S> {
    /// Each call graph is associated with a thread/task.
    /// This way the graphs can be stored in a global static and accessed via
    /// thread/task id only without any extra context.
    pub(super) graphs: RwLock<HashMap<Tid, Graph<S>>>,
    pub(super) completed: RwLock<HashMap<GraphId, CallGraph<S>>>,
}

impl<S> Default for Flames<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Flames<S> {
    /// Construct empty storage for flame graphs.
    pub fn new() -> Self {
        Flames {
            graphs: RwLock::new(HashMap::new()),
            completed: RwLock::new(HashMap::new()),
        }
    }
}

impl<S: Default + MergeMetrics + SpanMetrics> Flames<S> {
    /// Merge just finished call graph into the corresponding completed graph.
    fn finalize_call_graph(&self, tid: Tid, graph_id: GraphId, call_graph: CallGraph<S>) {
        if let Some(cursor) = call_graph.cursor() {
            panic!(
                "failed to finish '{graph_id}' call graph at {tid:?}: not all frames exited, cursor {cursor}"
            );
        }

        let mut wlock = self.completed.write();
        match wlock.entry(graph_id) {
            hash_map::Entry::Vacant(entry) => {
                entry.insert(call_graph);
            }
            hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().merge(call_graph);
            }
        }
    }

    /// Enter frame for the call graph associated with the given thread/task id.
    pub fn enter(
        &self,
        tid: Tid,
        label: FrameLabel,
        target: &'static str,
        arg: <S as SpanMetrics>::Arg,
    ) -> NodeId {
        let mut rlock = self.graphs.upgradable_read();
        if let Some(Graph { graph_id: _, mutex }) = rlock.get(&tid) {
            mutex.lock().enter_frame(label, arg)
        } else {
            let mut graph = CallGraph::new();
            let node_id = graph.enter_frame(label, arg);
            let graph_id = GraphId {
                caption: label.name,
                target,
            };

            rlock.with_upgraded(|graphs| {
                graphs.insert(
                    tid,
                    Graph {
                        graph_id,
                        mutex: Mutex::new(graph),
                    },
                )
            });
            node_id
        }
    }

    /// Exit frame for the call graph associated with the given thread/task id.
    pub fn exit(&self, tid: Tid, arg: <S as SpanMetrics>::Arg) {
        let mut rlock = self.graphs.upgradable_read();
        if let Some(Graph { graph_id: _, mutex }) = rlock.get(&tid) {
            let finished = mutex.lock().exit_frame(arg);
            if finished {
                let removed = rlock.with_upgraded(|graphs| graphs.remove_entry(&tid));
                if let Some((removed_tid, Graph { graph_id, mutex })) = removed {
                    debug_assert_eq!(removed_tid, tid);
                    let call_graph = mutex.into_inner();
                    self.finalize_call_graph(tid, graph_id, call_graph);
                } else {
                    panic!(
                        "failed to finish call graph at {tid:?}: there are no running call graphs"
                    );
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    pub fn exit_checked(
        &self,
        tid: Tid,
        label: &'static str,
        target: &'static str,
        cursor: NodeId,
        arg: <S as SpanMetrics>::Arg,
    ) {
        let mut rlock = self.graphs.upgradable_read();
        if let Some(Graph { graph_id: _, mutex }) = rlock.get(&tid) {
            let finished = mutex.lock().exit_frame_checked(cursor, arg);
            if finished {
                let removed = rlock.with_upgraded(|graphs| graphs.remove_entry(&tid));
                if let Some((removed_tid, Graph { graph_id, mutex })) = removed {
                    debug_assert_eq!(removed_tid, tid);
                    debug_assert_eq!(
                        graph_id,
                        GraphId {
                            caption: label,
                            target
                        }
                    );
                    let call_graph = mutex.into_inner();
                    self.finalize_call_graph(tid, graph_id, call_graph);
                } else {
                    panic!(
                        "failed to finish call graph at {tid:?}: there are no running call graphs"
                    );
                }
            }
        }
    }
}
