// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use async_graphql::*;
use iota_sdk_types::{ChangedObject, IdOperation, ObjectId, ObjectOut, Version};
use iota_types::object::Object as NativeObject;

use crate::types::{iota_address::IotaAddress, object::Object};

/// Represents the source of an object change (derived from transaction kind)
#[derive(Clone, Debug)]
pub(crate) enum ObjectChangeSource {
    /// Object change from a checkpointed transaction
    Checkpointed,
    /// Object change from an executed (not yet checkpointed) transaction
    Executed,
    /// Object change from a simulated transaction (`dryRunTransactionBlock`)
    Simulated,
}

pub(crate) struct ObjectChange {
    pub native: ChangedObject,
    /// The transaction's lamport version, which a written object takes.
    pub lamport_version: Version,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
    /// The source of this object change (derived from transaction kind)
    pub source: ObjectChangeSource,
    /// For a simulated transaction, the simulation's input and output objects.
    /// Object state is resolved from here rather than the
    /// database: objects a simulation writes are not indexed, and the state it
    /// reads may be ahead of indexer if the node is ahead. `None` for the other
    /// sources.
    pub input_objects: Option<Arc<BTreeMap<ObjectId, NativeObject>>>,
    pub output_objects: Option<Arc<BTreeMap<ObjectId, NativeObject>>>,
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

        // Resolve from the simulation's input objects: it ran against the
        // fullnode's state, which may be ahead of the index. Fall back to the
        // database when the simulation did not return them (e.g. dev inspect).
        if let ObjectChangeSource::Simulated = self.source {
            if let Some(object) = self.simulation_object(&self.input_objects) {
                return Ok(Some(object));
            }
        }

        let object_lookup = match self.source {
            ObjectChangeSource::Executed => Object::at_optimistic_version(version.as_u64()),
            ObjectChangeSource::Checkpointed | ObjectChangeSource::Simulated => {
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

        // Objects a simulation writes are never indexed, so resolve their state
        // from the simulation's output objects. Fall back to the database when
        // the simulation did not return them (e.g. dev inspect).
        if let ObjectChangeSource::Simulated = self.source {
            if let Some(object) = self.simulation_object(&self.output_objects) {
                return Ok(Some(object));
            }
        }

        let object_lookup = match self.source {
            ObjectChangeSource::Executed => Object::at_optimistic_version(version.as_u64()),
            ObjectChangeSource::Checkpointed | ObjectChangeSource::Simulated => {
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

    /// This object's state from objects used/produced by simulation, if
    /// present. Used instead of a database lookup since simulation input
    /// may not be indexed yet, and simulation output is never indexed. Pass
    /// the input objects for the prior state, the output objects for the
    /// resulting state.
    #[graphql(skip)]
    fn simulation_object(
        &self,
        objects: &Option<Arc<BTreeMap<ObjectId, NativeObject>>>,
    ) -> Option<Object> {
        let native = objects.as_ref()?.get(&self.native.object_id)?;
        Some(Object::from_native(
            self.native.object_id.into(),
            native.clone(),
            self.checkpoint_viewed_at,
            None,
        ))
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
                input_objects: None,
                output_objects: None,
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
