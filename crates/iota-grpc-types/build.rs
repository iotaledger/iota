// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

fn main() {
    tonic_build::configure()
        .compile_protos(
            &[
                "proto/iota/grpc/v0/common.proto",
                "proto/iota/grpc/v0/checkpoint.proto",
                "proto/iota/grpc/v0/event.proto",
            ],
            &["proto/iota/grpc/v0/"],
        )
        .unwrap();
}
