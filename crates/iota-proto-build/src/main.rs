// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
// Proto build tool for generating gRPC types with field constants

use std::{collections::HashMap, path::PathBuf};

use crate::{
    codegen::generate_fields::FileDescriptorWithPackageVersion, message_graph::DescriptorGraph,
};

mod codegen;
mod comments;
mod context;
mod dependency_graph;
mod ident;
mod message_graph;

fn main() {
    let root_dir = PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));

    let proto_dir = root_dir
        .join("../iota-grpc-types/proto")
        .canonicalize()
        .unwrap();
    let out_dir = root_dir
        .join("../iota-grpc-types/src/proto/generated")
        .canonicalize()
        .unwrap();

    let proto_ext = std::ffi::OsStr::new("proto");
    let proto_files = walkdir::WalkDir::new(&proto_dir)
        .into_iter()
        .filter_map(|entry| {
            (|| {
                let entry = entry?;
                if entry.file_type().is_dir() {
                    return Ok(None);
                }

                let path = entry.into_path();
                if path.extension() != Some(proto_ext) {
                    return Ok(None);
                }

                Ok(Some(path))
            })()
            .transpose()
        })
        .collect::<Result<Vec<_>, walkdir::Error>>()
        .unwrap();

    let mut compiler_init = protox::Compiler::new(std::slice::from_ref(&proto_dir)).unwrap();
    let compiler = compiler_init
        .include_source_info(true)
        .include_imports(true)
        .open_files(&proto_files)
        .unwrap();

    let descriptor_pool = compiler.descriptor_pool();
    let mut fds = compiler.file_descriptor_set();

    // Sort files by name to have deterministic codegen output
    fds.file.sort_by(|a, b| a.name.cmp(&b.name));

    // Define boxing configuration for prost-build
    // These fields are boxed by prost in the generated structs
    let boxed_types_prost = vec![
        ".iota.grpc.v1.filter.EventFilter.negation".to_string(),
        ".iota.grpc.v1.filter.TransactionFilter.negation".to_string(),
        ".iota.grpc.v1.filter.NotEventFilter.filter".to_string(),
        ".iota.grpc.v1.filter.NotTransactionFilter.filter".to_string(),
        ".iota.grpc.v1.types.TypeTag.vector_tag".to_string(),
    ];

    // for field info and accessor generation
    let boxed_types_field_info = vec![
        ".iota.grpc.v1.filter.AllEventFilter.filters".to_string(),
        ".iota.grpc.v1.filter.AnyEventFilter.filters".to_string(),
        ".iota.grpc.v1.filter.NotEventFilter.filter".to_string(),
        ".iota.grpc.v1.filter.AllTransactionFilter.filters".to_string(),
        ".iota.grpc.v1.filter.AnyTransactionFilter.filters".to_string(),
        ".iota.grpc.v1.filter.NotTransactionFilter.filter".to_string(),
        ".iota.grpc.v1.types.TypeTagVector.inner_type".to_string(),
    ];

    // for accessor generation - includes both boxed proto fields and fields where
    // we want the accessor to accept boxed types for ergonomics
    let boxed_types_accessor = vec![
        ".iota.grpc.v1.filter.EventFilter.negation".to_string(),
        ".iota.grpc.v1.filter.TransactionFilter.negation".to_string(),
        ".iota.grpc.v1.filter.NotEventFilter.filter".to_string(),
        ".iota.grpc.v1.filter.NotTransactionFilter.filter".to_string(),
        ".iota.grpc.v1.types.TypeTag.vector_tag".to_string(),
    ];

    let mut tonic_prost_builder = tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .bytes(".");

    // apply all boxed types
    for boxed_type in &boxed_types_prost {
        tonic_prost_builder = tonic_prost_builder.boxed(boxed_type);
    }

    tonic_prost_builder
        .message_attribute(".iota.grpc", "#[non_exhaustive]")
        .enum_attribute(".iota.grpc", "#[non_exhaustive]")
        .btree_map(".")
        .out_dir(&out_dir)
        .compile_protos(&proto_files, std::slice::from_ref(&proto_dir))
        .unwrap();

    let google_import_regex = regex::Regex::new(r"(?:super::)+google").unwrap();

    // Add IOTA license headers to tonic-generated files and fix clippy warnings
    for entry in std::fs::read_dir(&out_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        // ignore google proto files
        if path.to_str().unwrap().contains("google") {
            continue;
        }

        if path.extension().and_then(|s| s.to_str()) == Some("rs")
            && !path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("field_info")
        {
            let mut content = std::fs::read_to_string(&path).unwrap();

            // Add license header if missing
            if !content.starts_with("// Copyright") {
                content = format!(
                    "// Copyright (c) Mysten Labs, Inc.\n// Modifications Copyright (c) 2025 IOTA Stiftung\n// SPDX-License-Identifier: Apache-2.0\n\n{content}"
                );
            }

            // Replace all occurrences of super::google with crate::google (any number of
            // super::)
            content = google_import_regex
                .replace_all(&content, "crate::google")
                .to_string();
            std::fs::write(&path, content).unwrap();
        }
    }

    // Setup for extended codegen
    // Parse proto files to extract accessor annotations
    let accessor_map = codegen::accessor_config::parse_proto_accessors_from_pool(&descriptor_pool);

    let extern_paths = context::extern_paths::ExternPaths::new(&[], true).unwrap();
    let files = fds.file.clone().into_iter().collect::<Vec<_>>();
    let graph = DescriptorGraph::new(files.iter());
    let context = context::Context::new(extern_paths, graph);
    codegen::accessors::generate_accessors(
        &context,
        &out_dir,
        &boxed_types_prost,
        &boxed_types_accessor,
        &accessor_map,
    );

    // Group files by package for field info generation
    let mut packages: HashMap<String, FileDescriptorWithPackageVersion> = HashMap::new();
    for mut file in fds.file {
        // Clear source code info as it's not needed for field generation
        file.source_code_info = None;

        let package = packages.entry(file.package().to_owned()).or_default();

        package.fd_set.file.push(file.clone());

        // get the version from the file path
        package.version = file
            .name
            .as_ref()
            .and_then(|name| {
                // Extract version from file path like "iota/grpc/v1/types.proto" -> "v1"
                name.split('/').find(|part| {
                    part.starts_with('v')
                        && part.len() > 1
                        && part[1..].chars().all(|c| c.is_ascii_digit())
                })
            })
            .unwrap_or("v1")
            .to_string();
    }

    // Parse transparent message options from the descriptor pool
    let transparent_messages =
        codegen::generate_fields::parse_transparent_messages_from_pool(&descriptor_pool);

    // Generate field constants and MessageFields impls
    codegen::generate_fields::generate_field_info(
        &packages,
        &out_dir,
        &boxed_types_field_info,
        &transparent_messages,
    );

    let status = std::process::Command::new("git")
        .arg("diff")
        .arg("--exit-code")
        .arg("--")
        .arg(out_dir)
        .status();
    match status {
        Ok(status) if !status.success() => {
            eprintln!("Generated protobuf files have uncommitted changes. Please commit them.");
            std::process::exit(2); // Custom exit code for uncommitted changes
        }
        Err(error) => panic!("failed to run `git diff`: {error}"),
        Ok(_) => {}
    }
}
