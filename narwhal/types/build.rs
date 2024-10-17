// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, path::PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() -> Result<()> {
    let out_dir = if env::var("DUMP_GENERATED_GRPC").is_ok() {
        PathBuf::from("")
    } else {
        PathBuf::from(env::var("OUT_DIR")?)
    };

    let codec_path = "tonic::codec::ProstCodec";

    let service = tonic_build::manual::Service::builder()
        .name("Transactions")
        .package("narwhal")
        .method(
            tonic_build::manual::Method::builder()
                .name("submit_transaction")
                .route_name("SubmitTransaction")
                .input_type("crate::proto::narwhal::Transaction")
                .output_type("crate::proto::narwhal::Empty")
                .codec_path(codec_path)
                .build(),
        )
        .method(
            tonic_build::manual::Method::builder()
                .name("submit_transaction_stream")
                .route_name("SubmitTransactionStream")
                .input_type("crate::proto::narwhal::Transaction")
                .output_type("crate::proto::narwhal::Empty")
                .codec_path(codec_path)
                .client_streaming()
                .build(),
        )
        .build();

    tonic_build::manual::Builder::new()
        .out_dir(&out_dir)
        .compile(&[service]);

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=DUMP_GENERATED_GRPC");

    Ok(())
}
