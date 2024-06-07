// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{
    balance::Balance,
    base_types::ObjectID,
    id::UID,
    timelock::{
        label::label_struct_tag_to_string, stardust_upgrade_label::stardust_upgrade_label_type,
        timelock::TimeLock,
    },
};

#[test]
fn timelock_ser_deser_roundtrip() {
    let id = UID::new(ObjectID::random());
    let balance = Balance::new(100);
    let expiration_timestamp_ms = 10;
    let label = Option::Some(label_struct_tag_to_string(stardust_upgrade_label_type()));

    let timelock = TimeLock::new(id, balance, expiration_timestamp_ms, label);

    let timelock_bytes = timelock.to_bcs_bytes();
    let deserialized_timelock: TimeLock<Balance> = bcs::from_bytes(&timelock_bytes).unwrap();

    assert_eq!(deserialized_timelock.id(), timelock.id());
    assert_eq!(deserialized_timelock.locked(), timelock.locked());
    assert_eq!(
        deserialized_timelock.expiration_timestamp_ms(),
        timelock.expiration_timestamp_ms()
    );
    assert_eq!(deserialized_timelock.label(), timelock.label());
}
