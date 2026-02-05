// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use futures::Stream;
use iota_grpc_types::{
    field::{FieldMaskTree, FieldMaskUtil},
    google::rpc::bad_request::FieldViolation,
    v0::{
        error_reason::ErrorReason,
        ledger_service::{GetObjectsRequest, GetObjectsResponse, ObjectResult, object_result},
        object::Object,
    },
};
use iota_types::base_types::ObjectID;
use prost::Message;
use prost_types::FieldMask;

use crate::{
    constants::validate_max_message_size,
    error::{ObjectNotFoundError, RpcError},
    merge::Merge,
    types::{GrpcReader, ObjectsStreamResult},
};

pub const READ_MASK_DEFAULT: &str = crate::field_mask!(
    "reference.object_id",
    "reference.version",
    "reference.digest",
);

type ValidationResult = Result<(Vec<(ObjectID, Option<u64>)>, FieldMaskTree), RpcError>;

pub fn validate_get_object_requests(
    requests: Vec<(Option<String>, Option<u64>)>,
    read_mask: Option<FieldMask>,
) -> ValidationResult {
    let read_mask = {
        let read_mask = read_mask.unwrap_or_else(|| FieldMask::from_str(READ_MASK_DEFAULT));
        read_mask.validate::<Object>().map_err(|path| {
            FieldViolation::new("read_mask")
                .with_description(format!("invalid read_mask path: {path}"))
                .with_reason(ErrorReason::FieldInvalid)
        })?;
        FieldMaskTree::from(read_mask)
    };
    let requests = requests
        .into_iter()
        .enumerate()
        .map(|(idx, (object_id, version))| {
            let object_id = object_id
                .as_ref()
                .ok_or_else(|| {
                    FieldViolation::new("object_id")
                        .with_reason(ErrorReason::FieldMissing)
                        .nested_at("requests", idx)
                })?
                .parse()
                .map_err(|e| {
                    FieldViolation::new("object_id")
                        .with_description(format!("invalid object_id: {e}"))
                        .with_reason(ErrorReason::FieldInvalid)
                        .nested_at("requests", idx)
                })?;
            Ok((object_id, version))
        })
        .collect::<Result<_, RpcError>>()?;
    Ok((requests, read_mask))
}

#[tracing::instrument(skip(reader))]
pub(crate) fn get_objects(
    reader: GrpcReader,
    GetObjectsRequest {
        requests,
        read_mask,
        max_message_size_bytes,
        ..
    }: GetObjectsRequest,
) -> Result<impl Stream<Item = ObjectsStreamResult> + Send, RpcError> {
    let requests = requests
        .map(|r| r.requests)
        .unwrap_or_default()
        .into_iter()
        .map(|req| {
            let object_ref = req.object_ref;
            (
                object_ref.as_ref().and_then(|r| r.object_id.clone()),
                object_ref.and_then(|r| r.version),
            )
        })
        .collect();
    let (requests, read_mask) = validate_get_object_requests(requests, read_mask)?;

    // Validate and set max_message_size
    let max_message_size = validate_max_message_size(max_message_size_bytes.map(|v| v as u64))?;

    // Create lazy stream that fetches and batches objects on-demand
    Ok(crate::create_batching_stream!(
        requests.into_iter(),
        (object_id, version),
        {
            // Lazily fetch the object only when needed
            let object_result = match get_object_impl(&reader, object_id, version, &read_mask) {
                Ok(object) => ObjectResult {
                    result: Some(object_result::Result::Object(object)),
                },
                Err(error) => ObjectResult {
                    result: Some(object_result::Result::Error(error.into_status_proto())),
                },
            };

            let object_size = object_result.encoded_len();
            (object_result, object_size)
        },
        max_message_size,
        GetObjectsResponse,
        objects,
        has_next
    ))
}

#[tracing::instrument(skip(reader))]
fn get_object_impl(
    reader: &GrpcReader,
    object_id: ObjectID,
    version: Option<u64>,
    read_mask: &FieldMaskTree,
) -> Result<Object, RpcError> {
    let object = if let Some(version) = version {
        reader
            .get_object_by_key(&object_id, version.into())
            .ok_or_else(|| ObjectNotFoundError::new_with_version(object_id, version))?
    } else {
        reader
            .get_object(&object_id)
            .ok_or_else(|| ObjectNotFoundError::new(object_id))?
    };

    Object::merge_from(object, read_mask).map_err(|e| {
        RpcError::new(
            tonic::Code::Internal,
            format!("Failed to build object response: {e}"),
        )
    })
}
