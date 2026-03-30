// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_grpc_types::v1::move_package_service::{
    ListPackageVersionsRequest, ListPackageVersionsResponse, PackageVersion,
};
use iota_types::base_types::ObjectID;
use prost::Message;
use serde::{Deserialize, Serialize};

use crate::{
    constants::validate_max_message_size,
    error::RpcError,
    types::GrpcReader,
    validation::{
        decode_page_token, encode_page_token, object_id_proto, page_token_mismatch,
        require_object_id, validate_page_size,
    },
};

const DEFAULT_PAGE_SIZE: u32 = 1000;
const MAX_PAGE_SIZE: u32 = 10000;

#[derive(Serialize, Deserialize)]
struct PageToken {
    original_package_id: ObjectID,
    last_version: u64,
}

#[tracing::instrument(skip(reader))]
pub(crate) fn list_package_versions(
    reader: Arc<GrpcReader>,
    ListPackageVersionsRequest {
        package_id,
        page_size,
        page_token,
        max_message_size_bytes,
        ..
    }: ListPackageVersionsRequest,
) -> Result<ListPackageVersionsResponse, RpcError> {
    let pkg_id = require_object_id(&package_id, "package_id")?;
    let page_size = validate_page_size(page_size, DEFAULT_PAGE_SIZE, MAX_PAGE_SIZE);
    let max_message_size = validate_max_message_size(max_message_size_bytes)?;

    // Resolve the original package ID so we can list all versions across
    // different storage IDs (relevant for upgraded user packages).
    let original_package_id = match reader.get_object(&pkg_id)? {
        Some(current_object) => {
            if !current_object.is_package() {
                return Err(RpcError::new(
                    tonic::Code::InvalidArgument,
                    format!("Object {pkg_id} is not a package"),
                ));
            }

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

    let page_token: Option<PageToken> = decode_page_token(&page_token)?;
    if let Some(ref t) = page_token {
        if t.original_package_id != original_package_id {
            return Err(page_token_mismatch());
        }
    }

    let cursor_version = page_token.map(|t| t.last_version);

    let mut iter = reader.package_versions_iter(original_package_id, cursor_version)?;

    let mut versions = Vec::with_capacity(page_size);
    let mut size_bytes = 0usize;
    let mut last_version: Option<u64> = None;

    for result in iter.by_ref() {
        let (key, info) = result.map_err(RpcError::from)?;

        let version = PackageVersion::default()
            .with_original_id(object_id_proto(&key.original_package_id))
            .with_storage_id(object_id_proto(&info.storage_id))
            .with_version(key.version);

        let item_size = version.encoded_len();

        if !versions.is_empty() && size_bytes + item_size > max_message_size {
            let response = ListPackageVersionsResponse::default()
                .with_versions(versions)
                .with_next_page_token(encode_page_token(&PageToken {
                    original_package_id,
                    last_version: last_version.expect("versions is non-empty"),
                }));
            return Ok(response);
        }

        last_version = Some(key.version);
        versions.push(version);
        size_bytes += item_size;

        if versions.len() >= page_size {
            break;
        }
    }

    if versions.is_empty() && cursor_version.is_none() {
        return Err(RpcError::from(crate::error::ObjectNotFoundError::new(
            pkg_id,
        )));
    }

    // Check if there are more items.
    let has_more = iter.next().transpose().map_err(RpcError::from)?.is_some();

    let mut response = ListPackageVersionsResponse::default().with_versions(versions);
    if has_more {
        if let Some(ver) = last_version {
            response = response.with_next_page_token(encode_page_token(&PageToken {
                original_package_id,
                last_version: ver,
            }));
        }
    }

    Ok(response)
}
