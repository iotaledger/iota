// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Transaction response types that provide a better API over the changes that occurred.

use move_core_types::language_storage::StructTag;
use iota_json_rpc_types::{ObjectChange as IotaObjectChange, IotaTransactionBlockResponse};
use iota_types::{
    base_types::{ObjectID, ObjectRef, SequenceNumber, IotaAddress},
    digests::{ObjectDigest, TransactionDigest},
    object::Owner,
};

use crate::ClientError;

/// A response from an executed transaction.
#[derive(Debug, Clone)]
pub struct TransactionResponse {
    /// The transaction digest.
    pub digest: TransactionDigest,
    /// The set of published modules.
    pub published: Vec<Published>,
    /// The set of created objects.
    pub created: Vec<Created>,
    /// The set of mutated objects.
    pub mutated: Vec<Mutated>,
    /// The set of deleted objects.
    pub deleted: Vec<Deleted>,
    /// The set of transferred objects.
    pub transferred: Vec<Transferred>,
    /// The set of wrapped objects.
    pub wrapped: Vec<Wrapped>,
}

#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub enum ObjectChange {
    Published(Published),
    Created(Created),
    Transferred(Transferred),
    Mutated(Mutated),
    Deleted(Deleted),
    Wrapped(Wrapped),
}

/// A published module.
#[derive(Debug, Clone)]
pub struct Published {
    /// The package ID.
    pub package_id: ObjectID,
    /// The latest version of the object.
    pub version: SequenceNumber,
    /// The object digest.
    pub digest: ObjectDigest,
    /// The list of published module names.
    pub modules: Vec<String>,
}

/// A created object.
#[derive(Debug, Clone)]
pub struct Created {
    /// The sender address.
    pub sender: IotaAddress,
    /// The owner of the object.
    pub owner: Owner,
    /// The object's type information.
    pub object_type: StructTag,
    /// The object ID.
    pub object_id: ObjectID,
    /// The latest version of the object.
    pub version: SequenceNumber,
    /// The object digest.
    pub digest: ObjectDigest,
}

impl Created {
    /// Get the object reference information.
    pub fn object_ref(&self) -> ObjectRef {
        (self.object_id, self.version, self.digest)
    }
}

/// Transfer objects to new address / wrap in another object
#[derive(Debug, Clone)]
pub struct Transferred {
    /// The sender address.
    pub sender: IotaAddress,
    /// The new owner of the object.
    pub recipient: Owner,
    /// The object's type information.
    pub object_type: StructTag,
    /// The object ID.
    pub object_id: ObjectID,
    /// The latest version of the object.
    pub version: SequenceNumber,
    /// The object digest.
    pub digest: ObjectDigest,
}

impl Transferred {
    /// Get the object reference information.
    pub fn object_ref(&self) -> ObjectRef {
        (self.object_id, self.version, self.digest)
    }
}

/// Object mutated.
#[derive(Debug, Clone)]
pub struct Mutated {
    /// The sender address.
    pub sender: IotaAddress,
    /// The owner of the object.
    pub owner: Owner,
    /// The object's type information.
    pub object_type: StructTag,
    /// The object ID.
    pub object_id: ObjectID,
    /// The latest version of the object.
    pub version: SequenceNumber,
    /// The previous version of the object.
    pub previous_version: SequenceNumber,
    /// The object digest.
    pub digest: ObjectDigest,
}

impl Mutated {
    /// Get the object reference information.
    pub fn object_ref(&self) -> ObjectRef {
        (self.object_id, self.version, self.digest)
    }
}

/// Delete object
#[derive(Debug, Clone)]
pub struct Deleted {
    /// The sender address.
    pub sender: IotaAddress,
    /// The object's type information.
    pub object_type: StructTag,
    /// The object ID.
    pub object_id: ObjectID,
    /// The latest version of the object.
    pub version: SequenceNumber,
}

/// Wrapped object
#[derive(Debug, Clone)]
pub struct Wrapped {
    /// The sender address.
    pub sender: IotaAddress,
    /// The object's type information.
    pub object_type: StructTag,
    /// The object ID.
    pub object_id: ObjectID,
    /// The latest version of the object.
    pub version: SequenceNumber,
}

impl TryFrom<IotaTransactionBlockResponse> for TransactionResponse {
    type Error = ClientError;

    fn try_from(value: IotaTransactionBlockResponse) -> Result<Self, Self::Error> {
        let changes = value
            .object_changes
            .ok_or_else(|| ClientError::MissingObjectChanges)?;
        Ok(changes.into_iter().fold(
            TransactionResponse {
                digest: value.digest,
                published: Default::default(),
                created: Default::default(),
                mutated: Default::default(),
                deleted: Default::default(),
                transferred: Default::default(),
                wrapped: Default::default(),
            },
            |mut changes, o| {
                match o {
                    IotaObjectChange::Published {
                        package_id,
                        version,
                        digest,
                        modules,
                    } => changes.published.push(Published {
                        package_id,
                        version,
                        digest,
                        modules,
                    }),
                    IotaObjectChange::Transferred {
                        sender,
                        recipient,
                        object_type,
                        object_id,
                        version,
                        digest,
                    } => changes.transferred.push(Transferred {
                        sender,
                        recipient,
                        object_type,
                        object_id,
                        version,
                        digest,
                    }),
                    IotaObjectChange::Mutated {
                        sender,
                        owner,
                        object_type,
                        object_id,
                        version,
                        previous_version,
                        digest,
                    } => changes.mutated.push(Mutated {
                        sender,
                        owner,
                        object_type,
                        object_id,
                        version,
                        previous_version,
                        digest,
                    }),
                    IotaObjectChange::Deleted {
                        sender,
                        object_type,
                        object_id,
                        version,
                    } => changes.deleted.push(Deleted {
                        sender,
                        object_type,
                        object_id,
                        version,
                    }),
                    IotaObjectChange::Wrapped {
                        sender,
                        object_type,
                        object_id,
                        version,
                    } => changes.wrapped.push(Wrapped {
                        sender,
                        object_type,
                        object_id,
                        version,
                    }),
                    IotaObjectChange::Created {
                        sender,
                        owner,
                        object_type,
                        object_id,
                        version,
                        digest,
                    } => changes.created.push(Created {
                        sender,
                        owner,
                        object_type,
                        object_id,
                        version,
                        digest,
                    }),
                }
                changes
            },
        ))
    }
}
