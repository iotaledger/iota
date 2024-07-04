// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// https://github.com/iotaledger/iota/blob/1ec56b585905d7b96fb059a9f47135df6a82cd89/crates/iota-types/src/timelock/stardust_upgrade_label.rs#L12
const VESTING_LABEL =
    '00000000000000000000000000000000000000000000000000000000000010cf::stardust_upgrade_label::STARDUST_UPGRADE_LABEL';

interface ID {
    bytes: string;
}

interface UID {
    id: ID;
}

interface Balance {
    value: number;
}

interface Timelocked {
    id: UID;
    locked: Balance;
    expirationTimestampMs: number; // The epoch time stamp of when the lock expires
    label?: string;
}

interface StakedIota {
    id: UID;
    poolId: ID;
    stakeActivationEpoch: number;
    principal: Balance;
}

interface TimelockedStakedIota {
    id: UID;
    stakedIota: StakedIota;
    expirationTimestampMs: number;
    label?: string;
}

export const MOCKED_VESTING_TIMELOCKED_OBJECT: Timelocked[] = [
    {
        id: { id: { bytes: '0x3812f162182169816ef603d3e54ceb683f9a1f8d30f76c856e78169676ce2670' } },
        locked: { value: 10 },
        expirationTimestampMs: 1000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0xebe40a263480190dcd7939447ee01aefa73d6f3cc33c90ef7bf905abf8728655' } },
        locked: { value: 100 },
        expirationTimestampMs: 10000000,
        label: VESTING_LABEL,
    },
];

export const MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS: TimelockedStakedIota[] = [
    {
        id: { id: { bytes: '0xd252a449e46df3a78a94982f8bec2a9a7ab6251d0852942fd54d438b888756ee' } },
        stakedIota: {
            id: {
                id: { bytes: '0x4846a1f1030deffd9dea59016402d832588cf7e0c27b9e4c1a63d2b5e152873a' },
            },
            poolId: { bytes: '0xaeab97f96cf9877fee2883315d459552b2b921edc16d7ceac6eab944dd88919c' },
            stakeActivationEpoch: 1000,
            principal: { value: 100 },
        },
        expirationTimestampMs: 1720091758000,
        label: VESTING_LABEL,
    },
    {
        id: { id: { bytes: '0x1f9310238ee9298fb703c3419030b35b22bb1cc37113e3bb5007c99aec79e5b8' } },
        stakedIota: {
            id: {
                id: { bytes: '0x8267a85a21bb527ad4545bc29452cca715d3a1aa8975e4ef1e77f9862c9a9244' },
            },
            poolId: { bytes: '0x4216768d1e645dd1c0ee15f118b99935362adecfaf305aeb13690f14105be158' },
            stakeActivationEpoch: 1010,
            principal: { value: 250 },
        },
        expirationTimestampMs: 1720092138000,
        label: VESTING_LABEL,
    },
];

export const MOCKED_VESTING_TIMELOCKED_AND_TIMELOCK_STAKED_OBJECTS: (
    | Timelocked
    | TimelockedStakedIota
)[] = [...MOCKED_VESTING_TIMELOCKED_OBJECT, ...MOCKED_VESTING_TIMELOCKED_STAKED_OBJECTS];
