// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::timelock::{
    label::label_struct_tag_to_string,
    stardust_upgrade_label::{STARDUST_UPGRADE_LABEL_VALUE, stardust_upgrade_label_type},
};

#[test]
fn stardust_upgrade_label_check() {
    let label = label_struct_tag_to_string(stardust_upgrade_label_type());

    assert_eq!(label, STARDUST_UPGRADE_LABEL_VALUE);
}
