// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Predicates and utility functions based on gas versions.
//

use crate::gas_model::{tables::initial_cost_schedule_v1, units_types::CostTable};

/// If true, do not charge the entire budget on storage OOG
pub fn dont_charge_budget_on_storage_oog(gas_model_version: u64) -> bool {
    gas_model_version >= 4
}

/// If true, enable the check for gas price too high
pub fn gas_price_too_high(gas_model_version: u64) -> bool {
    gas_model_version >= 4
}

/// If true, input object bytes are treated as memory allocated in Move and
/// charged according to the bucket they end up in.
pub fn charge_input_as_memory(gas_model_version: u64) -> bool {
    gas_model_version == 4
}

/// If true, calculate value sizes using the legacy size calculation.
pub fn use_legacy_abstract_size(gas_model_version: u64) -> bool {
    gas_model_version <= 7
}

// If true, charge differently for package upgrades
pub fn charge_upgrades(gas_model_version: u64) -> bool {
    gas_model_version >= 7
}

// Return the version supported cost table
pub fn cost_table_for_version(_gas_model: u64) -> CostTable {
    initial_cost_schedule_v1()
}
