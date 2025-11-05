// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC-specific versioned types for forward compatibility.
//!
//! These types provide versioning for gRPC streaming while positioning
//! for future core type evolution. When core types themselves
//! need versioning, these wrappers will evolve naturally.

// Generated protobuf modules with field constants
mod proto_generated {
    pub mod common {
        include!("proto_generated/iota.grpc.v0.common.rs");
        include!("proto_generated/iota.grpc.v0.common.field_info.rs");
    }
    pub mod checkpoints {
        include!("proto_generated/iota.grpc.v0.checkpoints.rs");
        include!("proto_generated/iota.grpc.v0.checkpoints.field_info.rs");
    }
    pub mod events {
        include!("proto_generated/iota.grpc.v0.events.rs");
        include!("proto_generated/iota.grpc.v0.events.field_info.rs");
    }
}

// Re-export under v0 namespace
pub mod v0 {
    pub use super::proto_generated::*;
}

pub mod bcs;
pub mod checkpoints;
pub mod events;
pub mod field;
