// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::Stream;
use iota_grpc_types::{
    field::FieldMaskTree,
    google::rpc::bad_request::FieldViolation,
    read_masks::GET_OBJECTS_READ_MASK,
    v1::{
        error_reason::ErrorReason,
        ledger_service::{GetObjectsRequest, GetObjectsResponse, ObjectResult},
        object::Object,
        types::ObjectId,
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
    validation::validate_read_mask,
};

type ValidationResult = Result<(Vec<(ObjectID, Option<u64>)>, FieldMaskTree), RpcError>;

pub(crate) fn validate_get_object_requests(
    requests: Vec<(Option<ObjectId>, Option<u64>)>,
    read_mask: Option<FieldMask>,
) -> ValidationResult {
    let read_mask = validate_read_mask::<Object>(read_mask, GET_OBJECTS_READ_MASK)?;
    let requests = requests
        .into_iter()
        .enumerate()
        .map(|(idx, (object_id, version))| {
            let object_id: ObjectID = object_id
                .as_ref()
                .ok_or_else(|| {
                    FieldViolation::new("object_id")
                        .with_reason(ErrorReason::FieldMissing)
                        .nested_at("requests", idx)
                })?
                .object_id()
                .map(Into::into)
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

/// Available Read Mask Fields
///
/// The `get_objects` function supports the following `read_mask` fields to
/// control which data is included in the response:
///
/// ## Reference Fields
/// - `reference` - includes all reference fields
///   - `reference.object_id` - the ID of the object to fetch
///   - `reference.version` - the version of the object, which can be used to
///     fetch a specific historical version or the latest version if not
///     provided
///   - `reference.digest` - the digest of the object contents, which can be
///     used for integrity verification
///
/// ## Data Fields
/// - `bcs` - the full BCS-encoded object
#[tracing::instrument(skip(reader))]
pub(crate) fn get_objects(
    reader: Arc<GrpcReader>,
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
    let max_message_size = validate_max_message_size(max_message_size_bytes)?;

    // Create lazy stream that fetches and batches objects on-demand
    Ok(crate::create_batching_stream!(
        requests.into_iter(),
        (object_id, version),
        {
            // Lazily fetch the object only when needed
            let object_result = match get_object_impl(&reader, object_id, version, &read_mask) {
                Ok(object) => ObjectResult::default().with_object(object),
                Err(error) => ObjectResult::default().with_error(error.into_status_proto()),
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
            .get_object_by_key(&object_id, version.into())?
            .ok_or_else(|| ObjectNotFoundError::new_with_version(object_id, version))?
    } else {
        reader
            .get_object(&object_id)?
            .ok_or_else(|| ObjectNotFoundError::new(object_id))?
    };

    Object::merge_from(object, read_mask).map_err(|e| e.with_context("failed to merge object"))
}
