// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use futures::Stream;
use iota_grpc_types::v0::move_package_service::{
    ListPackageVersionsRequest, ListPackageVersionsResponse, PackageVersion,
};
use prost::Message;

use crate::{
    constants::validate_max_message_size,
    error::RpcError,
    types::{GrpcReader, ListPackageVersionsStreamResult},
    validation::{collect_iter, object_id_proto, require_object_id, validate_limit},
};

/// Default limit for package version listing.
/// Higher than other list endpoints (50) because packages typically have far
/// fewer versions than an owner has objects or an object has dynamic fields.
const DEFAULT_LIMIT: usize = 1000;
/// Maximum limit for package version listing (same rationale as DEFAULT_LIMIT).
const MAX_LIMIT: usize = 10000;

#[tracing::instrument(skip(reader))]
pub(crate) fn list_package_versions(
    reader: Arc<GrpcReader>,
    ListPackageVersionsRequest {
        package_id,
        limit,
        max_message_size_bytes,
        ..
    }: ListPackageVersionsRequest,
) -> Result<impl Stream<Item = ListPackageVersionsStreamResult> + Send, RpcError> {
    let pkg_id = require_object_id(&package_id, "package_id")?;
    let limit = validate_limit(limit, DEFAULT_LIMIT, MAX_LIMIT);
    let max_message_size = validate_max_message_size(max_message_size_bytes)?;

    // Fetch the current package to validate it exists and is a package.
    // If the object has been pruned, fall back to using the requested ID as
    // the original package ID and check whether the version index has entries.
    let original_package_id = match reader.get_object(&pkg_id)? {
        Some(current_object) => {
            if !current_object.is_package() {
                return Err(RpcError::new(
                    tonic::Code::InvalidArgument,
                    format!("Object {pkg_id} is not a package"),
                ));
            }

            // Resolve the original package ID so we can list all versions across
            // different storage IDs (relevant for upgraded user packages).
            current_object
                .data
                .try_as_package()
                .ok_or_else(|| {
                    RpcError::new(
                        tonic::Code::Internal,
                        format!("Object {pkg_id} passed is_package() but try_as_package() failed"),
                    )
                })?
                .original_package_id()
        }
        None => {
            // The object may have been pruned from the object store. Fall back
            // to treating the requested ID as the original package ID and check
            // whether the version index has any entries for it.
            pkg_id
        }
    };

    // Streaming handles pagination; limit cap is applied for safety.
    let items = collect_iter(
        reader
            .package_versions_iter(original_package_id, None)?
            .take(limit),
    )?;

    if items.is_empty() {
        return Err(RpcError::from(crate::error::ObjectNotFoundError::new(
            pkg_id,
        )));
    }

    let versions: Vec<_> = items
        .into_iter()
        .map(|(key, info)| {
            let version = PackageVersion::default()
                .with_package_id(object_id_proto(&info.storage_id))
                .with_version(key.version);
            let size = version.encoded_len();
            (version, size)
        })
        .collect();

    Ok(crate::create_batching_stream!(
        versions.into_iter(),
        (version, size),
        { (version, size) },
        max_message_size,
        ListPackageVersionsResponse,
        versions,
        has_next
    ))
}
