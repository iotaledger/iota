// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! High-level API for object queries.

use iota_grpc_types::v0::{
    ledger_service::{GetObjectsRequest, ObjectRequest, ObjectRequests},
    object::Object,
    types::ObjectReference,
};
use iota_sdk_types::{ObjectId, Version};

use crate::{
    Client,
    api::{
        Error, GET_OBJECTS_READ_MASK, MetadataEnvelope, ProtoResult, Result,
        field_mask_with_default,
    },
};

impl Client {
    /// Get objects by their IDs and optional versions.
    ///
    /// Returns proto `Object` types. Use `obj.object()` to convert to SDK
    /// type, or use `obj.object_reference()` to get the object reference.
    ///
    /// Results are returned in the same order as the input refs.
    /// If an object is not found, an error is returned.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyRequest`] if `refs` is empty.
    ///
    /// # Available Read Mask Fields
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`GET_OBJECTS_READ_MASK`].
    ///
    /// ## Reference Fields
    /// - `reference` - includes all reference fields
    ///   - `reference.object_id` - the ID of the object to fetch
    ///   - `reference.version` - the version of the object, which can be used
    ///     to fetch a specific historical version or the latest version if not
    ///     provided
    ///   - `reference.digest` - the digest of the object contents, which can be
    ///     used for integrity verification
    ///
    /// ## Data Fields
    /// - `bcs` - the full BCS-encoded object
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use iota_grpc_client::Client;
    /// # use iota_sdk_types::ObjectId;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect("http://localhost:9000").await?;
    /// let object_id: ObjectId = "0x2".parse()?;
    ///
    /// // Get proto objects
    /// let objs = client.get_objects(&[(object_id, None)], None).await?;
    ///
    /// for obj in objs.body() {
    ///     // Convert proto object to SDK type
    ///     let sdk_obj = obj.object()?;
    ///     println!("Got object ID: {:?}", sdk_obj.object_id());
    ///     let obj_ref = obj.object_reference()?;
    ///     println!("Object version: {:?}", obj_ref.version());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_objects(
        &self,
        refs: &[(ObjectId, Option<Version>)],
        read_mask: Option<&str>,
    ) -> Result<MetadataEnvelope<Vec<Object>>> {
        if refs.is_empty() {
            return Err(Error::EmptyRequest);
        }

        let requests = ObjectRequests::default().with_requests(
            refs.iter()
                .map(|(id, version)| {
                    let mut object_ref = ObjectReference::default().with_object_id(id.to_string());

                    if let Some(v) = version {
                        object_ref = object_ref.with_version(*v);
                    }

                    ObjectRequest::default().with_object_ref(object_ref)
                })
                .collect(),
        );

        let mut request = GetObjectsRequest::default()
            .with_requests(requests)
            .with_read_mask(field_mask_with_default(read_mask, GET_OBJECTS_READ_MASK));

        if let Some(max_size) = self.max_decoding_message_size() {
            request = request.with_max_message_size_bytes(max_size as u32);
        }

        let mut client = self.ledger_service_client();

        let response = client.get_objects(request).await?;
        let (mut stream, metadata) = MetadataEnvelope::from(response).into_parts();

        // Server guarantees results are returned in request order
        let mut results = Vec::with_capacity(refs.len());
        let mut has_next = false;

        while let Some(response) = stream.message().await? {
            has_next = response.has_next;
            for result in response.objects {
                results.push(result.into_result()?);
            }
        }

        if has_next {
            return Err(Error::UnexpectedEndOfStream);
        }

        Ok(MetadataEnvelope::new(results, metadata))
    }
}
