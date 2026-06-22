// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/dag_visualizer.proto"], &["proto"])?;
    Ok(())
}
