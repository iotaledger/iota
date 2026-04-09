// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use prost_types::FieldMask;

use super::{
    FIELD_PATH_SEPARATOR, FIELD_PATH_WILDCARD, FIELD_SEPARATOR, FieldMaskUtil, is_valid_path,
};

#[derive(Clone, Debug, Default)]
pub struct FieldMaskTree {
    wildcard: bool,
    root: Node,
}

#[derive(Clone, Debug, Default)]
struct Node {
    children: BTreeMap<String, Node>,
}

impl FieldMaskTree {
    pub fn new_wildcard() -> Self {
        Self {
            wildcard: true,
            root: Default::default(),
        }
    }

    pub fn add_field_path(&mut self, path: &str) -> &mut Self {
        if self.wildcard || !is_valid_path(path) {
            return self;
        }

        if path == FIELD_PATH_WILDCARD {
            self.wildcard = true;
            self.root.children.clear();
            return self;
        }

        let root = std::ptr::from_ref(&self.root);
        let mut node = &mut self.root;
        let mut create_new_branch = false;
        for component in path.split(FIELD_SEPARATOR) {
            if !create_new_branch && !std::ptr::eq(root, node) && node.children.is_empty() {
                return self;
            }

            node = node
                .children
                .entry(component.to_owned())
                .or_insert_with(|| {
                    create_new_branch = true;
                    Node::default()
                });
        }

        node.children.clear();
        self
    }

    pub fn from_field_mask(mask: &FieldMask) -> Self {
        let mut tree = Self::default();
        for path in &mask.paths {
            tree.add_field_path(path);
        }
        tree
    }

    pub fn to_field_mask(&self) -> FieldMask {
        if self.root.children.is_empty() {
            return FieldMask::default();
        }

        let mut paths = Vec::new();
        Self::collect_field_paths(&self.root, &mut String::new(), &mut paths);
        FieldMask { paths }
    }

    fn collect_field_paths(node: &Node, path: &mut String, paths: &mut Vec<String>) {
        if node.children.is_empty() {
            paths.push(path.clone());
            return;
        }

        let parent_path_len = path.len();
        for (part, child) in node.children.iter() {
            if path.is_empty() {
                path.push_str(part);
            } else {
                path.push(FIELD_SEPARATOR);
                path.push_str(part);
            };
            Self::collect_field_paths(child, path, paths);
            path.truncate(parent_path_len);
        }
    }

    /// Checks if the provided path is contained in this FieldMaskTree.
    ///
    /// A path is considered a match and contained by this tree if it is a
    /// prefix for any contained paths, including if it is an exact match.
    ///
    /// ```
    /// # use iota_grpc_types::field::FieldMaskTree;
    /// let mut tree = FieldMaskTree::default();
    /// tree.add_field_path("foo.bar");
    ///
    /// assert!(tree.contains("foo"));
    /// assert!(tree.contains("foo.bar"));
    /// assert!(!tree.contains("foo.baz"));
    /// ```
    pub fn contains<P: AsRef<str>>(&self, path: P) -> bool {
        let path = path.as_ref();

        if path.is_empty() {
            return false;
        }

        if self.wildcard {
            return true;
        }

        let mut node = &self.root;
        for component in path.split(FIELD_SEPARATOR) {
            // If this isn't the root node, and there are no sub-paths, then this path has
            // been matched and we can return a hit
            if !std::ptr::eq(node, &self.root) && node.children.is_empty() {
                return true;
            }

            if let Some(child) = node.children.get(component) {
                node = child;
            } else {
                return false;
            }
        }

        // We found a matching node for this path. This node may be empty or have leaf
        // children. In either case the provided patch is a "match" and is
        // contained by this tree.
        true
    }

    /// Returns the explicit map keys requested in this subtree, or `None` if
    /// this tree is a wildcard (no explicit key filter).
    ///
    /// When a user specifies explicit map keys via a field mask (e.g.
    /// `attributes.max_tx_gas`), this method returns an iterator over those
    /// key names. When the tree is a wildcard, `None` is returned to indicate
    /// "no filter — return all (or none, per policy)".
    pub fn map_keys(&self) -> Option<impl Iterator<Item = &str>> {
        if self.wildcard {
            None
        } else {
            Some(self.root.children.keys().map(String::as_str))
        }
    }

    /// Determines which entries of an inner map field should be included.
    ///
    /// Call this on the subtree rooted at a **map-wrapper message field**
    /// (e.g. the subtree at `feature_flags` or `attributes` inside
    /// `ProtocolConfig`) and pass the name of the **inner map field** (e.g.
    /// `"flags"` or `"attributes"`).  The return value tells the caller what
    /// to put in the resulting map:
    ///
    /// - `None` — the wrapper itself was a wildcard *or* the inner field was
    ///   not explicitly requested → **omit the wrapper field entirely** (leave
    ///   it as `None` in the response so clients can distinguish "not
    ///   requested" from "empty").
    /// - `Some(None)` — the inner field was requested without specific keys
    ///   (e.g. `feature_flags.flags`) → include **all entries**.
    /// - `Some(Some(keys))` — only these specific keys were requested (e.g.
    ///   `feature_flags.flags.flag_a`) → include **only those entries**.
    pub fn map_field_filter(
        &self,
        inner_field_name: &str,
    ) -> Option<Option<std::collections::HashSet<String>>> {
        match self.map_keys() {
            // Wildcard at the wrapper level (e.g. bare "feature_flags") → empty
            None => None,
            Some(_) => match self.subtree(inner_field_name) {
                // Inner field not among the requested children → empty
                None => None,
                Some(inner) => match inner.map_keys() {
                    // Inner field requested without specific keys → all entries
                    None => Some(None),
                    // Specific keys requested
                    Some(keys) => Some(Some(keys.map(String::from).collect())),
                },
            },
        }
    }

