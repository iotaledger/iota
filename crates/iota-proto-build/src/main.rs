// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
// Proto build tool for generating gRPC types with field constants

use std::{collections::HashMap, path::PathBuf};

use prost_types::FileDescriptorSet;

mod generate_fields;

fn main() {
    let root_dir = PathBuf::from(std::env!("CARGO_MANIFEST_DIR"));

    let proto_dir = root_dir
        .join("../iota-grpc-types/proto/iota/grpc/v0")
        .canonicalize()
        .unwrap();
    let out_dir = root_dir
        .join("../iota-grpc-types/src/proto_generated")
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

    let mut fds = protox::Compiler::new(std::slice::from_ref(&proto_dir))
        .unwrap()
        .include_source_info(true)
        .include_imports(true)
        .open_files(&proto_files)
        .unwrap()
        .file_descriptor_set();
    // Sort files by name to have deterministic codegen output
    fds.file.sort_by(|a, b| a.name.cmp(&b.name));

    tonic_prost_build::configure()
        .build_client(true)
        .build_server(true)
        .out_dir(&out_dir)
        .compile_protos(&proto_files, std::slice::from_ref(&proto_dir))
        .unwrap();

    // Add IOTA license headers to tonic-generated files and fix clippy warnings
    for entry in std::fs::read_dir(&out_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

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

            // Fix clippy::module_inception warning for nested modules with same name
            // This pattern appears in generated protobuf files like:
            //   pub struct Foo { ... }
            //   pub mod foo { ... }
            // We need to add #[allow(clippy::module_inception)] before such modules
            if content.contains("/// Nested message and enum types in") {
                content = content.replace(
                    "/// Nested message and enum types in",
                    "#[allow(clippy::module_inception)]\n/// Nested message and enum types in",
                );
            }

            std::fs::write(&path, content).unwrap();
        }
    }

    // Group files by package for field info generation
    let mut packages: HashMap<String, FileDescriptorSet> = HashMap::new();
    for mut file in fds.file {
        // Clear source code info as it's not needed for field generation
        file.source_code_info = None;
        packages
            .entry(file.package().to_owned())
            .or_default()
            .file
            .push(file);
    }

    // Generate field constants and MessageFields impls
    generate_fields::generate_field_info(&packages, &out_dir);

    let status = std::process::Command::new("git")
        .arg("diff")
        .arg("--exit-code")
        .arg("--")
        .arg(out_dir)
        .status();
    match status {
        Ok(status) if !status.success() => panic!("You should commit the protobuf files"),
        Err(error) => panic!("failed to run `git diff`: {error}"),
        Ok(_) => {}
    }
}