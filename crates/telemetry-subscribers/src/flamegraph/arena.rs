// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Arena type suitable for representing append-only graphs (and trees in
//! particular).

use std::fmt;

/// Node index within arena.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(usize);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl NodeId {
    pub fn new() -> Self {
        Self::default()
    }

    fn inc(&mut self) {
        self.0 += 1;
    }
}

impl From<usize> for NodeId {
    fn from(u: usize) -> NodeId {
        Self(u)
    }
}

impl From<NodeId> for usize {
    fn from(n: NodeId) -> usize {
        n.0
    }
}

/// Arena for nodes of type N indexed by NodeId.
/// Use TreeNode<T> as node type N to represent trees.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Arena<N>(Vec<N>);

impl<N> Default for Arena<N> {
    fn default() -> Self {
        Arena(Vec::new())
    }
}

impl<N> std::ops::Index<NodeId> for Arena<N> {
    type Output = N;

    fn index(&self, index: NodeId) -> &N {
        &self.0[index.0]
    }
}

impl<N> std::ops::IndexMut<NodeId> for Arena<N> {
    fn index_mut(&mut self, index: NodeId) -> &mut N {
        &mut self.0[index.0]
    }
}

impl<N> AsRef<[N]> for Arena<N> {
    fn as_ref(&self) -> &[N] {
        &self.0
    }
}

impl<N> Arena<N> {
    pub fn from_raw(iter: impl IntoIterator<Item = N>) -> Self {
        Self(iter.into_iter().collect())
    }

    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn push(&mut self, node: N) -> NodeId {
        let node_id = NodeId(self.0.len());
        self.0.push(node);
        node_id
    }

    pub fn iter(&self) -> impl Iterator<Item = &N> {
        self.0.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = N> {
        self.0.into_iter()
    }
}

impl<N> From<Arena<N>> for Vec<N> {
    fn from(arena: Arena<N>) -> Vec<N> {
        arena.0
    }
}

/// Tree node allocated within a certain Arena.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TreeNode<T> {
    /// Node value.
    pub value: T,
    /// NodeId of a parent node; root node is its own parent and has NodeId(0).
    pub parent: NodeId,
    /// List of children node ids.
    pub children: Vec<NodeId>,
}

/// Child index relative to parent node within arena.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChildId {
    pub parent_id: NodeId,
    pub child_no: usize,
}

pub type ArenaTree<T> = Arena<TreeNode<T>>;

/// Index a child node reference by its index.
impl<T> std::ops::Index<ChildId> for ArenaTree<T> {
    type Output = TreeNode<T>;
    fn index(&self, index: ChildId) -> &TreeNode<T> {
        let ix = self.resolve(index);
        &self[ix]
    }
}

/// Index a mutable child node reference by its index.
impl<T> std::ops::IndexMut<ChildId> for ArenaTree<T> {
    fn index_mut(&mut self, index: ChildId) -> &mut TreeNode<T> {
        let ix = self.resolve(index);
        &mut self[ix]
    }
}

pub type Level = usize;

/// Specialization of Arena type for supporting trees.
impl<T> ArenaTree<T> {
    fn resolve(&self, child_id: ChildId) -> NodeId {
        self[child_id.parent_id].children[child_id.child_no]
    }

    /// Add a childless root node to an empty graph returning the 0 node id.
    pub fn push_root(&mut self, value: T) -> NodeId {
        assert!(self.is_empty());

        self.push(TreeNode {
            value,
            parent: NodeId::default(),
            children: Vec::new(),
        })
    }

    /// Add a child leaf node to a given parent node returning the new node id.
    pub fn push_leaf(&mut self, parent: NodeId, value: T) -> NodeId {
        let node_id = self.push(TreeNode {
            value,
            parent,
            children: Vec::new(),
        });

        self[parent].children.push(node_id);
        node_id
    }
}

impl<T> ArenaTree<T> {
    pub fn find_child<F: FnMut(&TreeNode<T>) -> bool>(
        &self,
        parent: NodeId,
        mut predicate: F,
    ) -> Option<NodeId> {
        self[parent]
            .children
            .iter()
            .copied()
            .find(|ix| predicate(&self[*ix]))
    }

