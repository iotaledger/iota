// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
use itertools::Itertools;

use crate::{
    ident::{to_snake, to_upper_camel},
    message_graph::DescriptorGraph,
};

pub mod extern_paths;

pub struct Context {
    extern_paths: extern_paths::ExternPaths,
    graph: DescriptorGraph,
}

impl Context {
    pub fn new(extern_paths: extern_paths::ExternPaths, graph: DescriptorGraph) -> Self {
        Self {
            extern_paths,
            graph,
        }
    }

    // Used to check if a path is external and should be skipped
    pub fn is_extern(&self, fq_proto_name: &str) -> bool {
        self.extern_paths.resolve_ident(fq_proto_name).is_some()
    }

    // Resolves protobuf type to a rust path relative to package/scope.
    pub fn resolve_ident(&self, package: &str, pb_ident: &str) -> String {
        // protoc should always give fully qualified identifiers.
        assert_eq!(".", &pb_ident[..1]);

        if let Some(proto_ident) = self.extern_paths.resolve_ident(pb_ident) {
            return proto_ident;
        }

        let mut local_path = package.split('.').peekable();

        // If no package is specified the start of the package name will be '.'
        // and split will return an empty string ("") which breaks resolution
        // The fix to this is to ignore the first item if it is empty.
        if local_path.peek().is_some_and(|s| s.is_empty()) {
            local_path.next();
        }

        let mut ident_path = pb_ident[1..].split('.');
        let ident_type = ident_path.next_back().unwrap();
        let mut ident_path = ident_path.peekable();

        // Skip path elements in common.
        while local_path.peek().is_some() && local_path.peek() == ident_path.peek() {
            local_path.next();
            ident_path.next();
        }

        local_path
            .map(|_| "super".to_string())
            .chain(ident_path.map(to_snake))
            .chain(std::iter::once(to_upper_camel(ident_type)))
            .join("::")
    }

    pub fn graph(&self) -> &DescriptorGraph {
        &self.graph
    }
}
