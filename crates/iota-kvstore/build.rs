// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

const PROTOS: &[&str] = &[
    "src/bigtable/proto/google/api/client.proto",
    "src/bigtable/proto/google/api/field_behavior.proto",
    "src/bigtable/proto/google/api/launch_stage.proto",
    "src/bigtable/proto/google/api/resource.proto",
    "src/bigtable/proto/google/api/routing.proto",
    "src/bigtable/proto/google/bigtable/v2/bigtable.proto",
    "src/bigtable/proto/google/bigtable/v2/data.proto",
    "src/bigtable/proto/google/bigtable/v2/request_stats.proto",
    "src/bigtable/proto/google/bigtable/v2/types.proto",
    "src/bigtable/proto/google/rpc/status.proto",
];

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    for proto in PROTOS {
        println!("cargo:rerun-if-changed={proto}");
    }

    tonic_build::configure()
        // the server is on the google side, we don't need code generation for it.
        .build_server(false)
        .compile_protos(PROTOS, &["src/bigtable/proto"])?;

    Ok(())
}
