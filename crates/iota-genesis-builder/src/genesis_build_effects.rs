// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_config::{genesis::Genesis, migration_tx_data::MigrationTxData};

pub struct GenesisBuildEffects {
    pub genesis: Genesis,
    pub migration_tx_data: Option<MigrationTxData>,
}

impl GenesisBuildEffects {
    pub fn new(genesis: Genesis, migration_tx_data: Option<MigrationTxData>) -> Self {
        Self {
            genesis,
            migration_tx_data,
        }
    }
}
