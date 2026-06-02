// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter, Result};

use iota_sdk_types::{ObjectId, StructTag};
use iota_types::{
    base_types::{IotaAddress, ObjectDigest, ObjectRef, SequenceNumber},
    object::Owner,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    iota_owner::OwnerSchema,
    iota_primitives::{
        Base58 as Base58Schema, IotaAddress as IotaAddressSchema, ObjectId as ObjectIdSchema,
        SequenceNumberString as SequenceNumberStringSchema, StructTag as StructTagSchema,
    },
};

/// ObjectChange are derived from the object mutations in the TransactionEffect
/// to provide richer object information.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ObjectChange {
    /// Module published
    #[serde(rename_all = "camelCase")]
    Published {
        #[serde_as(as = "ObjectIdSchema")]
        #[schemars(with = "ObjectIdSchema")]
        package_id: ObjectId,
        #[schemars(with = "SequenceNumberStringSchema")]
        #[serde_as(as = "SequenceNumberStringSchema")]
        version: SequenceNumber,
        #[serde_as(as = "Base58Schema")]
        #[schemars(with = "Base58Schema")]
        digest: ObjectDigest,
        modules: Vec<String>,
    },
    /// Transfer objects to new address / wrap in another object
    #[serde(rename_all = "camelCase")]
    Transferred {
        #[serde_as(as = "IotaAddressSchema")]
        #[schemars(with = "IotaAddressSchema")]
        sender: IotaAddress,
        #[schemars(with = "OwnerSchema")]
        #[serde_as(as = "OwnerSchema")]
        recipient: Owner,
        #[schemars(with = "StructTagSchema")]
        #[serde_as(as = "StructTagSchema")]
        object_type: StructTag,
        #[serde_as(as = "ObjectIdSchema")]
        #[schemars(with = "ObjectIdSchema")]
        object_id: ObjectId,
        #[schemars(with = "SequenceNumberStringSchema")]
        #[serde_as(as = "SequenceNumberStringSchema")]
        version: SequenceNumber,
        #[serde_as(as = "Base58Schema")]
        #[schemars(with = "Base58Schema")]
        digest: ObjectDigest,
    },
    /// Object mutated.
    #[serde(rename_all = "camelCase")]
    Mutated {
        #[serde_as(as = "IotaAddressSchema")]
        #[schemars(with = "IotaAddressSchema")]
        sender: IotaAddress,
        #[schemars(with = "OwnerSchema")]
        #[serde_as(as = "OwnerSchema")]
        owner: Owner,
        #[schemars(with = "StructTagSchema")]
        #[serde_as(as = "StructTagSchema")]
        object_type: StructTag,
        #[serde_as(as = "ObjectIdSchema")]
        #[schemars(with = "ObjectIdSchema")]
        object_id: ObjectId,
        #[schemars(with = "SequenceNumberStringSchema")]
        #[serde_as(as = "SequenceNumberStringSchema")]
        version: SequenceNumber,
        #[schemars(with = "SequenceNumberStringSchema")]
        #[serde_as(as = "SequenceNumberStringSchema")]
        previous_version: SequenceNumber,
        #[serde_as(as = "Base58Schema")]
        #[schemars(with = "Base58Schema")]
        digest: ObjectDigest,
    },
    /// Delete object
    #[serde(rename_all = "camelCase")]
    Deleted {
        #[serde_as(as = "IotaAddressSchema")]
        #[schemars(with = "IotaAddressSchema")]
        sender: IotaAddress,
        #[schemars(with = "StructTagSchema")]
        #[serde_as(as = "StructTagSchema")]
        object_type: StructTag,
        #[serde_as(as = "ObjectIdSchema")]
        #[schemars(with = "ObjectIdSchema")]
        object_id: ObjectId,
        #[schemars(with = "SequenceNumberStringSchema")]
        #[serde_as(as = "SequenceNumberStringSchema")]
        version: SequenceNumber,
    },
    /// Wrapped object
    #[serde(rename_all = "camelCase")]
    Wrapped {
        #[serde_as(as = "IotaAddressSchema")]
        #[schemars(with = "IotaAddressSchema")]
        sender: IotaAddress,
        #[schemars(with = "StructTagSchema")]
        #[serde_as(as = "StructTagSchema")]
        object_type: StructTag,
        #[serde_as(as = "ObjectIdSchema")]
        #[schemars(with = "ObjectIdSchema")]
        object_id: ObjectId,
        #[schemars(with = "SequenceNumberStringSchema")]
        #[serde_as(as = "SequenceNumberStringSchema")]
        version: SequenceNumber,
    },
    /// Unwrapped object
    #[serde(rename_all = "camelCase")]
    Unwrapped {
        #[serde_as(as = "IotaAddressSchema")]
        #[schemars(with = "IotaAddressSchema")]
        sender: IotaAddress,
        #[schemars(with = "OwnerSchema")]
        #[serde_as(as = "OwnerSchema")]
        owner: Owner,
        #[schemars(with = "StructTagSchema")]
        #[serde_as(as = "StructTagSchema")]
        object_type: StructTag,
        #[serde_as(as = "ObjectIdSchema")]
        #[schemars(with = "ObjectIdSchema")]
        object_id: ObjectId,
        #[schemars(with = "SequenceNumberStringSchema")]
        #[serde_as(as = "SequenceNumberStringSchema")]
        version: SequenceNumber,
        #[serde_as(as = "Base58Schema")]
        #[schemars(with = "Base58Schema")]
        digest: ObjectDigest,
    },
    /// New object creation
    #[serde(rename_all = "camelCase")]
    Created {
        #[serde_as(as = "IotaAddressSchema")]
        #[schemars(with = "IotaAddressSchema")]
        sender: IotaAddress,
        #[schemars(with = "OwnerSchema")]
        #[serde_as(as = "OwnerSchema")]
        owner: Owner,
        #[schemars(with = "StructTagSchema")]
        #[serde_as(as = "StructTagSchema")]
        object_type: StructTag,
        #[serde_as(as = "ObjectIdSchema")]
        #[schemars(with = "ObjectIdSchema")]
        object_id: ObjectId,
        #[schemars(with = "SequenceNumberStringSchema")]
        #[serde_as(as = "SequenceNumberStringSchema")]
        version: SequenceNumber,
        #[serde_as(as = "Base58Schema")]
        #[schemars(with = "Base58Schema")]
        digest: ObjectDigest,
    },
}

