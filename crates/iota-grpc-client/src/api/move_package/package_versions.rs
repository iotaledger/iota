// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for listing package versions.

use iota_grpc_types::v0::move_package_service::{ListPackageVersionsRequest, PackageVersion};
use iota_sdk_types::ObjectId;

use crate::{
    Client,
    api::{MetadataEnvelope, Result, auto_paginate, proto_object_id},
};

impl Client {
    /// List all versions of a Move package.
    ///
    /// Returns proto `PackageVersion` types for each version of the package.
    /// Automatically paginates through all results.
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
        let base_request =
            ListPackageVersionsRequest::default().with_package_id(proto_object_id(package_id));

        let mut client = self.move_package_service_client();
        auto_paginate!(
            client,
            list_package_versions,
            base_request,
            limit,
            self.max_decoding_message_size(),
            versions
        )
    }
}
