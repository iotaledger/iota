// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for listing owned objects.
//!
//! # Available Read Mask Fields
//!
//! Object fields mirror those of `GetObjects`:
//! - `reference` - the object reference (includes sub-fields below)
//!   - `reference.object_id` - the object ID
//!   - `reference.version` - the object version
//!   - `reference.digest` - the object digest
//! - `object_type` - the Move type of the object
//! - `owner` - the object owner
//! - `bcs` - the full BCS-encoded object

use iota_grpc_types::v1::{
    object::Object,
    state_service::{ListOwnedObjectsRequest, state_service_client::StateServiceClient},
    types::Address as ProtoAddress,
};
use iota_sdk_types::{Address, StructTag};

use crate::{
    Client, InterceptedChannel,
    api::{LIST_OWNED_OBJECTS_READ_MASK, define_list_query, field_mask_with_default},
};

define_list_query! {
    /// Builder for listing objects owned by an address.
    ///
    /// Created by [`Client::list_owned_objects`]. Await directly for a
    /// single page, or call [`.collect(limit)`](Self::collect) to
    /// auto-paginate.
    pub struct ListOwnedObjectsQuery {
        service_client: StateServiceClient<InterceptedChannel>,
        request: ListOwnedObjectsRequest,
        item: Object,
        rpc_method: list_owned_objects,
        items_field: objects,
    }
}

impl Client {
    /// List objects owned by an address.
    ///
    /// Returns a query builder. Await it directly for a single page
    /// (with access to `next_page_token`), or call `.collect(limit)` to
    /// auto-paginate through all results.
    ///
    /// # Parameters
    ///
    /// - `owner` - The address that owns the objects.
    /// - `object_type` - Optional type filter as a [`StructTag`].
    /// - `page_size` - Optional maximum number of objects per page.
    /// - `page_token` - Optional continuation token from a previous page.
    /// - `read_mask` - Optional field mask. If `None`, uses
    ///   [`LIST_OWNED_OBJECTS_READ_MASK`].
    ///
    /// # Examples
    ///
    /// Single page:
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::Address;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let owner: Address = "0x1".parse()?;
    ///
    /// let page = client
    ///     .list_owned_objects(owner, None, None, None, None)
    ///     .await?;
    /// for obj in &page.body().items {
    ///     println!("Owned object: {:?}", obj);
    /// }
    /// if let Some(token) = &page.body().next_page_token {
    ///     // Fetch the next page using the token
    ///     let next = client
    ///         .list_owned_objects(owner, None, None, Some(token.clone()), None)
    ///         .await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Auto-paginate:
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::Address;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let owner: Address = "0x1".parse()?;
    ///
    /// let all = client
    ///     .list_owned_objects(owner, None, Some(50), None, None)
    ///     .collect(Some(500))
    ///     .await?;
    /// for obj in all.body() {
    ///     println!("Owned object: {:?}", obj);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_owned_objects(
        &self,
        owner: Address,
        object_type: Option<StructTag>,
        page_size: Option<u32>,
        page_token: Option<prost::bytes::Bytes>,
        read_mask: Option<&str>,
    ) -> ListOwnedObjectsQuery {
        let mut base_request = ListOwnedObjectsRequest::default()
            .with_owner(ProtoAddress::default().with_address(Vec::from(owner)))
            .with_read_mask(field_mask_with_default(
                read_mask,
                LIST_OWNED_OBJECTS_READ_MASK,
            ));

        if let Some(t) = object_type {
            base_request = base_request.with_object_type(t.to_string());
        }

        ListOwnedObjectsQuery::new(
            self.state_service_client(),
            base_request,
            self.max_decoding_message_size(),
            page_size,
            page_token,
        )
    }
}
