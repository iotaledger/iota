// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{Display, Formatter, Result};

use iota_types::{
    base_types::{Address, ObjectDigest, ObjectId, ObjectReference, Version},
    iota_serde::{IotaStructTag, Version as AsVersion},
    object::Owner,
};
use move_core_types::language_storage::StructTag;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// ObjectChange are derived from the object mutations in the TransactionEffect
/// to provide richer object information.
#[serde_as]
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum ObjectChange {
    /// Module published
    #[serde(rename_all = "camelCase")]
    Published {
        package_id: ObjectId,
        #[schemars(with = "AsVersion")]
        #[serde_as(as = "AsVersion")]
        version: Version,
        digest: ObjectDigest,
        modules: Vec<String>,
    },
    /// Transfer objects to new address / wrap in another object
    #[serde(rename_all = "camelCase")]
    Transferred {
        sender: Address,
        recipient: Owner,
        #[schemars(with = "String")]
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectId,
        #[schemars(with = "AsVersion")]
        #[serde_as(as = "AsVersion")]
        version: Version,
        digest: ObjectDigest,
    },
    /// Object mutated.
    #[serde(rename_all = "camelCase")]
    Mutated {
        sender: Address,
        owner: Owner,
        #[schemars(with = "String")]
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectId,
        #[schemars(with = "AsVersion")]
        #[serde_as(as = "AsVersion")]
        version: Version,
        #[schemars(with = "AsVersion")]
        #[serde_as(as = "AsVersion")]
        previous_version: Version,
        digest: ObjectDigest,
    },
    /// Delete object
    #[serde(rename_all = "camelCase")]
    Deleted {
        sender: Address,
        #[schemars(with = "String")]
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectId,
        #[schemars(with = "AsVersion")]
        #[serde_as(as = "AsVersion")]
        version: Version,
    },
    /// Wrapped object
    #[serde(rename_all = "camelCase")]
    Wrapped {
        sender: Address,
        #[schemars(with = "String")]
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectId,
        #[schemars(with = "AsVersion")]
        #[serde_as(as = "AsVersion")]
        version: Version,
    },
    /// New object creation
    #[serde(rename_all = "camelCase")]
    Created {
        sender: Address,
        owner: Owner,
        #[schemars(with = "String")]
        #[serde_as(as = "IotaStructTag")]
        object_type: StructTag,
        object_id: ObjectId,
        #[schemars(with = "AsVersion")]
        #[serde_as(as = "AsVersion")]
        version: Version,
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
            | ObjectChange::Created { object_id, .. } => *object_id,
        }
    }

    pub fn object_ref(&self) -> ObjectReference {
        match self {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                ..
            } => ObjectReference::new(*package_id, *version, *digest),
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
            | ObjectChange::Created {
                object_id,
                version,
                digest,
                ..
            } => ObjectReference::new(*object_id, *version, *digest),
            ObjectChange::Deleted {
                object_id, version, ..
            } => ObjectReference::new(*object_id, *version, ObjectDigest::OBJECT_DELETED),
            ObjectChange::Wrapped {
                object_id, version, ..
            } => ObjectReference::new(*object_id, *version, ObjectDigest::OBJECT_WRAPPED),
        }
    }

    pub fn mask_for_test(&mut self, new_version: Version, new_digest: ObjectDigest) {
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
                    u64::from(*version),
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
                    " ┌──\n │ ObjectId: {}\n │ Sender: {} \n │ Recipient: {}\n │ ObjectType: {} \n │ Version: {}\n │ Digest: {}\n └──",
                    object_id,
                    sender,
                    recipient,
                    object_type,
                    u64::from(*version),
                    digest
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
                    " ┌──\n │ ObjectId: {}\n │ Sender: {} \n │ Owner: {}\n │ ObjectType: {} \n │ Version: {}\n │ Digest: {}\n └──",
                    object_id,
                    sender,
                    owner,
                    object_type,
                    u64::from(*version),
                    digest
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
                    " ┌──\n │ ObjectId: {}\n │ Sender: {} \n │ ObjectType: {} \n │ Version: {}\n └──",
                    object_id,
                    sender,
                    object_type,
                    u64::from(*version)
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
                    " ┌──\n │ ObjectId: {}\n │ Sender: {} \n │ ObjectType: {} \n │ Version: {}\n └──",
                    object_id,
                    sender,
                    object_type,
                    u64::from(*version)
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
                    " ┌──\n │ ObjectId: {}\n │ Sender: {} \n │ Owner: {}\n │ ObjectType: {} \n │ Version: {}\n │ Digest: {}\n └──",
                    object_id,
                    sender,
                    owner,
                    object_type,
                    u64::from(*version),
                    digest
                )
            }
        }
    }
}
