// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

interface Timelocked {
    id: UID;
    locked: Balance;
    expiration_timestamp_ms: number; // The epoch time stamp of when the lock expires
    label: string;
}

interface Balance {
    value: number;
}

interface UID {
    id: {
        bytes: string;
    };
}

// https://github.com/iotaledger/iota/blob/1ec56b585905d7b96fb059a9f47135df6a82cd89/crates/iota-types/src/timelock/stardust_upgrade_label.rs#L12
const VESTING_LABEL =
    '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL';

export const MOCKED_VESTING_TIMELOCKED_OBJECT: Timelocked[] = [
    {
        id: { id: { bytes: '0x3812f162182169816ef603d3e54ceb683f9a1f8d30f76c856e78169676ce2670' } },
        locked: { value: 10 },
        expiration_timestamp_ms: 1000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655' } },
        locked: { value: 100 },
        expiration_timestamp_ms: 10000000,
        label: VESTING_LABEL,
    },
];
