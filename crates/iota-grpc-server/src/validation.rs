// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil, MessageFields},
    google::rpc::bad_request::FieldViolation,
    v0::{
        error_reason::ErrorReason,
        types::{Address, ObjectId},
    },
};
use iota_types::base_types::{IotaAddress, ObjectID};
use prost_types::FieldMask;

use crate::error::RpcError;

/// Parse and validate a read mask, falling back to a default if not provided.
pub(crate) fn validate_read_mask<M: MessageFields>(
    read_mask: Option<FieldMask>,
    default: &str,
) -> Result<FieldMaskTree, RpcError> {
    let read_mask = read_mask.unwrap_or_else(|| FieldMask::from_str(default));
    read_mask.validate::<M>().map_err(|path| {
        FieldViolation::new("read_mask")
            .with_description(format!("invalid read_mask path: {path}"))
            .with_reason(ErrorReason::FieldInvalid)
    })?;
    Ok(FieldMaskTree::from(read_mask))
}

/// Validate and extract a required `ObjectId` proto field as an internal
/// `ObjectID`.
pub(crate) fn require_object_id(
    field: &Option<ObjectId>,
    field_name: &str,
) -> Result<ObjectID, RpcError> {
    field
        .as_ref()
        .ok_or_else(|| {
            FieldViolation::new(field_name)
                .with_description(format!("{field_name} is required"))
                .with_reason(ErrorReason::FieldMissing)
        })?
        .object_id()
        .map(Into::into)
        .map_err(|e| {
            FieldViolation::new(field_name)
                .with_description(format!("invalid {field_name}: {e}"))
                .with_reason(ErrorReason::FieldInvalid)
                .into()
        })
}

/// Validate and clamp a limit parameter.
///
/// - `None` → returns `default`
/// - `Some(0)` → clamped to `1` (zero is treated as "unset" in proto3)
/// - `Some(n)` where `n > max` → clamped to `max`
pub(crate) fn validate_limit(limit: Option<u32>, default: usize, max: usize) -> usize {
    assert!(
        default <= max,
        "default ({default}) must not exceed max ({max})"
    );
    limit.map(|l| (l as usize).clamp(1, max)).unwrap_or(default)
}

/// Validate and extract a required `Address` proto field as an internal
/// `IotaAddress`.
pub(crate) fn require_address(
    field: &Option<Address>,
    field_name: &str,
) -> Result<IotaAddress, RpcError> {
    field
        .as_ref()
        .ok_or_else(|| {
            FieldViolation::new(field_name)
                .with_description(format!("{field_name} is required"))
                .with_reason(ErrorReason::FieldMissing)
        })?
        .address()
        .map(Into::into)
        .map_err(|e| {
            FieldViolation::new(field_name)
                .with_description(format!("invalid {field_name}: {e}"))
                .with_reason(ErrorReason::FieldInvalid)
                .into()
        })
}

/// Collect an iterator of `anyhow::Result<T>` into a `Vec<T>`, mapping errors
/// to [`RpcError`] via its `From<anyhow::Error>` impl (which correctly detects
/// [`MissingIndexesError`] and maps it to `FailedPrecondition`).
pub(crate) fn collect_iter<T>(
    iter: impl Iterator<Item = anyhow::Result<T>>,
) -> Result<Vec<T>, RpcError> {
    iter.collect::<Result<Vec<_>, _>>().map_err(RpcError::from)
}

/// Convert an `ObjectID` to a gRPC `ObjectId` proto.
pub(crate) fn object_id_proto(id: &ObjectID) -> ObjectId {
    ObjectId::default().with_object_id(id.as_ref().to_vec())
}
