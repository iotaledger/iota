// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;
use iota_field_count::FieldCount;

use crate::schema::sum_packages;

#[derive(Insertable, Debug, Clone, FieldCount)]
#[diesel(table_name = sum_packages, primary_key(package_id))]
pub struct StoredPackage {
    pub package_id: Vec<u8>,
    pub original_id: Vec<u8>,
    pub package_version: i64,
    pub move_package: Vec<u8>,
    pub cp_sequence_number: i64,
}
