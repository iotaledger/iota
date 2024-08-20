// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DelegatedTimelockedStake } from '@iota/iota-sdk/client';
import { TimelockedObject } from '../interfaces';

export function isTimelockedStakedIota(
    obj: TimelockedObject | DelegatedTimelockedStake,
): obj is DelegatedTimelockedStake {
    const referenceProperty: keyof DelegatedTimelockedStake = 'stakes';
    return referenceProperty in obj;
}

export function isTimelockedObject(
    obj: TimelockedObject | DelegatedTimelockedStake,
): obj is TimelockedObject {
    const referenceProperty: keyof TimelockedObject = 'locked';
    return referenceProperty in obj;
}