    fn find_unique_child<F: Fn(&T) -> bool>(&self, parent: NodeId, predicate: F) -> Option<NodeId> {
        let mut iter = self[parent]
            .children
            .iter()
            .copied()
            .filter(|child_id| predicate(&self[*child_id].value));

        if let Some(child_id) = iter.next() {
            debug_assert!(
                iter.next().is_none(),
                "there can be at most one child satisfying predicate"
            );
            Some(child_id)
        } else {
            None
        }
    }

    /// Depth-first folding of an accumulator from init value using combining
    /// function taking as input node id and level.
    pub fn dfs_fold<U, F: FnMut(&mut U, NodeId, Level)>(&self, init: U, mut visit_node: F) -> U {
        let mut acc = init;

        // depth-first walk in the tree
        if !self.is_empty() {
            let mut stack = Vec::new();

            // visit the root node
            visit_node(&mut acc, NodeId::default(), 0);
            // push to stack a parent node and its first child index to be visited
            stack.push(ChildId::default());

            while let Some(ChildId {
                parent_id,
                child_no,
            }) = stack.pop()
            {
                let parent = &self[parent_id];
                if child_no < parent.children.len() {
                    // increment the current node's child index so that next time we see this node
                    // in the stack we continue with the next child
                    stack.push(ChildId {
                        parent_id,
                        child_no: child_no + 1,
                    });

                    // child index is valid
                    let node_id = parent.children[child_no];
                    // visit the child node
                    visit_node(&mut acc, node_id, stack.len());

                    // push the child node to stack
                    stack.push(ChildId {
                        parent_id: node_id,
                        child_no: 0,
                    });
                }
            }
        }
        acc
    }

    /// Depth-first folding of an accumulator from the initial value using
    /// combining functions. The `enter_node` closure receives the
    /// accumulator, a child value, the node id, and the level. The
    /// `visit_child` closure receives the accumulator, a reference to the
    /// child value, and the node id.
    pub fn dfs_fold2<U, V, E, F, G>(
        &self,
        acc: &mut U,
        child_init: E,
        mut enter_node: F,
        mut visit_child: G,
    ) where
        E: FnOnce() -> V,
        F: FnMut(&mut U, V, NodeId, Level) -> V,
        G: FnMut(&mut U, &V, NodeId) -> V,
    {
        if !self.is_empty() {
            let mut stack = Vec::new();
            let root = NodeId::default();
            let child_init = enter_node(acc, child_init(), root, 0);
            stack.push((
                ChildId {
                    parent_id: root,
                    child_no: 0,
                },
                child_init,
            ));
            while let Some((
                ChildId {
                    parent_id,
                    child_no,
                },
                child_acc,
            )) = stack.pop()
            {
                let parent = &self[parent_id];
                if child_no < parent.children.len() {
                    let child_id = parent.children[child_no];
                    let next_child_acc = visit_child(acc, &child_acc, child_id);
                    stack.push((
                        ChildId {
                            parent_id,
                            child_no: child_no + 1,
                        },
                        next_child_acc,
                    ));
                    let child_init = enter_node(acc, child_acc, child_id, stack.len());
                    stack.push((
                        ChildId {
                            parent_id: child_id,
                            child_no: 0,
                        },
                        child_init,
                    ));
                }
            }
        }
    }

    /// Tree is normalized if depth-first walk traverses nodes in order they are
    /// internally stored.
    pub(super) fn is_normalized(&self) -> bool {
        self.dfs_fold((true, NodeId(0)), |(ok, id), node_id, _| {
            *ok = *ok && *id == node_id;
            id.inc();
        })
        .0
    }

    /// Rearrange nodes in the arena so that depth-first walk traverses nodes in
    /// order they are internally stored.
    fn normalize_nodes(&mut self) {
        let len = self.len();
        let mut map = self.dfs_fold(Vec::with_capacity(len), |map, node_id, _| {
            map.push(node_id.0)
        });

        let mut inv = vec![0; len];
        for n in 0..len {
            inv[map[n]] = n;
        }

        for n in 0..len {
            let m = map[n];
            if m != n {
                self.0.swap(n, m);
                let i = inv[n];
                map.swap(n, i);
                inv.swap(n, m);
            }
        }
    }

