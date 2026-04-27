// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::ObjectDigest,
    error::IotaObjectResponseError as NativeObjectResponseError,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeAs, SerializeAs, serde_as};

use crate::iota_primitives::{
    Base58 as Base58Schema, ObjectID as ObjectIDSchema,
    SequenceNumberU64 as SequenceNumberU64Schema,
};

#[serde_as]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(tag = "code", rename = "ObjectResponseError", rename_all = "camelCase")]
pub enum IotaObjectResponseError {
    NotExists {
        #[schemars(with = "ObjectIDSchema")]
        object_id: ObjectID,
    },
    DynamicFieldNotFound {
        #[schemars(with = "ObjectIDSchema")]
        parent_object_id: ObjectID,
    },
    Deleted {
        #[schemars(with = "ObjectIDSchema")]
        object_id: ObjectID,
        /// Object version.
        #[schemars(with = "SequenceNumberU64Schema")]
        version: SequenceNumber,
        /// Base64 string representing the object digest
        #[schemars(with = "Base58Schema")]
        digest: ObjectDigest,
    },
    Unknown,
    Display {
        error: String,
    },
}

impl SerializeAs<NativeObjectResponseError> for IotaObjectResponseError {
    fn serialize_as<S>(source: &NativeObjectResponseError, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        IotaObjectResponseError::from(source.clone()).serialize(serializer)
    }
}

impl<'de> DeserializeAs<'de, NativeObjectResponseError> for IotaObjectResponseError {
    fn deserialize_as<D>(deserializer: D) -> Result<NativeObjectResponseError, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let schema = IotaObjectResponseError::deserialize(deserializer)?;
        Ok(NativeObjectResponseError::from(schema))
    }
}

impl From<NativeObjectResponseError> for IotaObjectResponseError {
    fn from(error: NativeObjectResponseError) -> Self {
        match error {
            NativeObjectResponseError::NotExists { object_id } => Self::NotExists { object_id },
            NativeObjectResponseError::DynamicFieldNotFound { parent_object_id } => {
                Self::DynamicFieldNotFound { parent_object_id }
            }
            NativeObjectResponseError::Deleted {
                object_id,
                version,
                digest,
            } => Self::Deleted {
                object_id,
                version,
                digest,
            },
            NativeObjectResponseError::Unknown => Self::Unknown,
            NativeObjectResponseError::Display { error } => Self::Display { error },
        }
    }
}

impl From<IotaObjectResponseError> for NativeObjectResponseError {
    fn from(error: IotaObjectResponseError) -> Self {
        match error {
            IotaObjectResponseError::NotExists { object_id } => Self::NotExists { object_id },
            IotaObjectResponseError::DynamicFieldNotFound { parent_object_id } => {
                Self::DynamicFieldNotFound { parent_object_id }
            }
            IotaObjectResponseError::Deleted {
                object_id,
                version,
                digest,
            } => Self::Deleted {
                object_id,
                version,
                digest,
            },
            IotaObjectResponseError::Unknown => Self::Unknown,
            IotaObjectResponseError::Display { error } => Self::Display { error },
        }
    }
}
