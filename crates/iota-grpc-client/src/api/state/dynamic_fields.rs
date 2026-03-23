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
        LIST_DYNAMIC_FIELDS_READ_MASK, MetadataEnvelope, Result, collect_stream,
        field_mask_with_default, proto_object_id, saturating_usize_to_u32,
    },
};

impl Client {
    /// List dynamic fields owned by a parent object.
    ///
    /// Returns proto `DynamicField` types for the given parent.
    /// Results are streamed and collected into a `Vec`.
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
        let mut request = ListDynamicFieldsRequest::default()
            .with_parent(proto_object_id(parent))
            .with_read_mask(field_mask_with_default(
                read_mask,
                LIST_DYNAMIC_FIELDS_READ_MASK,
            ));

        if let Some(l) = limit {
            request = request.with_limit(l);
        }

        if let Some(max_size) = self.max_decoding_message_size() {
            request = request.with_max_message_size_bytes(saturating_usize_to_u32(max_size));
        }

        let mut client = self.state_service_client();

        let response = client.list_dynamic_fields(request).await?;
        let (stream, metadata) = MetadataEnvelope::from(response).into_parts();

        collect_stream(stream, metadata, |msg| {
            Ok((msg.has_next, msg.dynamic_fields))
        })
        .await
    }
}
