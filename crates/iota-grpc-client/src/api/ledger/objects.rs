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
    api::{OBJECTS_READ_MASK, ProtoResult, Result, field_mask_with_default},
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
    /// # Field Mask
    ///
    /// The optional `read_mask` parameter controls which fields the server
    /// returns. If `None`, uses [`OBJECTS_READ_MASK`].
    ///
    /// **Optional fields:**
    /// - `bcs` - Object BCS data (for full deserialization)
    /// - `reference` - Object metadata (ID, version, digest)
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
    /// for obj in objs {
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
    ) -> Result<Vec<Object>> {
        if refs.is_empty() {
            return Ok(vec![]);
        }

        let requests = ObjectRequests {
            requests: refs
                .iter()
                .map(|(id, version)| ObjectRequest {
                    object_ref: Some(ObjectReference {
                        object_id: Some(id.to_string()),
                        version: *version,
                        digest: None,
                    }),
                })
                .collect(),
        };

        let request = GetObjectsRequest {
            requests: Some(requests),
            read_mask: Some(field_mask_with_default(read_mask, OBJECTS_READ_MASK)),
            max_message_size_bytes: self.max_decoding_message_size().map(|s| s as u32),
        };

        let mut client = self.ledger_service_client();

        let mut stream = client.get_objects(request).await?.into_inner();

        // Server guarantees results are returned in request order
        let mut results = Vec::with_capacity(refs.len());

        while let Some(response) = stream.message().await? {
            for result in response.objects {
                results.push(result.into_result()?);
            }
        }

        Ok(results)
    }
}
