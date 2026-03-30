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
/// Accepts string **literals** only (for compile-time concatenation via
/// `concat!`). To combine pre-defined constants at runtime, use
/// [`field_masks_merge!`] instead.
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

/// Normalizes a comma-separated field mask by removing paths that are
/// subsumed by broader (ancestor) paths.
///
/// A path `"a.b"` is subsumed by `"a"` because requesting `"a"` already
/// includes all of its sub-fields. Exact duplicates are also removed.
///
/// # Examples
/// ```
/// use iota_grpc_types::field_mask_normalize;
///
/// assert_eq!(field_mask_normalize("effects,effects.bcs"), "effects");
/// assert_eq!(field_mask_normalize("effects.bcs,effects"), "effects");
/// assert_eq!(field_mask_normalize("a,b.c,b"), "a,b");
/// ```
pub fn field_mask_normalize(mask: &str) -> String {
    let mut paths: Vec<&str> = mask.split(',').filter(|s| !s.is_empty()).collect();
    // Sort by length so broader (shorter) paths are processed first.
    paths.sort_by_key(|p| p.len());

    let mut result: Vec<&str> = Vec::new();
    for path in paths {
        let subsumed = result.iter().any(|&kept| {
            path == kept
                || (path.starts_with(kept) && path.as_bytes().get(kept.len()) == Some(&b'.'))
        });
        if !subsumed {
            result.push(path);
        }
    }
    result.join(",")
}

/// Merges multiple read mask expressions into a single comma-separated
/// [`String`], normalizing overlapping paths.
///
/// Unlike [`field_mask!`], this macro works with any expression that
/// evaluates to `&str`, including `const` values from
/// [`read_masks`](crate::read_masks). The result is a heap-allocated
/// `String` suitable for passing to the client's `read_mask` parameter
/// (e.g. `Some(&mask)`).
///
/// Overlapping paths are normalized: a broader path subsumes all of its
/// sub-paths. For example, `"effects"` and `"effects.bcs"` are merged into
/// just `"effects"`.
///
/// # Examples
/// ```
/// use iota_grpc_types::{field_masks_merge, read_masks::*};
///
/// let mask = field_masks_merge!(CHECKPOINT_RESPONSE_SUMMARY, CHECKPOINT_RESPONSE_CONTENTS,);
/// assert_eq!(mask, "checkpoint.summary,checkpoint.contents");
/// ```
///
/// Broader paths subsume narrower ones:
/// ```
/// use iota_grpc_types::field_masks_merge;
///
/// let mask = field_masks_merge!("effects", "effects.bcs");
/// assert_eq!(mask, "effects");
/// ```
#[macro_export]
macro_rules! field_masks_merge {
    ($mask:expr $(,)?) => {
        $crate::field_mask_normalize($mask)
    };
    ($first:expr, $($rest:expr),+ $(,)?) => {{
        let parts: &[&str] = &[$first, $($rest),+];
        $crate::field_mask_normalize(&parts.join(","))
    }};
}

// Re-export google namespace
pub mod google {
    pub use super::proto::google::*;
}

// Re-export under v1 namespace
pub mod v1 {
    pub use super::proto::iota::grpc::v1::*;
}
