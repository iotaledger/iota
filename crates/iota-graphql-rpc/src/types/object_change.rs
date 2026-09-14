// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use iota_sdk_types::{ChangedObject, IdOperation, ObjectOut, Version};

use crate::types::{iota_address::IotaAddress, object::Object};

/// Represents the source of an object change (derived from transaction kind)
#[derive(Clone, Debug)]
pub(crate) enum ObjectChangeSource {
    /// Object change from a checkpointed transaction
    Checkpointed,
    /// Object change from an executed (not yet checkpointed) transaction
    Executed,
    /// Object change from a dry run transaction (dryRunTransactionBlock)
    DryRun,
}

pub(crate) struct ObjectChange {
    pub native: ChangedObject,
    /// The transaction's lamport version, which a written object takes.
    pub lamport_version: Version,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
    /// The source of this object change (derived from transaction kind)
    pub source: ObjectChangeSource,
}

/// Effect on an individual Object (keyed by its ID).
#[Object]
impl ObjectChange {
    /// The address of the object that has changed.
    async fn address(&self) -> IotaAddress {
        self.native.object_id.into()
    }

    /// The contents of the object immediately before the transaction.
    async fn input_state(&self, ctx: &Context<'_>) -> Result<Option<Object>> {
        let Some(version) = self.native.input_state.opt_version() else {
            return Ok(None);
        };

        let object_lookup = match self.source {
            ObjectChangeSource::Executed => Object::at_optimistic_version(version.as_u64()),
            ObjectChangeSource::Checkpointed | ObjectChangeSource::DryRun => {
                Object::at_version(version.as_u64(), self.checkpoint_viewed_at)
            }
        };
        Object::query(ctx, self.native.object_id.into(), object_lookup)
            .await
            .extend()
    }

    /// The contents of the object immediately after the transaction.
    async fn output_state(&self, ctx: &Context<'_>) -> Result<Option<Object>> {
        let Some(version) = self.output_version() else {
            return Ok(None);
        };

        let object_lookup = match self.source {
            ObjectChangeSource::Executed => Object::at_optimistic_version(version.as_u64()),
            ObjectChangeSource::Checkpointed | ObjectChangeSource::DryRun => {
                Object::at_version(version.as_u64(), self.checkpoint_viewed_at)
            }
        };
        Object::query(ctx, self.native.object_id.into(), object_lookup)
            .await
            .extend()
    }

    /// Whether the ID was created in this transaction.
    async fn id_created(&self) -> Option<bool> {
        Some(self.native.id_operation == IdOperation::Created)
    }

    /// Whether the ID was deleted in this transaction.
    async fn id_deleted(&self) -> Option<bool> {
        Some(self.native.id_operation == IdOperation::Deleted)
    }

    /// The version the object is at after the transaction, or `None` if it no
    /// longer exists.
    ///
    /// A written object takes the version the transaction assigned, which the
    /// effects entry does not carry; a package keeps the version it was
    /// published or upgraded at.
    #[graphql(skip)]
    fn output_version(&self) -> Option<Version> {
        match self.native.output_state {
            ObjectOut::ObjectWrite { .. } => Some(self.lamport_version),
            ObjectOut::PackageWrite { version, .. } => Some(version),
            ObjectOut::Missing => None,
            _ => unimplemented!("a new ObjectOut enum variant was added and needs to be handled"),
        }
    }
}

#[cfg(test)]
mod tests {
    use iota_sdk_types::{ObjectDigest, ObjectId, ObjectIn, Owner, Version};

    use super::*;

    /// A written object ends at the version the transaction assigned, which the
    /// effects entry does not carry; a package keeps its own; a removed object
    /// has none.
    #[test]
    fn the_output_version_depends_on_how_the_object_was_written() {
        let lamport = Version::from_u64(4);
        let package_version = Version::from_u64(2);
        let digest = ObjectDigest::new([1; 32]);

        let output_version = |output_state| {
            ObjectChange {
                native: ChangedObject {
                    object_id: ObjectId::ZERO,
                    input_state: ObjectIn::Missing,
                    output_state,
                    id_operation: IdOperation::None,
                },
                lamport_version: lamport,
                checkpoint_viewed_at: 0,
                source: ObjectChangeSource::Checkpointed,
            }
            .output_version()
        };

        assert_eq!(
            output_version(ObjectOut::ObjectWrite {
                digest,
                owner: Owner::Immutable,
            }),
            Some(lamport)
        );
        assert_eq!(
            output_version(ObjectOut::PackageWrite {
                version: package_version,
                digest,
            }),
            Some(package_version)
        );
        assert_eq!(output_version(ObjectOut::Missing), None);
    }
}
