// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaEvent } from '@iota/iota-sdk/client';
import { getUnstakeDetailsFromEvent, checkIfIsTimelockedStaking } from '.';

export function getTransactionAmountForTimelocked(
    events: IotaEvent[],
): bigint | undefined | string {
    if (!events) return;
    const { isTimelockedStaking, isTimelockedUnstaking } = checkIfIsTimelockedStaking(events);

    if (isTimelockedStaking) {
        let amount = 0;
        events?.forEach((event) => {
            if ((event.parsedJson as { amount: number }).amount) {
                amount += Number((event.parsedJson as { amount: number }).amount);
            }
        });
        return BigInt(amount);
    } else if (isTimelockedUnstaking) {
        const { totalAmount } = getUnstakeDetailsFromEvent(events[0]);
        return totalAmount;
    }
}
