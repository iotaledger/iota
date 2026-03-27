// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for listing package versions.

use iota_grpc_types::v1::move_package_service::{
    ListPackageVersionsRequest, PackageVersion,
    move_package_service_client::MovePackageServiceClient,
};
use iota_sdk_types::ObjectId;

use crate::{
    Client, InterceptedChannel,
    api::{define_list_query, proto_object_id},
};

define_list_query! {
    /// Builder for listing versions of a Move package.
    ///
    /// Created by [`Client::list_package_versions`]. Await directly for a
    /// single page, or call [`.collect(limit)`](Self::collect) to
    /// auto-paginate.
    pub struct ListPackageVersionsQuery {
        service_client: MovePackageServiceClient<InterceptedChannel>,
        request: ListPackageVersionsRequest,
        item: PackageVersion,
        rpc_method: list_package_versions,
        items_field: versions,
    }
}

impl Client {
    /// List all versions of a Move package.
    ///
    /// Returns a query builder. Await it directly for a single page
    /// (with access to `next_page_token`), or call `.collect(limit)` to
    /// auto-paginate through all results.
    ///
    /// # Parameters
    ///
    /// - `package_id` - The object ID of any version of the package.
    /// - `page_size` - Optional maximum number of versions per page.
    /// - `page_token` - Optional continuation token from a previous page.
    ///
    /// # Examples
    ///
    /// Single page:
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::ObjectId;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let package_id: ObjectId = "0x2".parse()?;
    ///
    /// let page = client.list_package_versions(package_id, None, None).await?;
    /// for version in &page.body().items {
    ///     println!("Package version: {:?}", version);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Auto-paginate:
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::ObjectId;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let package_id: ObjectId = "0x2".parse()?;
    ///
    /// let all = client
    ///     .list_package_versions(package_id, Some(50), None)
    ///     .collect(None)
    ///     .await?;
    /// for version in all.body() {
    ///     println!("Package version: {:?}", version);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_package_versions(
        &self,
        package_id: ObjectId,
        page_size: Option<u32>,
        page_token: Option<prost::bytes::Bytes>,
    ) -> ListPackageVersionsQuery {
        let base_request =
            ListPackageVersionsRequest::default().with_package_id(proto_object_id(package_id));

        ListPackageVersionsQuery::new(
            self.move_package_service_client(),
            base_request,
            self.max_decoding_message_size(),
            page_size,
            page_token,
        )
    }
}
