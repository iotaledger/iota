// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for listing dynamic fields.
//!
//! # Available Read Mask Fields
//!
//! - `kind` - the kind of dynamic field (field or object)
//! - `parent` - the parent object ID
//! - `field_id` - the field object ID
//! - `child_id` - the child object ID (for dynamic object fields)
//! - `name` - BCS-encoded field name
//! - `value` - BCS-encoded field value
//! - `value_type` - the Move type of the value
//! - `field_object` - the full field object (sub-fields match `GetObjects`)
//! - `child_object` - the full child object (sub-fields match `GetObjects`)

use iota_grpc_types::v1::{
    dynamic_field::DynamicField,
    state_service::{ListDynamicFieldsRequest, state_service_client::StateServiceClient},
};
use iota_sdk_types::ObjectId;

use crate::{
    Client, InterceptedChannel,
    api::{
        LIST_DYNAMIC_FIELDS_READ_MASK, define_list_query, field_mask_with_default, proto_object_id,
    },
};

define_list_query! {
    /// Builder for listing dynamic fields of a parent object.
    ///
    /// Created by [`Client::list_dynamic_fields`]. Await directly for a
    /// single page, or call [`.collect(limit)`](Self::collect) to
    /// auto-paginate.
    pub struct ListDynamicFieldsQuery {
        service_client: StateServiceClient<InterceptedChannel>,
        request: ListDynamicFieldsRequest,
        item: DynamicField,
        rpc_method: list_dynamic_fields,
        items_field: dynamic_fields,
    }
}

impl Client {
    /// List dynamic fields owned by a parent object.
    ///
    /// Returns a query builder. Await it directly for a single page
    /// (with access to `next_page_token`), or call `.collect(limit)` to
    /// auto-paginate through all results.
    ///
    /// # Parameters
    ///
    /// - `parent` - The object ID of the parent object.
    /// - `page_size` - Optional maximum number of fields per page.
    /// - `page_token` - Optional continuation token from a previous page.
    /// - `read_mask` - Optional field mask. If `None`, uses
    ///   [`LIST_DYNAMIC_FIELDS_READ_MASK`].
    ///
    /// # Examples
    ///
    /// Single page:
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::ObjectId;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let parent: ObjectId = "0x2".parse()?;
    ///
    /// let page = client.list_dynamic_fields(parent, None, None, None).await?;
    /// for field in &page.body().items {
    ///     println!("Dynamic field: {:?}", field);
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
    /// let parent: ObjectId = "0x2".parse()?;
    ///
    /// let all = client
    ///     .list_dynamic_fields(parent, Some(50), None, None)
    ///     .collect(None)
    ///     .await?;
    /// for field in all.body() {
    ///     println!("Dynamic field: {:?}", field);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_dynamic_fields(
        &self,
        parent: ObjectId,
        page_size: Option<u32>,
        page_token: Option<prost::bytes::Bytes>,
        read_mask: Option<&str>,
    ) -> ListDynamicFieldsQuery {
        let base_request = ListDynamicFieldsRequest::default()
            .with_parent(proto_object_id(parent))
            .with_read_mask(field_mask_with_default(
                read_mask,
                LIST_DYNAMIC_FIELDS_READ_MASK,
            ));

        ListDynamicFieldsQuery::new(
            self.state_service_client(),
            base_request,
            self.max_decoding_message_size(),
            page_size,
            page_token,
        )
    }
}
