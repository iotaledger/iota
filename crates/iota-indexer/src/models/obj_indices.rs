// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::schema::objects_version;

/// Model types related to tables that support efficient execution of queries
/// on the `objects` and related object tables.

#[derive(
    Queryable,
    Insertable,
    Debug,
    Identifiable,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    QueryableByName,
)]
#[diesel(table_name = objects_version, primary_key(object_id, object_version))]
pub struct StoredObjectVersion {
    pub object_id: Vec<u8>,
    pub object_version: i64,
    pub cp_sequence_number: i64,
}
