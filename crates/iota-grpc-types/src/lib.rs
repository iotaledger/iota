// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! gRPC-specific versioned types for forward compatibility.
//!
//! These types provide versioning for gRPC streaming while positioning
//! for future core type evolution. When core types themselves
//! need versioning, these wrappers will evolve naturally.

pub mod field;
pub mod headers;
pub mod proto;

// Re-export google namespace
pub mod google {
    pub use super::proto::google::*;
}

// Re-export under v0 namespace
pub mod v0 {
    pub use super::proto::iota::grpc::v0::*;
}