    /// Rearrange children node ids so that they point to correct nodes after
    /// normalization.
    fn normalize_children(&mut self) {
        let mut node_id = NodeId::default();
        let mut stack = Vec::new();
        debug_assert_eq!(node_id, self[node_id].parent);
        stack.push(ChildId::default());
        node_id.inc();

        while let Some(ChildId {
            parent_id,
            child_no,
        }) = stack.pop()
        {
            let children_len = self[parent_id].children.len();
            if child_no < children_len {
                self[parent_id].children[child_no] = node_id;
                stack.push(ChildId {
                    parent_id,
                    child_no: child_no + 1,
                });

                self[node_id].parent = parent_id;
                stack.push(ChildId {
                    parent_id: node_id,
                    child_no: 0,
                });

                node_id.inc();
            }
        }
    }

    /// Reorder nodes and their children so that depth-first walk traverses
    /// nodes in order they are internally stored.
    pub(super) fn normalize(&mut self) {
        if !self.is_empty() {
            self.normalize_nodes();
            self.normalize_children();
        }
        debug_assert!(self.is_normalized());
    }

    fn rebase_branch<I: Iterator<Item = TreeNode<T>>>(
        &mut self,
        parent_id: NodeId,
        mut other_child: TreeNode<T>,
        other: &mut I,
        mut other_id: NodeId,
    ) -> NodeId {
        debug_assert!(other_id.0 > 0, "{other_id:?} > 0");

        let other_base = other_id.0 - 1; // other_child other node id
        let self_base = self.0.len(); // other_child node id when rebased into self
        debug_assert!(other_base <= self_base, "{other_base} <= {self_base}");

        let id_delta = self_base - other_base;
        let adjust_node_id = |c: &mut NodeId| c.0 += id_delta;

        // number of nodes to rebase
        let mut m = 1; // just one: other_child
        // number of rebased nodes
        let mut k = 0_usize;

        // other_child is rebased as parent's child: adjust child node's parent id
        other_child.parent = parent_id;
        other_child.children.iter_mut().for_each(adjust_node_id);
        m += other_child.children.len();
        self.0.push(other_child);
        k += 1;

        // other_child is rebased as parent's child: add child node's id into parent
        // node's children list
        self[parent_id].children.push(NodeId(self_base));

        // while we have nodes to rebase
        while k < m {
            let mut other_node = other.next().unwrap();
            other_id.inc();

            adjust_node_id(&mut other_node.parent);
            other_node.children.iter_mut().for_each(adjust_node_id);
            m += other_node.children.len();

            self.0.push(other_node);
            k += 1;
        }

        other_id
    }

    /// Merge another arena tree into this one using provided functions to
    /// decide if two nodes can be merged.
    pub fn merge<G, H>(&mut self, mut other: Self, can_merge_node: G, merge_node: H)
    where
        G: Fn(&T, &T) -> bool,
        H: Fn(&mut T, T),
    {
        if self.is_empty() {
            *self = other;
        } else if !other.is_empty() {
            // normalized order allows to safely remove other nodes later
            if !other.is_normalized() {
                other.normalize();
            }

            // iterate over all other nodes in depth-first order
            let mut other = other.0.into_iter();
            // the node id of the next extracted other node
            let mut other_id = NodeId::default();

            // get the other root node and merge it
            let TreeNode {
                value,
                parent,
                children,
            } = other.next().unwrap();

            other_id.inc();

            let root_id = NodeId::default();
            assert_eq!(root_id, parent);
            debug_assert!(can_merge_node(&self[root_id].value, &value));
            merge_node(&mut self[root_id].value, value);

            // stack contains parent node id and other children ids to be adopted
            let mut stack = Vec::new();
            stack.push((root_id, children.into_iter()));

            while let Some((parent_id, mut child_ids)) = stack.pop() {
                if let Some(child_id) = child_ids.next() {
                    // next unmerged other child id to be adopted by parent
                    debug_assert_eq!(other_id, child_id);
                    // this is the other child node with child_id
                    let other_child = other.next().unwrap();
                    other_id.inc();
                    // save the rest of children ids for later
                    stack.push((parent_id, child_ids));

                    // find a parent's child to merge the other child into
                    if let Some(child_id) = self.find_unique_child(parent_id, |child_value| {
                        can_merge_node(child_value, &other_child.value)
                    }) {
                        // found a parent's child
                        let TreeNode {
                            value,
                            parent: _,
                            children,
                        } = other_child;
                        merge_node(&mut self[child_id].value, value);
                        // recursive step, child becomes parent, continue merging child's children
                        stack.push((child_id, children.into_iter()));
                    } else {
                        // not found, just move the whole branch using the normalized property
                        other_id = self.rebase_branch(parent_id, other_child, &mut other, other_id);
                    }
                }
            }
        }
    }
}
