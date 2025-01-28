// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaEvent } from '@iota/iota-sdk/client';
import { getUnstakeDetailsFromEvents, getStakeDetailsFromEvents } from '.';

export function getTransactionAmountForTimelocked(
    events: IotaEvent[],
    isTimelockedStaking: boolean,
    isTimelockedUnstaking: boolean,
): bigint | undefined | string {
    if (!events) return;

    if (isTimelockedStaking) {
        const { totalStakedAmount } = getStakeDetailsFromEvents(events);
        return totalStakedAmount;
    } else if (isTimelockedUnstaking) {
        const { totalUnstakeAmount } = getUnstakeDetailsFromEvents(events);
        return totalUnstakeAmount;
    }
}
