// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{balance::Balance, base_types::ObjectID, id::UID, timelock::timelock::TimeLock};

#[test]
fn test_timelock_ser_deser_roundtrip() {
    let balance = Balance::new(100);
    let timelock = TimeLock::new(UID::new(ObjectID::random()), balance, 10);

    let timelock_bytes = timelock.to_bcs_bytes();

    let deserialized_timelock: TimeLock<Balance> = bcs::from_bytes(&timelock_bytes).unwrap();

    assert_eq!(deserialized_timelock.id(), timelock.id());
    assert_eq!(deserialized_timelock.locked(), timelock.locked());
    assert_eq!(
        deserialized_timelock.expiration_timestamp_ms(),
        timelock.expiration_timestamp_ms()
    );
}
