// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for listing package versions.

use iota_grpc_types::v0::move_package_service::{ListPackageVersionsRequest, PackageVersion};
use iota_sdk_types::ObjectId;

use crate::{
    Client,
    api::{MetadataEnvelope, Result, collect_stream, proto_object_id, saturating_usize_to_u32},
};

impl Client {
    /// List all versions of a Move package.
    ///
    /// Returns proto `PackageVersion` types for each version of the package.
    /// Results are streamed and collected into a `Vec`.
    ///
    /// # Parameters
    ///
    /// - `package_id` - The object ID of any version of the package.
    /// - `limit` - Optional maximum number of versions to return.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::ObjectId;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let package_id: ObjectId = "0x2".parse()?;
    ///
    /// let response = client.list_package_versions(package_id, None).await?;
    /// for version in response.body() {
    ///     println!("Package version: {:?}", version);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_package_versions(
        &self,
        package_id: ObjectId,
        limit: Option<u32>,
    ) -> Result<MetadataEnvelope<Vec<PackageVersion>>> {
        let mut request =
            ListPackageVersionsRequest::default().with_package_id(proto_object_id(package_id));

        if let Some(l) = limit {
            request = request.with_limit(l);
        }

        if let Some(max_size) = self.max_decoding_message_size() {
            request = request.with_max_message_size_bytes(saturating_usize_to_u32(max_size));
        }

        let mut client = self.move_package_service_client();

        let response = client.list_package_versions(request).await?;
        let (stream, metadata) = MetadataEnvelope::from(response).into_parts();

        collect_stream(stream, metadata, |msg| Ok((msg.has_next, msg.versions))).await
    }
}
