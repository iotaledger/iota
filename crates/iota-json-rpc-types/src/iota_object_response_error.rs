// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::ObjectId;
use iota_types::{base_types::SequenceNumber, digests::ObjectDigest};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{AsRefStr, IntoStaticStr};
use thiserror::Error;

use crate::iota_primitives::{
    Base58 as Base58Schema, ObjectID as ObjectIDSchema,
    SequenceNumberU64 as SequenceNumberU64Schema,
};

#[derive(
    Eq,
    PartialEq,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    JsonSchema,
    Hash,
    AsRefStr,
    IntoStaticStr,
    Error,
)]
#[serde(tag = "code", rename = "ObjectResponseError", rename_all = "camelCase")]
pub enum IotaObjectResponseError {
    #[error("Object {object_id} does not exist")]
    NotExists {
        #[schemars(with = "ObjectIDSchema")]
        object_id: ObjectId,
    },
    #[error("Cannot find dynamic field for parent object {parent_object_id}")]
    DynamicFieldNotFound {
        #[schemars(with = "ObjectIDSchema")]
        parent_object_id: ObjectId,
    },
    #[error(
        "Object has been deleted object_id: {object_id} at version: {version} in digest {digest}"
    )]
    Deleted {
        #[schemars(with = "ObjectIDSchema")]
        object_id: ObjectId,
        /// Object version.
        #[schemars(with = "SequenceNumberU64Schema")]
        version: SequenceNumber,
        /// Base64 string representing the object digest
        #[schemars(with = "Base58Schema")]
        digest: ObjectDigest,
    },
    #[error("Unknown Error")]
    Unknown,
    #[error("Display Error: {error}")]
    Display { error: String },
}
