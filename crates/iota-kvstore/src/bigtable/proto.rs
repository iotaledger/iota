// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![allow(clippy::all)]
#[path = "proto"]
pub mod bigtable {
    #[path = "google.bigtable.v2.rs"]
    pub mod v2;
}

#[path = "proto/google.rpc.rs"]
pub mod rpc;
