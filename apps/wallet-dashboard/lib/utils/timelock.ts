// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SUPPLY_INCREASE_VESTING_LABEL } from '../constants';
import { Timelocked, TimelockedStakedIota } from '../interfaces';

export function isTimelockedStakedIota(
    obj: Timelocked | TimelockedStakedIota,
): obj is TimelockedStakedIota {
    const referenceProperty: keyof TimelockedStakedIota = 'stakedIota';
    return referenceProperty in obj;
}

export function isTimelocked(obj: Timelocked | TimelockedStakedIota): obj is Timelocked {
    const referenceProperty: keyof Timelocked = 'locked';
    return referenceProperty in obj;
}

export function isVesting(obj: Timelocked | TimelockedStakedIota): boolean {
    return obj.label === SUPPLY_INCREASE_VESTING_LABEL;
}

export function isTimelockedUnlocked(
    timelockedObject: Timelocked | TimelockedStakedIota,
    currentEpochMs: number,
): boolean {
    return timelockedObject.expirationTimestampMs <= currentEpochMs;
}
