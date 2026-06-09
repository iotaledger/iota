// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::{
    errors::IndexerError,
    schema::objects_version,
    types::{IndexedDeletedObject, IndexedObject},
};

/// Model types related to tables that support efficient execution of queries on
/// the `objects`, `objects_history` and related object tables.

#[derive(Queryable, Insertable, Debug, Identifiable, Clone, QueryableByName)]
#[diesel(table_name = objects_version, primary_key(object_id, object_version))]
pub struct StoredObjectVersion {
    pub object_id: Vec<u8>,
    pub object_version: i64,
    pub cp_sequence_number: i64,
}

impl TryFrom<&IndexedObject> for StoredObjectVersion {
    type Error = IndexerError;

    fn try_from(o: &IndexedObject) -> Result<Self, Self::Error> {
        Ok(Self {
            object_id: o.object.id().as_bytes().to_vec(),
            object_version: o.object.version().as_u64() as i64,
            cp_sequence_number: o.checkpoint_sequence_number.ok_or_else(|| {
                IndexerError::InvalidArgument(
                    "checkpoint_sequence_number is required for StoredObjectVersion".to_string(),
                )
            })? as i64,
        })
    }
}

impl From<&IndexedDeletedObject> for StoredObjectVersion {
    fn from(o: &IndexedDeletedObject) -> Self {
        Self {
            object_id: o.object_id.as_bytes().to_vec(),
            object_version: o.object_version as i64,
            cp_sequence_number: o.checkpoint_sequence_number as i64,
        }
    }
}
