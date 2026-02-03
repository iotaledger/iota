// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

use prost_types::{DescriptorProto, field_descriptor_proto::Type};

// Dependency graph for tracking message dependencies
#[derive(Debug, Default)]
pub(crate) struct DependencyGraph {
    // Maps message name to set of messages it depends on
    dependencies: HashMap<String, HashSet<String>>,
    // Maps message name to its full qualified name
    full_names: HashMap<String, String>,
}

impl DependencyGraph {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_dependency(&mut self, from_message: &str, to_message: &str) {
        self.dependencies
            .entry(from_message.to_string())
            .or_default()
            .insert(to_message.to_string());
    }

    pub(crate) fn set_full_name(&mut self, message: &str, full_name: &str) {
        self.full_names
            .insert(message.to_string(), full_name.to_string());
    }

    // Check if there's a circular dependency between two messages
    pub(crate) fn has_circular_dependency(&self, from_message: &str, to_message: &str) -> bool {
        // If to_message can reach back to from_message, we have a cycle
        self.can_reach(to_message, from_message, &mut HashSet::new())
    }

    // Check if we can reach target from source (used for cycle detection)
    fn can_reach(&self, source: &str, target: &str, visited: &mut HashSet<String>) -> bool {
        if source == target {
            return true;
        }

        if visited.contains(source) {
            return false; // Already visited this node
        }

        visited.insert(source.to_string());

        if let Some(deps) = self.dependencies.get(source) {
            for dep in deps {
                if self.can_reach(dep, target, visited) {
                    return true;
                }
            }
        }

        false
    }
}

// Helper function to build dependency graph for all messages
pub(crate) fn build_dependency_graph(
    messages: &[DescriptorProto],
    package: &str,
    prefix: &str,
) -> DependencyGraph {
    let mut graph = DependencyGraph::new();

    for message in messages {
        let full_name = if prefix.is_empty() {
            format!("{}.{}", package, message.name())
        } else {
            format!("{}.{}", prefix, message.name())
        };

        graph.set_full_name(message.name(), &full_name);

        // Add dependencies for each message field
        for field in &message.field {
            if matches!(field.r#type(), Type::Message) {
                let field_message_name = field.type_name().split('.').next_back().unwrap();
                graph.add_dependency(message.name(), field_message_name);
            }
        }

        // Recursively handle nested messages
        let nested_graph = build_dependency_graph(&message.nested_type, package, &full_name);

        // Merge nested graph dependencies
        graph
            .dependencies
            .extend(nested_graph.dependencies.into_iter());

        //// Merge nested graph full names
        graph.full_names.extend(nested_graph.full_names);
    }

    graph
}
