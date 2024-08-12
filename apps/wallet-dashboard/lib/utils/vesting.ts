// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SUPPLY_INCREASE_VESTING_LABEL } from '../constants';
import { TimelockedObject, TimelockedStakedIota } from '../interfaces';

export function isSupplyIncreaseVestingObject(
    obj: TimelockedObject | TimelockedStakedIota,
): boolean {
    return obj.label === SUPPLY_INCREASE_VESTING_LABEL;
}