impl ObjectChange {
    pub fn object_id(&self) -> ObjectId {
        match self {
            ObjectChange::Published { package_id, .. } => *package_id,
            ObjectChange::Transferred { object_id, .. }
            | ObjectChange::Mutated { object_id, .. }
            | ObjectChange::Deleted { object_id, .. }
            | ObjectChange::Wrapped { object_id, .. }
            | ObjectChange::Unwrapped { object_id, .. }
            | ObjectChange::Created { object_id, .. } => *object_id,
        }
    }

    pub fn object_ref(&self) -> ObjectRef {
        match self {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                ..
            } => ObjectRef::new(*package_id, *version, *digest),
            ObjectChange::Transferred {
                object_id,
                version,
                digest,
                ..
            }
            | ObjectChange::Mutated {
                object_id,
                version,
                digest,
                ..
            }
            | ObjectChange::Unwrapped {
                object_id,
                version,
                digest,
                ..
            }
            | ObjectChange::Created {
                object_id,
                version,
                digest,
                ..
            } => ObjectRef::new(*object_id, *version, *digest),
            ObjectChange::Deleted {
                object_id, version, ..
            } => ObjectRef::new(*object_id, *version, ObjectDigest::OBJECT_DELETED),
            ObjectChange::Wrapped {
                object_id, version, ..
            } => ObjectRef::new(*object_id, *version, ObjectDigest::OBJECT_WRAPPED),
        }
    }

    pub fn mask_for_test(&mut self, new_version: SequenceNumber, new_digest: ObjectDigest) {
        match self {
            ObjectChange::Published {
                version, digest, ..
            }
            | ObjectChange::Transferred {
                version, digest, ..
            }
            | ObjectChange::Mutated {
                version, digest, ..
            }
            | ObjectChange::Unwrapped {
                version, digest, ..
            }
            | ObjectChange::Created {
                version, digest, ..
            } => {
                *version = new_version;
                *digest = new_digest
            }
            ObjectChange::Deleted { version, .. } | ObjectChange::Wrapped { version, .. } => {
                *version = new_version
            }
        }
    }
}

impl Display for ObjectChange {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => {
                write!(
                    f,
                    " ┌──\n │ PackageID: {} \n │ Version: {} \n │ Digest: {}\n │ Modules: {}\n └──",
                    package_id,
                    version,
                    digest,
                    modules.join(", ")
                )
            }
            ObjectChange::Transferred {
                sender,
                recipient,
                object_type,
                object_id,
                version,
                digest,
            } => {
                write!(
                    f,
                    " ┌──\n │ ObjectId: {object_id}\n │ Sender: {sender} \n │ Recipient: {recipient}\n │ ObjectType: {object_type} \n │ Version: {version}\n │ Digest: {digest}\n └──"
                )
            }
            ObjectChange::Mutated {
                sender,
                owner,
                object_type,
                object_id,
                version,
                previous_version: _,
                digest,
            } => {
                write!(
                    f,
                    " ┌──\n │ ObjectId: {object_id}\n │ Sender: {sender} \n │ Owner: {owner}\n │ ObjectType: {object_type} \n │ Version: {version}\n │ Digest: {digest}\n └──"
                )
            }
            ObjectChange::Deleted {
                sender,
                object_type,
                object_id,
                version,
            } => {
                write!(
                    f,
                    " ┌──\n │ ObjectId: {object_id}\n │ Sender: {sender} \n │ ObjectType: {object_type} \n │ Version: {version}\n └──"
                )
            }
            ObjectChange::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            } => {
                write!(
                    f,
                    " ┌──\n │ ObjectId: {object_id}\n │ Sender: {sender} \n │ ObjectType: {object_type} \n │ Version: {version}\n └──"
                )
            }
            ObjectChange::Unwrapped {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => {
                write!(
                    f,
                    " ┌──\n │ ObjectId: {}\n │ Sender: {} \n │ Owner: {}\n │ ObjectType: {} \n │ Version: {}\n │ Digest: {}\n └──",
                    object_id,
                    sender,
                    owner,
                    object_type,
                    version.as_u64(),
                    digest
                )
            }
            ObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => {
                write!(
                    f,
                    " ┌──\n │ ObjectId: {object_id}\n │ Sender: {sender} \n │ Owner: {owner}\n │ ObjectType: {object_type} \n │ Version: {version}\n │ Digest: {digest}\n └──"
                )
            }
        }
    }
}
