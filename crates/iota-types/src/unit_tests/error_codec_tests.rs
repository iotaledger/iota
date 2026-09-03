// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Pins the declaration order of [`IotaError`] and [`UserInputError`]
//! variants against the snapshots in `tests/staged/`. See the WARNING on
//! [`IotaError`] for why the order must never change. Adding a variant at the
//! very end of the enum is the only allowed change; running these tests
//! updates the snapshot, commit it together with the new variant.

use iota_enum_compat_util::check_enum_compat_order;

use crate::error::{IotaError, UserInputError};

fn snapshot_path(file: &str) -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "staged", file]);
    path
}

#[test]
fn iota_error_variant_order_is_stable() {
    check_enum_compat_order::<IotaError>(snapshot_path("iota_error.yaml"));
}

#[test]
fn user_input_error_variant_order_is_stable() {
    check_enum_compat_order::<UserInputError>(snapshot_path("user_input_error.yaml"));
}
