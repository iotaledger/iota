// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Predicates and utility functions based on gas versions.
//

use crate::gas_model::{tables::initial_cost_schedule_v1, units_types::CostTable};

// Threshold after which native functions contribute to virtual instruction
// count.
const V2_NATIVE_FUNCTION_CALL_THRESHOLD: u64 = 700;

// Return if the native function call threshold is exceeded
pub fn native_function_threshold_exceeded(gas_model_version: u64, num_native_calls: u64) -> bool {
    if gas_model_version > 1 {
        num_native_calls > V2_NATIVE_FUNCTION_CALL_THRESHOLD
    } else {
        false
    }
}

// Return the version supported cost table
pub fn cost_table_for_version(_gas_model: u64) -> CostTable {
    initial_cost_schedule_v1()
}
