// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

fn main() {
    tonic_build::configure()
        .compile_protos(&["proto/checkpoint.proto"], &["proto"])
        .unwrap();
}