    pub fn subtree<P: AsRef<str>>(&self, path: P) -> Option<Self> {
        let path = path.as_ref();

        if path.is_empty() {
            return None;
        }

        if self.wildcard {
            return Some(self.clone());
        }

        let mut node = &self.root;
        for component in path.split(FIELD_SEPARATOR) {
            if let Some(child) = node.children.get(component) {
                node = child;
            } else {
                return None;
            }
        }

        if std::ptr::eq(node, &self.root) {
            None
        } else {
            Some(Self {
                wildcard: node.children.is_empty(),
                root: node.clone(),
            })
        }
    }
}

impl From<FieldMask> for FieldMaskTree {
    fn from(mask: FieldMask) -> Self {
        Self::from_field_mask(&mask)
    }
}

impl From<FieldMaskTree> for FieldMask {
    fn from(tree: FieldMaskTree) -> Self {
        tree.to_field_mask()
    }
}

impl std::fmt::Display for FieldMaskTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        FieldMaskUtil::display(&self.to_field_mask()).fmt(f)
    }
}

impl std::str::FromStr for FieldMaskTree {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tree = Self::default();

        for path in s.split(FIELD_PATH_SEPARATOR) {
            tree.add_field_path(path);
        }

        Ok(tree)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_field_path() {
        let mut tree = FieldMaskTree::default();

        assert!(tree.to_string().is_empty());
        tree.add_field_path("");
        assert!(tree.to_string().is_empty());

        tree.add_field_path("foo");
        assert_eq!(tree.to_string(), "foo");
        // redundant path
        tree.add_field_path("foo");
        assert_eq!(tree.to_string(), "foo");

        tree.add_field_path("bar.baz");
        assert_eq!(tree.to_string(), "bar.baz,foo");

        // redundant sub-path
        tree.add_field_path("foo.bar");
        assert_eq!(tree.to_string(), "bar.baz,foo");

        // new sub-path
        tree.add_field_path("bar.quz");
        assert_eq!(tree.to_string(), "bar.baz,bar.quz,foo");

        // path that matches several existing sub-paths
        tree.add_field_path("bar");
        assert_eq!(tree.to_string(), "bar,foo");
    }

    #[test]
    fn test_contains() {
        let mut tree = FieldMaskTree::default();

        assert!(!tree.contains("foo"));
        assert!(!tree.contains("foo.bar"));

        tree.add_field_path("foo.bar");

        assert!(tree.contains("foo"));
        assert!(tree.contains("foo.bar"));
        assert!(!tree.contains("foo.baz"));
        assert!(!tree.contains("foobar"));
    }

    #[test]
    fn test_map_keys() {
        // Empty tree → no wildcard, no keys
        let tree = FieldMaskTree::default();
        let keys: Option<Vec<&str>> = tree.map_keys().map(|it| it.collect());
        assert_eq!(keys, Some(vec![]));

        // Wildcard tree → None
        let tree = FieldMaskTree::new_wildcard();
        assert!(tree.map_keys().is_none());

        // Tree with specific keys
        let mut tree = FieldMaskTree::default();
        tree.add_field_path("key_a");
        tree.add_field_path("key_b");
        let mut keys: Vec<&str> = tree.map_keys().unwrap().collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["key_a", "key_b"]);

        // Subtree map_keys: after subtree(), map_keys on the result
        let mut tree = FieldMaskTree::default();
        tree.add_field_path("attributes.max_tx_gas");
        tree.add_field_path("attributes.max_num_events");

        // The subtree at "attributes" should have the two keys
        let subtree = tree.subtree("attributes").unwrap();
        let mut keys: Vec<&str> = subtree.map_keys().unwrap().collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["max_num_events", "max_tx_gas"]);

        // Subtree when "attributes" is a leaf (wildcard-like subtree) → None
        let mut tree = FieldMaskTree::default();
        tree.add_field_path("attributes");
        let subtree = tree.subtree("attributes").unwrap();
        assert!(subtree.map_keys().is_none());
    }

    #[test]
    fn test_map_field_filter() {
        // Simulates: wrapper "feature_flags" contains inner map field "flags"

        // Case 1: bare wrapper (wildcard) → None (empty map)
        let mut tree = FieldMaskTree::default();
        tree.add_field_path("feature_flags");
        let subtree = tree.subtree("feature_flags").unwrap();
        assert!(subtree.map_field_filter("flags").is_none());

        // Case 2: inner map field requested without keys → Some(None) (all entries)
        let mut tree = FieldMaskTree::default();
        tree.add_field_path("feature_flags.flags");
        let subtree = tree.subtree("feature_flags").unwrap();
        assert!(matches!(subtree.map_field_filter("flags"), Some(None)));

        // Case 3: specific keys → Some(Some(keys))
        let mut tree = FieldMaskTree::default();
        tree.add_field_path("feature_flags.flags.flag_a");
        tree.add_field_path("feature_flags.flags.flag_c");
        let subtree = tree.subtree("feature_flags").unwrap();
        let filter = subtree.map_field_filter("flags");
        let mut keys: Vec<String> = filter.unwrap().unwrap().into_iter().collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["flag_a", "flag_c"]);

        // Case 4: inner field name not in the tree → None (empty map)
        let mut tree = FieldMaskTree::default();
        tree.add_field_path("feature_flags.other_field");
        let subtree = tree.subtree("feature_flags").unwrap();
        assert!(subtree.map_field_filter("flags").is_none());
    }
}
