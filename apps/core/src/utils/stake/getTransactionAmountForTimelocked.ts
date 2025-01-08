// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaEvent } from '@iota/iota-sdk/client';
import {
    getStakeDetailsFromEvent,
    getUnstakeDetailsFromEvent,
    checkIfIsTimelockedStaking,
} from '.';

export function getTransactionAmountForTimelocked(
    events: IotaEvent[],
): bigint | undefined | string {
    if (!events) return;
    const { isTimelockedStaking, isTimelockedUnstaking } = checkIfIsTimelockedStaking(events);

    if (isTimelockedStaking) {
        const { stakedAmount } = getStakeDetailsFromEvent(events[0]);
        return stakedAmount;
    } else if (isTimelockedUnstaking) {
        const { totalAmount } = getUnstakeDetailsFromEvent(events[0]);
        return totalAmount;
    }
}
