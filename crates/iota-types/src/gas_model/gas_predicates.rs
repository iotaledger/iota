// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Predicates and utility functions based on gas versions.
//

use crate::gas_model::{tables::initial_cost_schedule_v1, units_types::CostTable};

// Return the version supported cost table
pub fn cost_table_for_version(_gas_model: u64) -> CostTable {
    initial_cost_schedule_v1()
}
