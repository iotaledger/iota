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
pub mod read_masks;

/// Joins field names with commas to build a read mask string constant.
///
/// # Example
/// ```
/// use iota_grpc_types::field_mask;
///
/// const MASK: &str = field_mask!("transaction.digest", "effects.bcs");
/// assert_eq!(MASK, "transaction.digest,effects.bcs");
/// ```
#[macro_export]
macro_rules! field_mask {
    ($field:literal) => {
        $field
    };
    ($first:literal, $($rest:literal),+ $(,)?) => {
        concat!($first, ",", $crate::field_mask!($($rest),+))
    };
}

// Re-export google namespace
pub mod google {
    pub use super::proto::google::*;
}

// Re-export under v0 namespace
pub mod v0 {
    pub use super::proto::iota::grpc::v0::*;
}
