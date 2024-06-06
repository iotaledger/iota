// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_enum_compat_util::*;

use crate::{IotaMoveStruct, IotaMoveValue};

#[test]
fn enforce_order_test() {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "staged", "iota_move_struct.yaml"]);
    check_enum_compat_order::<IotaMoveStruct>(path);

    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.extend(["tests", "staged", "iota_move_value.yaml"]);
    check_enum_compat_order::<IotaMoveValue>(path);
}
