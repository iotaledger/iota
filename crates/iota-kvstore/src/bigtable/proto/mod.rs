// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// for generated code we ignore all clippy warnings
#![allow(clippy::all)]
mod google {

    mod rpc {
        tonic::include_proto!("google.rpc");
    }

    mod r#type {
        tonic::include_proto!("google.r#type");
    }

    pub mod bigtable {
        pub mod v2 {
            tonic::include_proto!("google.bigtable.v2");
        }
    }
}

pub use google::bigtable;
