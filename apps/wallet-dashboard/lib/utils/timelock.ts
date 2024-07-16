// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
