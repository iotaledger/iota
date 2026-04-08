// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil, MessageFields},
    google::rpc::bad_request::FieldViolation,
    v1::{
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

/// Validate and clamp a `page_size` parameter.
///
/// - `None` or `Some(0)` → `default`
/// - `Some(n)` where `n > max` → `max`
/// - Otherwise → `n`
pub(crate) fn validate_page_size(page_size: Option<u32>, default: u32, max: u32) -> usize {
    match page_size {
        None | Some(0) => default as usize,
        Some(n) => n.min(max) as usize,
    }
}

/// Decode and validate a BCS-encoded page token.
///
/// Returns `Ok(None)` when no token is provided.
/// Returns `Err(InvalidArgument)` when the token cannot be decoded.
pub(crate) fn decode_page_token<T: serde::de::DeserializeOwned>(
    token: &Option<prost::bytes::Bytes>,
) -> Result<Option<T>, RpcError> {
    match token {
        None => Ok(None),
        Some(bytes) => bcs::from_bytes(bytes).map(Some).map_err(|_| {
            FieldViolation::new("page_token")
                .with_description("invalid page_token")
                .with_reason(ErrorReason::FieldInvalid)
                .into()
        }),
    }
}

/// Encode a page token as BCS bytes.
pub(crate) fn encode_page_token<T: serde::Serialize>(token: &T) -> Vec<u8> {
    bcs::to_bytes(token).expect("page token serialization cannot fail")
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

/// Return an error indicating the page token does not match the current
/// request parameters (e.g. owner, parent, or package ID changed).
pub(crate) fn page_token_mismatch() -> RpcError {
    FieldViolation::new("page_token")
        .with_description("page_token does not match request parameters")
        .with_reason(ErrorReason::FieldInvalid)
        .into()
}

/// Convert an `ObjectID` to a gRPC `ObjectId` proto.
pub(crate) fn object_id_proto(id: &ObjectID) -> ObjectId {
    ObjectId::default().with_object_id(id.as_ref().to_vec())
}
