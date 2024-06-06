// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use diesel::prelude::*;

use crate::{schema::packages, types::IndexedPackage};

#[derive(Queryable, Insertable, Clone, Debug, Identifiable)]
#[diesel(table_name = packages, primary_key(package_id))]
pub struct StoredPackage {
    pub package_id: Vec<u8>,
    pub move_package: Vec<u8>,
}

impl From<IndexedPackage> for StoredPackage {
    fn from(p: IndexedPackage) -> Self {
        Self {
            package_id: p.package_id.to_vec(),
            move_package: bcs::to_bytes(&p.move_package).unwrap(),
        }
    }
}
