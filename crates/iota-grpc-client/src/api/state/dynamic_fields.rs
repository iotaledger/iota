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

use iota_grpc_types::v0::{dynamic_field::DynamicField, state_service::ListDynamicFieldsRequest};
use iota_sdk_types::ObjectId;

use crate::{
    Client,
    api::{
        LIST_DYNAMIC_FIELDS_READ_MASK, MetadataEnvelope, Result, auto_paginate,
        field_mask_with_default, proto_object_id,
    },
};

impl Client {
    /// List dynamic fields owned by a parent object.
    ///
    /// Returns proto `DynamicField` types for the given parent.
    /// Automatically paginates through all results.
    ///
    /// # Parameters
    ///
    /// - `parent` - The object ID of the parent object.
    /// - `limit` - Optional maximum number of fields to return.
    /// - `read_mask` - Optional field mask. If `None`, uses
    ///   [`LIST_DYNAMIC_FIELDS_READ_MASK`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::ObjectId;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let parent: ObjectId = "0x2".parse()?;
    ///
    /// let response = client.list_dynamic_fields(parent, None, None).await?;
    /// for field in response.body() {
    ///     println!("Dynamic field: {:?}", field);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_dynamic_fields(
        &self,
        parent: ObjectId,
        limit: Option<u32>,
        read_mask: Option<&str>,
    ) -> Result<MetadataEnvelope<Vec<DynamicField>>> {
        let base_request = ListDynamicFieldsRequest::default()
            .with_parent(proto_object_id(parent))
            .with_read_mask(field_mask_with_default(
                read_mask,
                LIST_DYNAMIC_FIELDS_READ_MASK,
            ));

        let mut client = self.state_service_client();
        auto_paginate!(
            client,
            list_dynamic_fields,
            base_request,
            limit,
            self.max_decoding_message_size(),
            dynamic_fields
        )
    }
}
