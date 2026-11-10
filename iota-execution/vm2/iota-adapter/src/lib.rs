// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[macro_use]
extern crate iota_types;
// Local versions of the `iota_types` invariant macros: the upstream adapter uses them in
// modules that do not import `ExecutionError`, so the type is named by its full path here.
macro_rules! make_invariant_violation {
    ($($args:expr),* $(,)?) => {{
        if cfg!(debug_assertions) {
            panic!($($args),*)
        }
        iota_types::error::ExecutionError::invariant_violation(format!($($args),*))
    }}
}

macro_rules! invariant_violation {
    ($($args:expr),* $(,)?) => {
        return Err(make_invariant_violation!($($args),*).into())
    };
}

macro_rules! assert_invariant {
    ($cond:expr, $($args:expr),* $(,)?) => {{
        if !$cond {
            invariant_violation!($($args),*)
        }
    }};
}

/// Checked integer conversion that reports an invariant violation instead of truncating.
macro_rules! checked_as {
    ($value:expr, $target_type:ty) => {{
        let v = $value;
        <$target_type>::try_from(v).map_err(|e| {
            make_invariant_violation!(
                "Value {} cannot be safely cast to {}: {:?}",
                v,
                stringify!($target_type),
                e
            )
        })
    }};
}

pub mod adapter;
pub mod compat;
pub mod data_store;
pub mod error;
pub mod execution_engine;
pub mod execution_mode;
pub mod execution_value;
pub mod gas_charger;
pub mod gas_meter;
mod sdk_conv;
pub mod static_programmable_transactions;
pub mod temporary_store;
pub mod type_layout_resolver;

#[cfg(test)]
mod unit_tests;
