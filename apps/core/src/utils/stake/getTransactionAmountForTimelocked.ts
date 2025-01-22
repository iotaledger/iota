// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaEvent } from '@iota/iota-sdk/client';
import {
    getUnstakeDetailsFromEvents,
    checkIfIsTimelockedStaking,
    getStakeDetailsFromEvents,
} from '.';

export function getTransactionAmountForTimelocked(
    events: IotaEvent[],
): bigint | undefined | string {
    if (!events) return;
    const { isTimelockedStaking, isTimelockedUnstaking } = checkIfIsTimelockedStaking(events);

    if (isTimelockedStaking) {
        const { totalStakedAmount } = getStakeDetailsFromEvents(events);
        return totalStakedAmount;
    } else if (isTimelockedUnstaking) {
        const { totalUnstakeAmount } = getUnstakeDetailsFromEvents(events);
        return totalUnstakeAmount;
    }
}
