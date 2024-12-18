// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

fn main() {
    cynic_codegen::register_schema("iota")
        .from_sdl_file("../iota-graphql-rpc/schema.graphql")
        .unwrap()
        .as_default()
        .unwrap();
}
