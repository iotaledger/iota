// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC-specific versioned types for forward compatibility.
//!
//! These types provide versioning for gRPC streaming while positioning
//! for future core type evolution. When core types themselves
//! need versioning, these wrappers will evolve naturally.

mod proto {
    pub mod iota {
        pub mod grpc {
            pub mod v0 {
                pub mod common {
                    tonic::include_proto!("iota.grpc.v0.common");
                }
                pub mod checkpoints {
                    tonic::include_proto!("iota.grpc.v0.checkpoints");
                }
                pub mod events {
                    tonic::include_proto!("iota.grpc.v0.events");
                }
                pub mod transactions {
                    tonic::include_proto!("iota.grpc.v0.transactions");
                }
                pub mod read {
                    tonic::include_proto!("iota.grpc.v0.read");
                }
                pub mod write {
                    tonic::include_proto!("iota.grpc.v0.write");
                }
            }
        }
    }
}

pub use proto::iota::grpc::v0;

pub mod bcs;
pub mod checkpoints;
pub mod events;
pub mod transactions;
