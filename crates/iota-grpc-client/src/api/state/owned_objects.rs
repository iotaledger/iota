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

use iota_grpc_types::v0::{
    object::Object, state_service::ListOwnedObjectsRequest, types::Address as ProtoAddress,
};
use iota_sdk_types::{Address, StructTag};

use crate::{
    Client,
    api::{
        LIST_OWNED_OBJECTS_READ_MASK, MetadataEnvelope, Result, collect_stream,
        field_mask_with_default, saturating_usize_to_u32,
    },
};

impl Client {
    /// List objects owned by an address.
    ///
    /// Returns proto `Object` types owned by the given address.
    /// Results are streamed and collected into a `Vec`.
    ///
    /// # Parameters
    ///
    /// - `owner` - The address that owns the objects.
    /// - `object_type` - Optional type filter as a [`StructTag`].
    /// - `limit` - Optional maximum number of objects to return.
    /// - `read_mask` - Optional field mask. If `None`, uses
    ///   [`LIST_OWNED_OBJECTS_READ_MASK`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::Address;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let owner: Address = "0x1".parse()?;
    ///
    /// let response = client.list_owned_objects(owner, None, None, None).await?;
    /// for obj in response.body() {
    ///     println!("Owned object: {:?}", obj);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn list_owned_objects(
        &self,
        owner: Address,
        object_type: Option<StructTag>,
        limit: Option<u32>,
        read_mask: Option<&str>,
    ) -> Result<MetadataEnvelope<Vec<Object>>> {
        let mut request = ListOwnedObjectsRequest::default()
            .with_owner(ProtoAddress::default().with_address(Vec::from(owner)))
            .with_read_mask(field_mask_with_default(
                read_mask,
                LIST_OWNED_OBJECTS_READ_MASK,
            ));

        if let Some(t) = object_type {
            request = request.with_object_type(t.to_string());
        }

        if let Some(l) = limit {
            request = request.with_limit(l);
        }

        if let Some(max_size) = self.max_decoding_message_size() {
            request = request.with_max_message_size_bytes(saturating_usize_to_u32(max_size));
        }

        let mut client = self.state_service_client();

        let response = client.list_owned_objects(request).await?;
        let (stream, metadata) = MetadataEnvelope::from(response).into_parts();

        collect_stream(stream, metadata, |msg| Ok((msg.has_next, msg.objects))).await
    }
}
