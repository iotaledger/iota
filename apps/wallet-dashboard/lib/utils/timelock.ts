// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TimelockedObject, TimelockedStakedIota } from '../interfaces';

export function isTimelockedStakedIota(
    obj: TimelockedObject | TimelockedStakedIota,
): obj is TimelockedStakedIota {
    const referenceProperty: keyof TimelockedStakedIota = 'stakedIota';
    return referenceProperty in obj;
}

export function isTimelockedObject(
    obj: TimelockedObject | TimelockedStakedIota,
): obj is TimelockedObject {
    const referenceProperty: keyof TimelockedObject = 'locked';
    return referenceProperty in obj;
}
